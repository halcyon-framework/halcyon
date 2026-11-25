#![cfg(debug_assertions)]

mod textfield_motion_metal;
mod compound_fill;


use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{cell::Cell, rc::Rc, time::Instant};

use crate::compositor::{Compositor, SceneKey, SurfaceId};
use crate::core::app::RetainedSceneTestHarness;
use crate::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomainState, KeyCode, KeyboardAuthorityMode,
    Modifiers, PointerButton, PointerButtonsState, PointerDevice, PointerEvent, PointerEventKind,
    PointerId, PointerType, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icon::{IconId, IconRegistry};
use crate::core::icons::builtin_registry;
use crate::core::layout::{Constraints, FlexParams};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{LAYER_CONTENT, LAYER_OVERLAY, LAYER_SCROLLBAR};
use crate::core::scene::{Color, SceneBuilder, SceneGraph, SceneNode, TextNode};
use crate::core::scroll_container::HorizontalScrollLink;
use crate::core::theme::Theme;
use crate::core::widget::EventContext;
use crate::core::widget_tree::WidgetTree;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::renderer::DrawPlanBuilder;
use crate::renderer::batcher::FrameBatches;
use crate::renderer::compiled_draw_plan::SurfaceDrawCommand;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::path::Path;
use crate::renderer::wgpu_renderer::{
    WgpuRenderer, build_surface_draw_commands, merge_ordered_surface_draw_commands,
};
use crate::runtime::binding_registry::BindingRegistry;
use crate::widgets::ComboBox;
use crate::widgets::event_log::{EventLog, EventLogConfig, EventLogEntry, EventLogLevel};
use crate::widgets::flex::Flex;
use crate::widgets::list_view::{ListView, ListViewBody, RowId};
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::tabular_view::column::{Column, ColumnId, ColumnScrollDomain};
use crate::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use crate::widgets::tag_input::{TagInput, TagItem};

const WIDTH: u32 = 96;
const HEIGHT: u32 = 64;
const IMAGE_ID: u64 = 359_001;

// Include the actual example in this owner, resolving its ordinary imports to
// this crate. No framework API or substitute scene producer is introduced.
mod glass_material_lab_producer {
    use crate as halcyon;
    include!("../../examples/glass_material_lab.rs");
}

#[cfg(target_os = "macos")]
#[test]
fn glass_material_lab_crossing_matches_the_metal_coating_reference_1x_2x() {
    use crate::core::text_engine::TextEngine;
    use crate::renderer::pipelines::analytic_surface::tests::coating_reference;

    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(1280, 820, scale).expect("actual Metal required");
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        assert!(renderer.gpu_resources().format.is_srgb());
        renderer.text_engine_mut().set_scale(scale);
        let width = (1280.0 * scale) as usize;
        let mut endpoint = None;
        for (millis, scene, face, optics) in
            glass_material_lab_producer::tests::crossing_frames(scale, renderer.text_engine_mut())
        {
            let reference = |u, v| {
                coating_reference(&optics, [u, v], -32.0, [0.0, 0.0], [0.08, 0.08, 0.08, 1.0])
            };
            let anchor = match millis {
                0 | 2401 | 2500 => Some([0.139736, 0.108569, 0.141035]),
                1200 => Some([0.264, 0.168, 0.268]),
                _ => None,
            };
            if let Some(anchor) = anchor {
                for (actual, expected) in reference(0.5, 0.5).into_iter().zip(anchor) {
                    assert!((actual - expected).abs() < 2e-6, "reference at {millis}ms");
                }
            }

            let pixels = render_scene_at_scale(&mut renderer, &compositor, id, scene, scale);
            let sample = |u: f32| {
                let x = ((face.x + face.width * u) * scale) as usize;
                let y = ((face.y + face.height * 0.5) * scale) as usize;
                let u = ((x as f32 + 0.5) / scale - face.x) / face.width;
                let v = ((y as f32 + 0.5) / scale - face.y) / face.height;
                let expected = reference(f64::from(u), f64::from(v));
                let pos = (y * width + x) * 4;
                let pixel: [u8; 4] = pixels[pos..pos + 4].try_into().unwrap();
                for (actual, linear) in pixel[..3].iter().zip(expected) {
                    let srgb = crate::core::scene::linear_to_srgb(linear as f32);
                    assert!(
                        actual.abs_diff((srgb * 255.0).round() as u8) <= 2,
                        "{scale}x at {millis}ms UV ({u}, {v}): {pixel:?} vs {expected:?}"
                    );
                }
                assert_eq!(pixel[3], 255);
                pixel
            };
            let [left, center, right] = [0.20, 0.50, 0.80].map(sample);
            if millis < 1200 {
                assert!(left[0] > left[2] && right[2] > right[0]);
            } else if millis > 1200 {
                assert!(left[2] > left[0] && right[0] > right[2]);
            } else {
                for channel in [0, 2] {
                    assert!(center[channel] > left[channel] && center[channel] > right[channel]);
                }
            }
            if millis >= 2401 {
                let face_left = (face.x * scale) as usize;
                let face_top = (face.y * scale) as usize;
                let face_width = (face.width * scale) as usize;
                let face_height = (face.height * scale) as usize;
                let face_pixels: Vec<_> = (face_top..face_top + face_height)
                    .flat_map(|y| {
                        let pos = (y * width + face_left) * 4;
                        pixels[pos..pos + face_width * 4].iter().copied()
                    })
                    .collect();
                if let Some(previous) = &endpoint {
                    assert!(
                        previous == &face_pixels,
                        "terminal material stays pixel-identical"
                    );
                } else {
                    endpoint = Some(face_pixels);
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn bloom_read_hdr(gpu: &crate::renderer::wgpu_renderer::GpuResources, texture: &wgpu::Texture) -> Vec<[f64; 4]> {
    let row = texture.width() * 8;
    let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("HAL395 test readback"),
        size: u64::from(padded) * u64::from(texture.height()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = gpu.device.create_command_encoder(&Default::default());
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    let submission = gpu.queue.submit([copy.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range().expect("mapped readback range");
    data.chunks_exact(padded as usize).flat_map(|line| {
        line[..row as usize].chunks_exact(8).map(|rgba| [0, 1, 2, 3].map(|channel| {
            let bits = u16::from_le_bytes([rgba[channel * 2], rgba[channel * 2 + 1]]);
            let fraction = f64::from(bits & 1023) / 1024.0;
            let exp = i32::from(bits >> 10);
            assert!(exp < 31, "HDR premultiplied data stays nonnegative and finite");
            if exp == 0 { fraction * 2_f64.powi(-14) } else { (1.0 + fraction) * 2_f64.powi(exp - 15) }
        }))
    }).collect()
}

#[cfg(target_os = "macos")]
#[test]
fn bloom_metal_matches_source_gaussian_and_screen_at_1x_2x() {
    use crate::core::paint::{Color as PaintColor, FrostedGlassAppearance, RoundedCorners as PaintCorners};
    use crate::core::text_engine::TextEngine;
    use crate::renderer::bloom::tests::{crossing, extraction};
    use crate::renderer::pipelines::analytic_surface::tests::distance;
    use crate::renderer::{BloomCompileRefusal, RendererCompileError};

    let rect = RectF::new(1.0, 6.0, 64.0, 44.0);
    let corners = PaintCorners::uniform(9.0);
    let body = glass_material_lab_producer::neutral_smoked_glass();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, id) = setup_with_size_and_scale(92, 68, scale).expect("HAL395 Metal");
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.adapter.get_info().backend, wgpu::Backend::Metal);
        assert!(gpu.format.is_srgb());
        renderer.text_engine_mut().set_scale(scale);
        let run = renderer.text_engine_mut().shape("A", 15.0);
        let dimensions = [(92.0 * scale) as u32, (68.0 * scale) as u32];
        let [width, height] = dimensions;
        for frosted in [false, true] {
            let clear = if frosted { Color::from_linear(0.55, 0.55, 0.55, 1.0) }
                else { Color::from_linear(0.03, 0.04, 0.05, 1.0) };
            let render = |phase: f32, mix: f32, clear: Color, feather: f32, renderer: &mut WgpuRenderer| {
                let optics = crossing(phase, 4.0, mix);
                let mut scene = SceneBuilder::new();
                scene.push_rect(RectF::new(1.0, 3.0, 80.0, 57.0), Color::from_linear(0.12, 0.16, 0.18, 0.4), 1, None);
                if frosted {
                    let appearance = FrostedGlassAppearance::try_new(1.5, PaintColor::TRANSPARENT, 1.0)
                        .unwrap().try_with_optics(Some(optics), 0.0, 0.75).unwrap()
                        .try_with_edge_feather(feather).unwrap();
                    scene.push_frosted_glass(rect, corners, &appearance, 5, None).unwrap();
                } else {
                    scene.push_analytic_surface(rect, corners, &body.clone().with_glass_optics(Some(optics)), 5, None).unwrap();
                }
                renderer.text_engine_mut().draw(&run, 37.0, 22.0, PaintColor::BLACK, 12, &mut scene);
                scene.push_rect(RectF::new(22.0, 26.0, 2.0, 6.0), Color::BLACK, 14, None);
                let mut scene = scene.build();
                scene.clear_color = clear;
                let plan = DrawPlanBuilder::new().build_multi(
                    &HashMap::from([(id, scene)]), compositor.snapshot_multi(), &mut MeshCache::new(), |_| scale, 0,
                ).unwrap();
                assert_eq!(plan.surface_data[&id].bloom.is_some(), mix > 0.0);
                renderer.compile_multi(&plan)
            };
            let absent = render(0.5, 0.0, clear, 0.0, &mut renderer).unwrap().expect_ready("baseline");
            assert_eq!(absent.frosted_surfaces.keys().len(), usize::from(frosted));
            let base = read_compiled_surface(&renderer, &absent, id).unwrap();
            assert!(renderer.bloom_textures_for_test(id).is_none());
            let ready = render(0.5, 1.0, clear, 0.0, &mut renderer).unwrap().expect_ready("combined optical graph");
            let plan = &ready.frosted_surfaces[&id];
            let selected = plan.bloom.as_ref().unwrap();
            let names: Vec<_> = plan.topology.passes_in_order().map(|node| node.name).collect();
            let start = names.iter().position(|&name| name == "bloom_source").unwrap();
            assert_eq!(&names[start..start+4], &["bloom_source", "bloom_horizontal", "bloom_vertical", "bloom_composite"]);
            assert_eq!(names.last(), Some(&"frost_terminal"));
            if frosted { assert_eq!(names[start - 1], "frost_replace_0"); }
            let bloom = read_compiled_surface(&renderer, &ready, id).unwrap();
            let textures = renderer.bloom_textures_for_test(id).unwrap();
            let gpu_stages = textures.each_ref().map(|texture| bloom_read_hdr(&gpu, texture));
            let [v, h, b] = selected.regions();
            assert_eq!(v[0], 0, "source touches the real target edge");
            assert_eq!(h[1..2], v[1..2]);
            let texel = |region: [u32; 4], x: u32, y: u32| ((y - region[1]) * region[2] + x - region[0]) as usize;
            let run_region = |region: [u32; 4], f: &mut dyn FnMut(u32, u32) -> [f64; 4]| -> Vec<[f64; 4]> {
                let [x0, y0, w, h] = region;
                let mut pixels = Vec::with_capacity((w * h) as usize);
                for y in y0..y0+h { for x in x0..x0+w { pixels.push(f(x, y)); } }
                pixels
            };
            let optics = crossing(0.5, 4.0, 1.0);
            let sample_distance = |x: u32, y: u32| distance(rect, corners,
                Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale));
            let expected_source = run_region(v, &mut |x, y| {
                let (d, n) = sample_distance(x, y);
                let dx = (sample_distance(x & !1, y).0 - sample_distance((x & !1) + 1, y).0).abs();
                let dy = (sample_distance(x, y & !1).0 - sample_distance(x, (y & !1) + 1).0).abs();
                let aa = (dx + dy).max(0.5 / f64::from(scale));
                let a = ((aa - d)/(2.0 * aa)).clamp(0.0, 1.0);
                let coverage = a * a * (3.0 - 2.0 * a);
                let uv = [
                    f64::from(((x as f32 + 0.5)/scale - rect.x)/rect.width),
                    f64::from(((y as f32 + 0.5)/scale - rect.y)/rect.height),
                ];
                let (_, rgb) = extraction(&optics, uv, d, n);
                [rgb[0]*coverage, rgb[1]*coverage, rgb[2]*coverage, coverage].map(frost_half)
            });
            let radius = (12.0 * scale) as i32;
            let filter = |input: &[[f64; 4]], from: [u32; 4], to: [u32; 4], horizontal: bool| {
                run_region(to, &mut |x, y| {
                    let mut total = 0.0;
                    let mut sum = [0.0; 4];
                    for k in -radius..=radius {
                        let xx = x as i32 + if horizontal { k } else { 0 };
                        let yy = y as i32 + if horizontal { 0 } else { k };
                        if xx < 0 || yy < 0 || xx >= width as i32 || yy >= height as i32 { continue; }
                        let weight = f64::from((-0.5 * (f64::from(k)/(4.0*f64::from(scale))).powi(2)).exp() as f32);
                        total += weight;
                        let (xx, yy) = (xx as u32, yy as u32);
                        if (from[0]..from[0]+from[2]).contains(&xx) && (from[1]..from[1]+from[3]).contains(&yy) {
                            for (value, sample) in sum.iter_mut().zip(input[texel(from, xx, yy)]) {
                                *value += weight * sample;
                            }
                        }
                    }
                    sum.map(|value| frost_half(value/total))
                })
            };
            let expected_h = filter(&expected_source, v, h, true);
            let expected_g = filter(&expected_h, h, b, false);
            for (index, (actual, expected)) in gpu_stages.iter().zip([&expected_source, &expected_h, &expected_g]).enumerate() {
                assert_eq!(actual.len(), expected.len());
                for (actual, expected) in actual.iter().zip(expected) {
                    for (&actual, &expected) in actual[..3].iter().zip(&expected[..3]) {
                        assert!((actual - expected).abs() <= 0.002, "{scale}× frost={frosted} stage={index} {actual} != {expected}");
                    }
                }
            }
            let x = (33.0 * scale) as u32;
            let y = (30.0 * scale) as u32;
            let i = ((y * width + x) * 4) as usize;
            let g = expected_g[texel(b, x, y)];
            let dst = base[i..i + 3].iter().map(|&value| frost_decode(f64::from(value) / 255.0));
            let expected: Vec<_> = g.into_iter().zip(dst).map(|(light, dst)| {
                let screen = light / (1.0 + light);
                (frost_encode(screen + (1.0 - screen) * dst) * 255.0).round() as u8
            }).collect();
            for (&actual, expected) in bloom[i..i+3].iter().zip(expected) {
                assert!(actual.abs_diff(expected) <= 2, "{scale}× screen blend {actual} != {expected}");
            }
            assert_eq!(bloom[i+3], 255);
            assert!(bloom[i..i+3].iter().zip(&base[i..i+3]).all(|(a, b)| a > b));
            let focus = (((28.0 * scale) as u32 * width + (23.0 * scale) as u32) * 4) as usize;
            assert_eq!(&bloom[focus..focus+4], &[0, 0, 0, 255]);
            let allocations = renderer.frosted_pool_for_test().0;
            let end_with_bloom = render(1.0, 1.0, clear, 0.0, &mut renderer).unwrap().expect_ready("settled");
            let ended = read_compiled_surface(&renderer, &end_with_bloom, id).unwrap();
            assert_eq!(renderer.frosted_pool_for_test().0, allocations);
            let end_plain = render(1.0, 0.0, clear, 0.0, &mut renderer).unwrap().expect_ready("plain endpoint");
            let plain = read_compiled_surface(&renderer, &end_plain, id).unwrap();
            assert!(ended == plain, "separated light has exact zero contribution");
            assert!(matches!(render(0.5, 1.0, Color::TRANSPARENT, 0.0, &mut renderer),
                Err(RendererCompileError::Bloom { reason: BloomCompileRefusal::NonOpaqueClear, .. })));
            if frosted {
                let feathered = render(0.5, 1.0, clear, 8.0, &mut renderer)
                    .unwrap()
                    .expect_ready("feathered Frosted Glass and Bloom");
                let feathered_pixels = read_compiled_surface(&renderer, &feathered, id).unwrap();
                let textures = renderer.bloom_textures_for_test(id).unwrap();
                let feathered_stages = textures.each_ref().map(|texture| bloom_read_hdr(&gpu, texture));
                assert_eq!(gpu_stages, feathered_stages, "all three Bloom stages are independent");
                assert_ne!(bloom, feathered_pixels, "the replacement body must feather");
                assert_eq!(&feathered_pixels[focus..focus + 4], &[0, 0, 0, 255]);
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn bloom_refuses_stale_source_and_shared_plan_overflow_before_backing() {
    use crate::core::paint::{Color as PaintColor, FrostedGlassAppearance, RoundedCorners as PaintCorners};
    use crate::renderer::{BloomCompileRefusal as Refusal, RendererCompileError};
    let (mut renderer, compositor, id) =
        setup_with_size_and_scale(4096, 3072, 1.0).expect("target for Bloom budget admission");
    let optics = crate::renderer::bloom::tests::crossing(0.5, 16.0, 1.0);
    let glass = FrostedGlassAppearance::try_new(5.0, PaintColor::TRANSPARENT, 1.0)
        .unwrap().try_with_optics(Some(optics), 0.0, 1.0).unwrap();
    let mut scene = SceneBuilder::new();
    scene.push_frosted_glass(
        RectF::new(2.0, 2.0, 4092.0, 3068.0), PaintCorners::uniform(4.0), &glass, 10, None,
    ).unwrap();
    let mut plan = DrawPlanBuilder::new().build_multi(&HashMap::from([(id, scene.build())]),
        compositor.snapshot_multi(), &mut MeshCache::new(), |_| 1.0, 0).unwrap();
    let before = renderer.frosted_pool_for_test();
    assert!(matches!(renderer.compile_multi(&plan), Err(RendererCompileError::Bloom {
        reason: Refusal::ResourceBudget { limit_bytes: 268_435_456, .. }, ..
    })));
    assert_eq!(renderer.frosted_pool_for_test(), before);
    plan.surface_data.get_mut(&id).unwrap().bloom.as_mut().unwrap().ordinal += 1;
    assert!(matches!(renderer.compile_multi(&plan), Err(RendererCompileError::Bloom {
        reason: Refusal::MissingCompiledSource, ..
    })));
    assert_eq!(renderer.frosted_pool_for_test(), before);
}

#[cfg(target_os = "macos")]
fn thin_film_pixel_error(pixel: [u8; 4], expected: [f64; 4]) -> u8 {
    let rgb = [0, 1, 2].map(|channel| (frost_encode(expected[channel]) * 255.0).round() as u8);
    let alpha = (expected[3] * 255.0).round() as u8;
    pixel.into_iter().zip([rgb[0], rgb[1], rgb[2], alpha])
        .map(|(actual, expected)| actual.abs_diff(expected)).max().unwrap()
}

#[cfg(target_os = "macos")]
#[test]
fn glass_material_lab_thin_film_angles_swap_on_metal_1x_2x() {
    use crate::core::text_engine::TextEngine;
    use crate::renderer::pipelines::analytic_surface::tests::coating_reference;

    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(1280, 820, scale).expect("HAL394 requires actual Metal");
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.adapter.get_info().backend, wgpu::Backend::Metal);
        assert!(gpu.format.is_srgb());
        renderer.text_engine_mut().set_scale(scale);
        let (frames, faces, optics) =
            glass_material_lab_producer::tests::thin_film_frames(scale, renderer.text_engine_mut());
        let mut maximum_error = 0;
        let samples = frames.into_iter().enumerate().map(|(index, scene)| {
            let width = (1280.0 * scale) as usize;
            let pixels = render_scene_at_scale(&mut renderer, &compositor, id, scene, scale);
            [0, 1].map(|sample| {
                let face = faces[sample];
                let x = ((face.x + face.width * 0.5) * scale) as usize;
                let y = ((face.y + face.height * 0.5) * scale) as usize;
                let u = ((x as f32 + 0.5) / scale - face.x) / face.width;
                let v = ((y as f32 + 0.5) / scale - face.y) / face.height;
                let expected = coating_reference(&optics[sample ^ index], [u.into(), v.into()],
                    -32.0, [0.0, 0.0], [0.08, 0.08, 0.08, 1.0]);
                let pos = (y * width + x) * 4;
                let pixel = pixels[pos..pos + 4].try_into().unwrap();
                maximum_error = maximum_error.max(thin_film_pixel_error(pixel, expected));
                pixel
            })
        }).collect::<Vec<_>>();
        assert!(maximum_error <= 2, "{scale}× maximum lab error: {maximum_error}");
        let [oblique, steep] = samples[0];
        assert!(oblique[0] > oblique[2] && steep[2] > steep[0]);
        assert_eq!(samples[1], [steep, oblique]);
        eprintln!("HAL394 lab {scale}×: {samples:?}, max sRGB code error {maximum_error}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn glass_material_lab_side_reflection_glint_real_metal_1x_2x() {
    use crate::renderer::pipelines::analytic_surface::tests::distance;
    const W: usize = 304;
    const H: usize = 236;
    let directory = std::path::PathBuf::from(format!(
        "target/evidence/glass-side-reflection/metal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let mut receipts = Vec::new();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(W as u32, H as u32, scale).expect("actual Metal required");
        let info = renderer.gpu_resources().adapter.get_info();
        assert_eq!(info.backend, wgpu::Backend::Metal);
        assert!(renderer.gpu_resources().format.is_srgb());
        renderer.text_engine_mut().set_scale(scale);
        let width = (W as f32 * scale) as usize;
        let height = (H as f32 * scale) as usize;
        let mut images = Vec::new();
        let mut common_prefix = None;
        let mut common_geometry = None;
        let mut shape = None;
        let mut exclusions = None;
        for (mode, name) in [
            "absent",
            "face-only",
            "rim-only",
            "combined",
            "mirror-right",
            "black-sources",
        ]
        .into_iter()
        .enumerate()
        {
            let authored = glass_material_lab_producer::tests::side_reflection_scene(
                mode,
                renderer.text_engine_mut(),
            );
            let geometry = (authored.rect, authored.corners, authored.foreground.clone());
            if let Some(previous) = &common_geometry {
                assert_eq!(previous, &geometry);
            } else {
                common_geometry = Some(geometry);
            }
            shape = Some((authored.rect, authored.corners));
            let foreground = render_scene_at_scale(
                &mut renderer,
                &compositor,
                id,
                authored.foreground_scene,
                scale,
            );
            if let Some(previous) = &exclusions {
                assert_eq!(
                    previous, &foreground,
                    "same actual sharp foreground in every case"
                );
            } else {
                exclusions = Some(foreground.clone());
            }
            let prefix =
                render_scene_at_scale(&mut renderer, &compositor, id, authored.prefix, scale);
            if let Some(previous) = &common_prefix {
                assert_eq!(
                    previous, &prefix,
                    "all authored cases keep the actual source identical"
                );
            } else {
                common_prefix = Some(prefix.clone());
            }
            let mut expected: Vec<_> = prefix
                .chunks_exact(4)
                .map(|pixel| {
                    [
                        frost_decode(pixel[0] as f64 / 255.0),
                        frost_decode(pixel[1] as f64 / 255.0),
                        frost_decode(pixel[2] as f64 / 255.0),
                        pixel[3] as f64 / 255.0,
                    ]
                })
                .collect();
            frost_cpu_oracle(
                &mut expected,
                width,
                height,
                scale,
                authored.rect,
                authored.corners,
                &authored.appearance,
            );
            let plan = DrawPlanBuilder::new()
                .build_multi(
                    &HashMap::from([(id, authored.scene)]),
                    compositor.snapshot_multi(),
                    &mut MeshCache::new(),
                    |_| scale,
                    0,
                )
                .unwrap();
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            assert!(
                compiled
                    .text_realization(id)
                    .unwrap()
                    .outcome()
                    .is_complete(),
                "current admitted renderer text must be Complete"
            );
            let pixels = read_compiled_surface(&renderer, &compiled, id).unwrap();
            let mut worst = 0_i16;
            let mut worst_pixel = (0, 0, 0, 0, 0);
            let mut compared = 0;
            let mut sharp_cores = 0;
            for y in 0..height {
                for x in 0..width {
                    let index = (y * width + x) * 4;
                    if foreground[index..index + 4] == [255, 255, 255, 255] {
                        assert_eq!(
                            &pixels[index..index + 4],
                            &[255, 255, 255, 255],
                            "actual white opaque foreground ink remains sharp above the material"
                        );
                        sharp_cores += 1;
                    }
                    if foreground[index + 3] != 0 {
                        continue;
                    }
                    for channel in 0..4 {
                        let e = expected[y * width + x][channel];
                        let encoded = if channel == 3 { e } else { frost_encode(e) };
                        let reference = (encoded.clamp(0.0, 1.0) * 255.0).round() as i16;
                        let actual = pixels[(y * width + x) * 4 + channel] as i16;
                        let error = (actual - reference).abs();
                        if error > worst {
                            worst = error;
                            worst_pixel = (x, y, channel, actual, reference);
                        }
                        compared += 1;
                    }
                }
            }
            assert!(compared > 100_000);
            assert!(
                sharp_cores > 0,
                "actual native font supplies sharp white foreground cores"
            );
            std::fs::write(
                directory.join(format!("{scale}x-{name}-foreground.rgba")),
                &foreground,
            )
            .unwrap();
            std::fs::write(
                directory.join(format!("{scale}x-{name}-prefix.rgba")),
                &prefix,
            )
            .unwrap();
            let expected_encoded: Vec<u8> = expected
                .iter()
                .flat_map(|pixel| {
                    pixel.iter().enumerate().map(|(channel, value)| {
                        let encoded = if channel == 3 {
                            *value
                        } else {
                            frost_encode(*value)
                        };
                        (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
                    })
                })
                .collect();
            std::fs::write(
                directory.join(format!("{scale}x-{name}-expected.rgba")),
                &expected_encoded,
            )
            .unwrap();
            std::fs::write(directory.join(format!("{scale}x-{name}.rgba")), &pixels).unwrap();
            let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
            for pixel in pixels.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(directory.join(format!("{scale}x-{name}.ppm")), ppm).unwrap();
            receipts.push(serde_json::json!({"scale":scale,"case":name,"size":[width,height],"source":"actual Lab checked publication and canonical editable cell","backend":format!("{:?}",info.backend),"adapter":info.name,"text":"Complete","compared_channels":compared,"sharp_foreground_opaque_core_pixels":sharp_cores,"maximum_encoded_error":worst,"worst_pixel_xy_channel_actual_expected":worst_pixel,"face_rect":[authored.rect.x,authored.rect.y,authored.rect.width,authored.rect.height],"foreground_exclusion":"actual same-source native text mask, not approximate metrics","tolerance_codes":2}));
            std::fs::write(
                directory.join("receipts.json"),
                serde_json::to_vec_pretty(&receipts).unwrap(),
            )
            .unwrap();
            assert!(
                worst <= 2,
                "authored side reflection {name} DPR={scale} max_encoded_error={worst}; tolerance stays2"
            );
            images.push(pixels);
        }
        for mode in 1..5 {
            assert_ne!(images[0], images[mode]);
        }
        assert_ne!(images[1], images[2]);
        assert_eq!(
            images[5], images[0],
            "black sources retain authored values but contribute exactly zero coating"
        );
        let (rect, corners) = shape.unwrap();
        let exclusions = exclusions.unwrap();
        let mut contrast = Vec::new();
        for mode in 1..5 {
            let mut sum = [0.0_f64; 4];
            let mut count = [0_usize; 4];
            let mut peak = [0_i16; 4];
            for y in 0..height {
                for x in 0..width {
                    let point = Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                    if exclusions[(y * width + x) * 4 + 3] != 0 {
                        continue;
                    }
                    let (signed, _) = distance(rect, corners, point);
                    let index = (y * width + x) * 4;
                    if signed > 2.0 {
                        assert_eq!(
                            &images[mode][index..index + 4],
                            &images[0][index..index + 4],
                            "nonemissive coating has no outside halo"
                        );
                    }
                    if signed > 0.0 {
                        continue;
                    }
                    let u = (point.x - rect.x) / rect.width;
                    let v = (point.y - rect.y) / rect.height;
                    let gain = (0..3)
                        .map(|c| images[mode][index + c] as i16 - images[0][index + c] as i16)
                        .max()
                        .unwrap()
                        .max(0);
                    let regions = [
                        u < 0.4,
                        u > 0.6,
                        u < 0.18 && v < 0.24 && signed > -2.5,
                        (u > 0.9 || v > 0.9) && signed > -2.5,
                    ];
                    for region in 0..4 {
                        if regions[region] {
                            sum[region] += f64::from(gain);
                            count[region] += 1;
                            peak[region] = peak[region].max(gain);
                        }
                    }
                }
            }
            let mean: [f64; 4] = std::array::from_fn(|i| {
                assert!(count[i] > 0);
                sum[i] / count[i] as f64
            });
            contrast.push((mean, peak));
            receipts.push(serde_json::json!({"scale":scale,"case":(["face-only","rim-only","combined","mirror-right"][mode-1]),"metric":"positive encoded RGB channel gain versus identical uncoated source; reference contrast, not monitor measurement","regions":["left face","right face","short upper-left edge","opposite right/bottom edge"],"mean_codes":mean,"peak_codes":peak,"sample_pixels":count}));
        }
        assert!(
            contrast[2].0[0] > contrast[2].0[1],
            "authored left source follows its side"
        );
        assert!(
            contrast[3].0[1] > contrast[3].0[0],
            "mirrored directions/centers/rotations move reflection to right"
        );
        assert!(
            contrast[2].1[2] > contrast[2].1[3],
            "upper-left glint is stronger than opposite edge"
        );
    }
    std::fs::write(
        directory.join("receipts.json"),
        serde_json::to_vec_pretty(&receipts).unwrap(),
    )
    .unwrap();
    println!(
        "HAL_GLASS_SIDE_REFLECTION Metal1x/2x actual Lab/oracle/spatial evidence {}",
        directory.display()
    );
}
#[cfg(target_os = "macos")]
#[test]
fn glass_material_lab_temporal_build_and_metal_1x_2x() {
    use crate::core::scene::{ClipNode, GradientRectNode};
    use crate::renderer::batcher::PrimitiveData;
    let samples = glass_material_lab_producer::tests::temporal_scenes();
    let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 1280.0, 820.0));
    let surface = compositor.create_surface(0, RectF::new(0.0, 0.0, 1280.0, 820.0), SceneKey::BASE);
    let scene_of = |b: RectF, gradients: Vec<GradientRectNode>| {
        let mut s = SceneGraph::new();
        s.clear_color = Color::TRANSPARENT;
        s.root.push(SceneNode::Clip(ClipNode {
            rect: b,
            rounded: None,
            children: gradients.into_iter().map(SceneNode::GradientRect).collect(),
            z_index: 0,
            key: None,
        }));
        s
    };
    let mut old_front = Vec::new();
    for (_, b, gradients, actual) in &samples {
        assert_eq!(
            gradients.iter().map(|g| g.z_index).collect::<Vec<_>>(),
            [2, 3]
        );
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, actual.clone())]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .unwrap();
        let data = &plan.surface_data[&surface];
        let mut batches = FrameBatches::new();
        batches.batches = data.batches.clone();
        batches.paths = data.path_draws.clone();
        let source_keys = data
            .primitives
            .iter()
            .filter(|p| {
                matches!(&p.data,
            PrimitiveData::GradientRect {rect,..} if rect.width==190.0 && rect.height==190.0)
            })
            .map(|p| p.key.clone())
            .collect::<std::collections::HashSet<_>>();
        let commands = build_surface_draw_commands(&batches, Vec::new());
        let gradient_z = commands
            .iter()
            .filter_map(|c| match c {
                SurfaceDrawCommand::Batch { batch_index, .. }
                    if source_keys.contains(&batches.batches[*batch_index].key) =>
                {
                    Some(c.z_index())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            gradient_z,
            [vec![2; 6], vec![3; 6]].concat(),
            "actual six-cell Build execution order"
        );
        let mut old = gradients.clone();
        old[1].z_index = 2;
        let old_plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene_of(*b, old))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .unwrap();
        let data = &old_plan.surface_data[&surface];
        let last = data.batches.last().unwrap();
        let front = data.primitives.iter().find(|p| p.key == last.key).unwrap();
        let PrimitiveData::GradientRect { params, .. } = &front.data else {
            panic!("gradient control");
        };
        old_front.push(params.stops[0].color[0] > params.stops[0].color[2]);
    }
    let reversals = old_front.windows(2).filter(|p| p[0] != p[1]).count();
    let mut gpu_cases = Vec::new();
    eprintln!(
        "HAL_S2B equal_z_control samples={} reversals={} historical_binary_attribution=false",
        samples.len(),
        reversals
    );
    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(720, 320, scale).expect("actual Metal required");
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        for (ms, b, gradients, _) in &samples {
            if ![16, 32, 48, 7440, 7456, 12000].contains(ms) {
                continue;
            }
            let actual = render_scene_at_scale(
                &mut renderer,
                &compositor,
                id,
                scene_of(*b, gradients.clone()),
                scale,
            );
            let reference = render_scene_at_scale(
                &mut renderer,
                &compositor,
                id,
                scene_of(*b, vec![gradients[1].clone()]),
                scale,
            );
            let x = ((gradients[0].rect.x + gradients[1].rect.x) * 0.5 + 95.0) as i32;
            let y = (gradients[0].rect.y + 95.0) as i32;
            let width = (720.0 * scale) as usize;
            let mut worst = 0_i16;
            for py in ((y - 5) as f32 * scale) as usize..((y + 5) as f32 * scale) as usize {
                for px in ((x - 5) as f32 * scale) as usize..((x + 5) as f32 * scale) as usize {
                    let offset = (py * width + px) * 4;
                    for c in 0..4 {
                        worst = worst.max((actual[offset+c] as i16-reference[offset+c] as i16).abs());
                        assert!(
                            (actual[offset + c] as i16 - reference[offset + c] as i16).abs() <= 2,
                            "pink foreground DPR={scale} ms={ms} pixel=({px},{py})"
                        );
                    }
                }
            }
            eprintln!("HAL_S2B Metal DPR={scale} ms={ms} pink_foreground=true tolerance_codes=2");
            gpu_cases.push(serde_json::json!({"dpr":scale,"millis":ms,"pink_foreground":true,
                "worst_error_codes":worst,"probe_logical":[x-5,y-5,10,10]}));
        }
    }
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let evidence = std::env::temp_dir().join(format!(
        "hal-s2b-metal-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&evidence)
        .unwrap();
    std::fs::set_permissions(&evidence,std::fs::Permissions::from_mode(0o700)).unwrap();
    eprintln!("HAL_S2B Metal evidence at {}", evidence.display());
    let output = evidence.join("proof.json");
    std::fs::write(&output,serde_json::to_vec_pretty(&serde_json::json!({
        "source":"actual Lab::build and common Timeline", "build_samples":samples.len(),
        "millis":samples.iter().map(|s|s.0).collect::<Vec<_>>(),
        "old_equal_z_pink_front":old_front,"old_equal_z_reversals":reversals,
        "historical_binary_attribution":false,"gpu_cases":gpu_cases,"tolerance_codes":2
    })).unwrap()).unwrap();
    std::fs::set_permissions(output,std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn registered_textblock_covering_face_reaches_real_metal_1x_2x() {
    use crate::core::paint::Color as PaintColor;
    use crate::core::text_engine::TextEngine;
    use crate::platform::font_manager::{FontAsset, FontManager};
    use crate::renderer::{TextRealizationFailureReason, TextRealizationOutcome};
    use crate::widgets::text_block::TextBlock;
    let mut manager = FontManager::new();
    let id = manager
        .load_font_file("/System/Library/Fonts/Supplemental/Arial Unicode.ttf")
        .expect("selected installed supplemental asset");
    let face = manager.get_face(id).unwrap();
    let actual = ttf_parser::Face::parse(&face.data, 0).unwrap();
    assert!(actual.glyph_index('\u{770b}').is_some_and(|g| g.0 != 0));
    let asset = FontAsset {
        family_name: face.family_name.clone(),
        weight: face.weight,
        italic: face.italic,
        data: face.data.clone(),
    };
    for scale in [1.0, 2.0] {
        for supplemental in [false, true] {
            for source in [
                "Latin Привет \u{770b} 龍 한글\nSecond line ffi e\u{301}",
                "אבג 看龍 דהו",
            ] {
                let (mut renderer, compositor, surface) =
                    setup_with_size_and_scale(360, 180, scale).expect("real Metal required");
                assert_eq!(
                    renderer.gpu_resources().adapter.get_info().backend,
                    wgpu::Backend::Metal
                );
                if supplemental {
                    renderer
                        .admit_font_assets(std::slice::from_ref(&asset))
                        .unwrap();
                }
                let engine = renderer.text_engine_mut();
                engine.set_scale(scale);
                let primary = engine
                    .shape("Latin", 18.0)
                    .binding
                    .unwrap()
                    .face()
                    .font_id();
                assert_ne!(primary, id, "supplemental asset must not replace SystemUI");
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(TextBlock::new(source).with_color(PaintColor::WHITE)),
                    None,
                    0,
                );
                let mut harness = RetainedSceneTestHarness::new();
                let frame = harness.frame(
                    &mut tree,
                    root,
                    crate::core::layout::Constraints::tight(Size::new(360.0, 180.0)),
                    &Theme::default(),
                    renderer.text_engine_mut(),
                    &IconRegistry::new(&[]),
                );
                let mut scene = SceneGraph::new();
                scene.root = frame.scene_root;
                scene.clear_color = Color::TRANSPARENT;
                let plan = DrawPlanBuilder::new()
                    .build_multi(
                        &HashMap::from([(surface, scene)]),
                        compositor.snapshot_multi(),
                        &mut MeshCache::new(),
                        |_| scale,
                        0,
                    )
                    .unwrap();
                let compiled = renderer
                    .compile_multi(&plan)
                    .unwrap()
                    .expect_ready("native Compile must be ready");
                let outcome = *compiled.text_realization(surface).unwrap().outcome();
                if supplemental {
                    assert!(
                        matches!(outcome, TextRealizationOutcome::Complete { .. }),
                        "covering registered face must survive TextBlock Layout/Paint/Compile: {outcome:?}"
                    );
                    let pixels = read_compiled_surface(&mut renderer, &compiled, surface)
                        .expect("complete native text output");
                    assert!(pixels.chunks_exact(4).filter(|p| p[3] > 0).count() > 100);
                } else {
                    assert!(
                        matches!(outcome,TextRealizationOutcome::Degraded {first_failure:f,missing_cluster_count,..} if f.reason==TextRealizationFailureReason::MissingGlyph && missing_cluster_count>0),
                        "uncovered source remains explicitly degraded: {outcome:?}"
                    );
                    assert!(!outcome.is_complete());
                    assert!(outcome.is_renderable());
                    let pixels = read_compiled_surface(&mut renderer, &compiled, surface)
                        .expect("same surface retains healthy text and markers");
                    assert!(pixels.chunks_exact(4).filter(|p| p[3] > 0).count() > 100);
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn analytic_surface_micrograin_metal_colour_clip_motion_text_1x_2x() {
    use crate::core::paint::{
        RoundedCorners, SurfaceAnalytic, SurfaceGradient, SurfaceMicrograin,
        analytic_surface_tests::fixture,
    };
    use crate::core::text_engine::TextEngine;
    use crate::renderer::pipelines::analytic_surface::tests::{
        decode, distance, encode, reference_at_scale,
    };
    let directory =
        std::path::PathBuf::from(format!("target/evidence/hal387/gpu-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let rect = RectF::new(18.0, 14.0, 112.0, 80.0);
    let corners = RoundedCorners {
        tl: 18.0,
        tr: 6.0,
        bl: 12.0,
        br: 0.0,
    };
    let grain = SurfaceMicrograin::try_new(1.5, 2.0 / 255.0, 387).unwrap();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(160, 120, scale).expect("HAL387 requires actual Metal");
        let info = renderer.gpu_resources().adapter.get_info();
        assert_eq!(info.backend, wgpu::Backend::Metal);
        assert_eq!(
            renderer.gpu_resources().format,
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
        println!(
            "HAL387 colour Metal={} format=Bgra8UnormSrgb DPR={scale} logical=160x120",
            info.name
        );
        let width = (160.0 * scale) as usize;
        let height = (120.0 * scale) as usize;
        let base = fixture(1.0, 0.2, 0.6);
        let mut cases = vec![
            base.clone(),
            base.clone().with_micrograin(Some(
                SurfaceMicrograin::try_new(3.0, 0.0, u32::MAX).unwrap(),
            )),
        ];
        for opacity in [0.0, 0.5, 1.0] {
            cases.push(fixture(opacity, 0.2, 0.6).with_micrograin(Some(grain)));
        }
        cases.push(fixture(1.0, 0.0, 0.6).with_micrograin(Some(grain)));
        cases.push(fixture(1.0, 0.2, -0.6).with_micrograin(Some(grain)));
        for rgb in [
            [0.0; 3],
            [1.0e-40; 3],
            [1.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ] {
            let mut def = base.base().definition().clone();
            for stop in &mut def.stops {
                stop.color = crate::core::paint::Color::from_linear(rgb[0], rgb[1], rgb[2], 1.0);
            }
            cases.push(
                SurfaceAnalytic::try_smoked_glass(
                    SurfaceGradient::new(def).unwrap(),
                    base.lights(),
                    base.parameters(),
                    base.rim(),
                )
                .unwrap()
                .with_micrograin(Some(grain)),
            );
        }
        let mut outputs = Vec::new();
        for (case, m) in cases.iter().enumerate() {
            let mut scene = SceneBuilder::new();
            scene
                .push_analytic_surface(rect, corners, m, 10, None)
                .unwrap();
            let mut scene = scene.build();
            scene.clear_color = Color::TRANSPARENT;
            let pixels = render_scene_at_scale(&mut renderer, &compositor, id, scene, scale);
            // WGSL f32 may flush subnormal operands/results to zero. The f64
            // oracle otherwise preserves a tiny positive S and adds grain that
            // the target correctly bypasses for computed Y == 0. Admit either
            // complete f32 result for this explicit subnormal input, keeping the
            // same two-code tolerance and the actual authored material intact.
            let flushed = (case == 8).then(|| cases[7].clone());
            for y in 0..height {
                for x in 0..width {
                    let p = Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                    let actual = &pixels[(y * width + x) * 4..][..4];
                    let lane = |dx: usize, dy: usize| {
                        distance(
                            rect,
                            corners,
                            Point::new(
                                (((x / 2) * 2 + dx) as f32 + 0.5) / scale,
                                (((y / 2) * 2 + dy) as f32 + 0.5) / scale,
                            ),
                        )
                        .0
                    };
                    let fine = (lane(1, y % 2) - lane(0, y % 2)).abs()
                        + (lane(x % 2, 1) - lane(x % 2, 0)).abs();
                    let coarse = (lane(1, 0) - lane(0, 0)).abs() + (lane(0, 1) - lane(0, 0)).abs();
                    let error = |material: &SurfaceAnalytic, derivative: f64| {
                        reference_at_scale(
                            material,
                            rect,
                            corners,
                            p,
                            derivative.max(0.5 / f64::from(scale)),
                            f64::from(scale),
                        )
                        .into_iter()
                        .enumerate()
                        .map(|(c, v)| {
                            (f64::from(actual[c]) - 255.0 * if c < 3 { encode(v) } else { v }).abs()
                        })
                        .fold(0.0_f64, f64::max)
                    };
                    let ordinary_error = error(m, fine).min(error(m, coarse));
                    let allowed_error = flushed.as_ref().map_or(ordinary_error, |material| {
                        ordinary_error
                            .min(error(material, fine))
                            .min(error(material, coarse))
                    });
                    assert!(
                        allowed_error <= 2.0,
                        "HAL387 scale={scale} case={case} pixel({x},{y}) actual={actual:?}"
                    );
                    for c in &actual[..3] {
                        assert!(
                            decode(f64::from(*c) / 255.0) <= f64::from(actual[3]) / 255.0 + 0.01
                        );
                    }
                }
            }
            std::fs::write(
                directory.join(format!("colour-{scale}x-{case}.rgba")),
                &pixels,
            )
            .unwrap();
            outputs.push(pixels);
        }
        assert!(
            outputs[0] == outputs[1],
            "absent/zero produces exactly the same raster"
        );
        assert!(outputs[0] != outputs[4], "permitted grain is present");
        assert!(
            outputs[0]
                .chunks_exact(4)
                .zip(outputs[4].chunks_exact(4))
                .all(|(a, b)| a[3] == b[3]),
            "grain never changes alpha or own AA"
        );
        assert!(outputs[4] != outputs[5], "rim remains independent");
        assert!(outputs[4] != outputs[6], "light reversal remains independent");

        let m = base.with_micrograin(Some(grain));
        let render = |renderer: &mut WgpuRenderer, r: RectF, text: bool, clip: bool| {
            let mut scene = SceneBuilder::new();
            if clip {
                scene.begin_clip(
                    RectF::new(12.0, 10.0, 120.0, 90.0),
                    Some(crate::core::scene::RoundedCorners::uniform(44.0)),
                );
                scene.begin_clip(RectF::new(14.0, 12.0, 100.0, 84.0), None);
            }
            scene
                .push_analytic_surface(r, corners, &m, 10, None)
                .unwrap();
            if text {
                let engine = renderer.text_engine_mut();
                engine.set_scale(scale);
                let run = engine.shape("Sharp text", 16.0);
                engine.draw(
                    &run,
                    30.0,
                    45.0,
                    crate::core::paint::Color::WHITE,
                    20,
                    &mut scene,
                );
            }
            if clip {
                scene.end_clip();
                scene.end_clip();
            }
            let mut scene = scene.build();
            scene.clear_color = Color::TRANSPARENT;
            render_scene_at_scale(renderer, &compositor, id, scene, scale)
        };
        let first = render(&mut renderer, rect, false, false);
        assert!(
            first == render(&mut renderer, rect, false, false),
            "repeat is bitwise stationary"
        );
        let mut scroll = SceneBuilder::new();
        scroll.begin_scroll((3.0 / scale, 2.0 / scale));
        scroll
            .push_analytic_surface(rect, corners, &m, 10, None)
            .unwrap();
        scroll.end_scroll();
        let mut scroll = scroll.build();
        scroll.clear_color = Color::TRANSPARENT;
        let scrolled = render_scene_at_scale(&mut renderer, &compositor, id, scroll, scale);
        let moved = render(
            &mut renderer,
            RectF::new(
                rect.x + 3.0 / scale,
                rect.y + 2.0 / scale,
                rect.width,
                rect.height,
            ),
            false,
            false,
        );
        for y in (32.0 * scale) as usize..(75.0 * scale) as usize {
            for x in (38.0 * scale) as usize..(115.0 * scale) as usize {
                assert_eq!(
                    &first[(y * width + x) * 4..][..4],
                    &moved[((y + 2) * width + x + 3) * 4..][..4],
                    "integer physical motion carries material texture"
                );
                assert_eq!(
                    &first[(y * width + x) * 4..][..4],
                    &scrolled[((y - 2) * width + x - 3) * 4..][..4],
                    "scroll projects the material origin once"
                );
            }
        }
        // Fractional translation must resample rather than copy screen noise.
        let fractional_rect = RectF::new(
            rect.x + 0.375 / scale,
            rect.y + 0.625 / scale,
            rect.width + 9.0,
            rect.height,
        );
        let fractional = render(&mut renderer, fractional_rect, false, false);
        for y in (35.0 * scale) as usize..(75.0 * scale) as usize {
            for x in (40.0 * scale) as usize..(110.0 * scale) as usize {
                let p = Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                let expected = reference_at_scale(
                    &m,
                    fractional_rect,
                    corners,
                    p,
                    0.5 / f64::from(scale),
                    f64::from(scale),
                );
                for c in 0..4 {
                    assert!(
                        (f64::from(fractional[(y * width + x) * 4 + c])
                            - 255.0
                                * if c < 3 {
                                    encode(expected[c])
                                } else {
                                    expected[c]
                                })
                        .abs()
                            <= 2.0
                    );
                }
            }
        }
        let clipped = render(&mut renderer, rect, false, true);
        for y in 0..height {
            for x in 0..width {
                let p = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                if p.0 < 14.0 || p.0 >= 114.0 || p.1 < 12.0 || p.1 >= 96.0 {
                    assert_eq!(&clipped[(y * width + x) * 4..][..4], [0, 0, 0, 0]);
                }
            }
        }
        assert!(
            clipped
                .chunks_exact(4)
                .zip(first.chunks_exact(4))
                .any(|(a, b)| a[3] < b[3]),
            "rounded ancestor remains effective below rectangular top clip"
        );
        let mask_point = ((20.0 * scale) as usize * width + (26.0 * scale) as usize) * 4 + 3;
        assert!(
            u16::from(clipped[mask_point]) + 100 < u16::from(first[mask_point]),
            "original rounded ancestor is not replaced by the inner rectangle"
        );
        let text_pixels = render(&mut renderer, rect, true, false);
        let mut text_only = SceneBuilder::new();
        let engine = renderer.text_engine_mut();
        engine.set_scale(scale);
        let run = engine.shape("Sharp text", 16.0);
        engine.draw(
            &run,
            30.0,
            45.0,
            crate::core::paint::Color::WHITE,
            20,
            &mut text_only,
        );
        let mut text_only = text_only.build();
        text_only.clear_color = Color::TRANSPARENT;
        let text_only = render_scene_at_scale(&mut renderer, &compositor, id, text_only, scale);
        let mut sharp = 0;
        for (glyph, result) in text_only.chunks_exact(4).zip(text_pixels.chunks_exact(4)) {
            if glyph == [255, 255, 255, 255] {
                assert_eq!(result, glyph, "opaque foreground glyph cores remain sharp");
                sharp += 1;
            }
        }
        assert!(sharp > 10, "actual foreground glyph cores were realized");
        std::fs::write(
            directory.join(format!("text-clip-{scale}x.rgba")),
            text_pixels,
        )
        .unwrap();
        println!(
            "HAL387 colour/motion/clip/text evidence {} DPR={scale} opaque_glyph_pixels={sharp}",
            directory.display()
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn analytic_surface_micrograin_metal_spatial_fields_1x_2x() {
    use crate::core::paint::{
        RoundedCorners, SurfaceAnalytic, SurfaceGradient, SurfaceMicrograin,
        analytic_surface_tests::fixture,
    };
    use crate::renderer::pipelines::analytic_surface::tests::{
        assert_spatial_statistics, decode, micrograin_field, spatial_statistics,
    };
    let directory =
        std::path::PathBuf::from(format!("target/evidence/hal387/gpu-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut receipts = Vec::new();
    let mut previous = HashMap::new();
    for dpr in [1.0_f32, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale((520.0 / dpr) as u32, (520.0 / dpr) as u32, dpr)
                .expect("HAL387 requires actual Metal");
        let info = renderer.gpu_resources().adapter.get_info();
        assert_eq!(info.backend, wgpu::Backend::Metal);
        assert_eq!(
            renderer.gpu_resources().format,
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let original = fixture(1.0, 0.0, 0.6);
        let mut def = original.base().definition().clone();
        for stop in &mut def.stops {
            let c = decode(0.5) as f32;
            stop.color = crate::core::paint::Color::from_linear(c, c, c, 1.0);
        }
        let mut p = original.parameters();
        p.ambient_strength = 0.0;
        p.diffuse_strength = 0.0;
        p.specular_strength = 0.0;
        p.bevel_width = 0.0;
        p.bevel_slope = 0.0;
        let smooth = SurfaceAnalytic::try_smoked_glass(
            SurfaceGradient::new(def).unwrap(),
            original.lights(),
            p,
            None,
        )
        .unwrap();
        for seed in [387, 305419896] {
            for scale in [0.75, 1.5, 3.0] {
                let grain = SurfaceMicrograin::try_new(scale, 2.0 / 255.0, seed).unwrap();
                let mut scene = SceneBuilder::new();
                scene
                    .push_analytic_surface(
                        RectF::new(0.0, 0.0, 520.0 / dpr, 520.0 / dpr),
                        RoundedCorners::uniform(0.0),
                        &smooth.clone().with_micrograin(Some(grain)),
                        10,
                        None,
                    )
                    .unwrap();
                let pixels =
                    render_scene_at_scale(&mut renderer, &compositor, id, scene.build(), dpr);
                let mut field = Vec::with_capacity(512 * 512);
                let mut crop = Vec::with_capacity(512 * 512 * 3);
                for y in 4..516 {
                    for x in 4..516 {
                        let rgba = &pixels[(y * 520 + x) * 4..][..4];
                        assert_eq!(rgba[3], 255);
                        assert_eq!(rgba[0], rgba[1]);
                        assert_eq!(rgba[1], rgba[2]);
                        let n = micrograin_field([x as f64 + 0.5, y as f64 + 0.5], grain);
                        let expected = 127.5 + n * f64::from(grain.amplitude_srgb()) * 255.0;
                        assert!(
                            (f64::from(rgba[0]) - expected).abs() <= 1.0,
                            "GPU footprint seed={seed} scale={scale} ({x},{y})"
                        );
                        field.push((f64::from(rgba[0]) - 127.5) / 2.0);
                        crop.extend_from_slice(&rgba[..3]);
                    }
                }
                let stats = spatial_statistics(&field, 512);
                assert_spatial_statistics(stats);
                let name = format!("field-{dpr}x-seed{seed}-scale{scale}");
                let mut ppm = b"P6\n512 512\n255\n".to_vec();
                ppm.extend_from_slice(&crop);
                std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
                std::fs::write(directory.join(format!("{name}.rgba")), &pixels).unwrap();
                if dpr == 1.0 {
                    previous.insert((seed, (scale * 100.0) as u32), crop);
                } else {
                    assert_eq!(
                        previous[&(seed, (scale * 100.0) as u32)],
                        crop,
                        "same physical local grid at DPR1/2; no UV stretch"
                    );
                }
                println!(
                    "HAL387 GPU Metal={} format=Bgra8UnormSrgb DPR={dpr} seed={seed} scale={scale} stats={stats:?}",
                    info.name
                );
                receipts.push(serde_json::json!({"adapter":info.name,"backend":"Metal","format":"Bgra8UnormSrgb","dpr":dpr,"physical":[520,520],"crop":[4,4,512,512],"seed":seed,"scale":scale,"stats":stats}));
            }
        }
    }
    std::fs::write(
        directory.join("spatial.json"),
        serde_json::to_vec_pretty(&receipts).unwrap(),
    )
    .unwrap();
    println!("HAL387 GPU spatial evidence {}", directory.display());
}

#[cfg(target_os = "macos")]
#[test]
fn analytic_surface_metal_reference_rim_alpha_clip_z_1x_2x() {
    use crate::core::paint::{
        AnalyticSurfaceLight, RoundedCorners as Corners, SurfaceAnalytic, SurfaceGradient,
        analytic_surface_tests::fixture,
    };
    use crate::renderer::pipelines::analytic_surface::tests::{distance, reference_at_scale};
    let rect = RectF::new(18.0, 14.0, 112.0, 80.0);
    let corners = Corners {
        tl: 18.0,
        tr: 6.0,
        bl: 12.0,
        br: 0.0,
    };
    let lit = fixture(1.0, 0.0, 0.6);
    let mut p = lit.parameters();
    p.ambient_strength = 0.0;
    p.diffuse_strength = 0.0;
    p.specular_strength = 0.0;
    let unlit =
        SurfaceAnalytic::try_smoked_glass(lit.base().clone(), lit.lights(), p, None).unwrap();
    let mut cases = vec![
        unlit,
        lit.clone(),
        fixture(1.0, 0.2, 0.6),
        fixture(0.5, 0.2, 0.6),
        fixture(1.0, 0.2, -0.6),
        fixture(0.0, 0.2, 0.6),
    ];
    for color in [0.0, 1.0, 1.0e-40] {
        let mut base = lit.base().definition().clone();
        for stop in &mut base.stops {
            stop.color = crate::core::paint::Color::from_linear(color, color, color, 1.0);
        }
        cases.push(
            SurfaceAnalytic::try_smoked_glass(
                SurfaceGradient::new(base).unwrap(),
                lit.lights(),
                lit.parameters(),
                None,
            )
            .unwrap(),
        );
    }
    let zero_lights = lit.lights().map(|l| {
        AnalyticSurfaceLight::try_new(
            l.direction(),
            l.color(),
            0.0,
            l.center_uv(),
            l.radii_uv(),
            l.rotation_cos_sin(),
        )
        .unwrap()
    });
    cases.push(
        SurfaceAnalytic::try_smoked_glass(
            lit.base().clone(),
            zero_lights,
            lit.parameters(),
            fixture(1.0, 0.2, 0.6).rim(),
        )
        .unwrap(),
    );
    let mut flat = lit.parameters();
    flat.bevel_width = 0.0;
    flat.bevel_slope = 0.0;
    cases.push(
        SurfaceAnalytic::try_smoked_glass(lit.base().clone(), lit.lights(), flat, None).unwrap(),
    );
    use crate::core::paint::analytic_surface_tests::{coating,emission};
    cases.extend([
        lit.clone().with_glass_optics(Some(coating(0.6))),
        lit.clone().with_glass_optics(Some(coating(-0.6))),
        fixture(0.5,0.0,0.6).with_glass_optics(Some(coating(0.6))),
        fixture(0.0,0.0,0.6).with_surface_emission(Some(emission())),
        lit.clone().with_glass_optics(Some(coating(0.6))).with_surface_emission(Some(emission())),
        lit.clone().with_micrograin(Some(crate::core::paint::SurfaceMicrograin::try_new(1.5,8.0/255.0,387).unwrap())),
    ]);
    let directory = std::path::PathBuf::from(format!(
        "target/evidence/hal386389-s1-20261113/gpu-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(160, 120, scale).expect("HAL386 requires actual Metal");
        let gpu = renderer.gpu_resources();
        let info = gpu.adapter.get_info();
        let srgb = gpu.format.is_srgb();
        assert_eq!(info.backend, wgpu::Backend::Metal);
        println!(
            "HAL386 GPU {:?} {} scale={scale} format={:?}; encoded RGBA8 oracle tolerance=2 codes; subnormal values quantized to this target",
            info.backend, info.name, gpu.format
        );
        let width = (160.0 * scale) as usize;
        let height = (120.0 * scale) as usize;
        let mut outputs = Vec::new();
        for (case, m) in cases.iter().enumerate() {
            let mut scene = SceneBuilder::new();
            scene
                .push_analytic_surface(rect, corners, m, 10, None)
                .unwrap();
            let mut scene = scene.build();
            scene.clear_color = Color::TRANSPARENT;
            let pixels = render_scene_at_scale(&mut renderer, &compositor, id, scene, scale);
            let mut compared = 0;
            for y in 0..height {
                for x in 0..width {
                    let point = Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                    let (d, _) = distance(rect, corners, point);
                    let actual = &pixels[(y * width + x) * 4..][..4];
                    // Evaluate actual 2x2 helper-lane SDF differences, including
                    // the max-axis ridge at a square corner. WGSL fwidth admits
                    // fine/coarse derivatives; either complete RGBA oracle must
                    // match, with the same two-code format tolerance, everywhere.
                    let lane = |dx: usize, dy: usize| {
                        distance(
                            rect,
                            corners,
                            Point::new(
                                (((x / 2) * 2 + dx) as f32 + 0.5) / scale,
                                (((y / 2) * 2 + dy) as f32 + 0.5) / scale,
                            ),
                        )
                        .0
                    };
                    let fine = (lane(1, y % 2) - lane(0, y % 2)).abs()
                        + (lane(x % 2, 1) - lane(x % 2, 0)).abs();
                    let coarse = (lane(1, 0) - lane(0, 0)).abs() + (lane(0, 1) - lane(0, 0)).abs();
                    let error = |derivative: f64| {
                        let expected = reference_at_scale(
                            m,
                            rect,
                            corners,
                            point,
                            derivative.max(0.5 / f64::from(scale)),
                            f64::from(scale),
                        );
                        expected
                            .into_iter()
                            .enumerate()
                            .map(|(channel, value)| {
                                let encoded = if srgb && channel < 3 {
                                    if value <= 0.0031308 {
                                        12.92 * value
                                    } else {
                                        1.055 * value.powf(1.0 / 2.4) - 0.055
                                    }
                                } else {
                                    value
                                };
                                (f64::from(actual[channel]) - 255.0 * encoded).abs()
                            })
                            .fold(0.0_f64, f64::max)
                    };
                    assert!(
                        error(fine).min(error(coarse)) <= 2.0,
                        "scale={scale} case={case} pixel({x},{y}) actual={actual:?} d={d} fine={fine} coarse={coarse}"
                    );
                    compared += 1;
                    if d > f64::from(m.halo_extent()) + 2.0 / f64::from(scale)
                        || (m.parameters().opacity == 0.0 && m.surface_emission().is_none()) {
                        assert_eq!(actual, [0, 0, 0, 0]);
                    }
                    let alpha = f64::from(actual[3]) / 255.0;
                    for c in &actual[..3] {
                        let v = f64::from(*c) / 255.0;
                        let linear = if srgb {
                            if v <= 0.04045 {
                                v / 12.92
                            } else {
                                ((v + 0.055) / 1.055).powf(2.4)
                            }
                        } else {
                            v
                        };
                        assert!(
                            linear <= alpha + 0.01,
                            "premultiplied bound including RGBA8 quantization"
                        );
                    }
                }
            }
            assert_eq!(compared, width * height);
            std::fs::write(
                directory.join(format!("analytic-{scale}x-case{case}.rgba")),
                &pixels,
            )
            .unwrap();
            outputs.push(pixels);
        }
        assert_ne!(
            outputs[0], outputs[1],
            "actual nonzero face reflection differs from plain mathematical Gradient"
        );
        assert_ne!(
            outputs[1], outputs[2],
            "independent reflective rim must be observable"
        );
        assert_ne!(
            outputs[2], outputs[4],
            "light reversal changes actual GPU output"
        );
        let center = ((48.0 * scale) as usize * width + (64.0 * scale) as usize) * 4;
        assert_eq!(
            &outputs[1][center..center + 4],
            &outputs[2][center..center + 4],
            "rim-off preserves fully evaluated interior S"
        );
        // The same original rounded ancestor survives a nested rectangular scissor
        // and a single scroll translation. No diagnostic flag can disable this mask.
        renderer.set_gpu_rounded_clips_enabled(false);
        let mut scene = SceneBuilder::new();
        scene.begin_clip(
            RectF::new(12.0, 10.0, 120.0, 90.0),
            Some(crate::core::scene::RoundedCorners {
                tl: 20.0,
                tr: 6.0,
                bl: 14.0,
                br: 0.0,
            }),
        );
        scene.begin_scroll((7.0, 9.0));
        scene.begin_clip(RectF::new(21.0, 21.0, 100.0, 84.0), None);
        scene
            .push_analytic_surface(
                RectF::new(7.0, 9.0, 150.0, 110.0),
                Corners::uniform(0.0),
                &cases[2],
                10,
                None,
            )
            .unwrap();
        scene.push_rect(
            RectF::new(57.0, 47.0, 24.0, 20.0),
            Color::from_hex(0x33dd77),
            20,
            None,
        );
        scene.end_clip();
        scene.end_scroll();
        scene.end_clip();
        let mut scene = scene.build();
        scene.clear_color = Color::TRANSPARENT;
        let pixels = render_scene_at_scale(&mut renderer, &compositor, id, scene, scale);
        for y in 0..height {
            for x in 0..width {
                let px = (x as f32 + 0.5) / scale;
                let py = (y as f32 + 0.5) / scale;
                if px < 14.0 || px >= 114.0 || py < 12.0 || py >= 96.0 {
                    assert_eq!(
                        &pixels[(y * width + x) * 4..][..4],
                        [0, 0, 0, 0],
                        "inherited rectangular intersection"
                    );
                }
            }
        }
        let outside = ((13.0 * scale) as usize * width + (15.0 * scale) as usize) * 4;
        assert_eq!(
            &pixels[outside..outside + 4],
            &[0, 0, 0, 0],
            "original rounded mask is not erased by inner rect or diagnostic flag"
        );
        let edge_x = (16.0 * scale) as usize;
        let edge_y = (16.0 * scale) as usize;
        let point = (
            (edge_x as f64 + 0.5) / f64::from(scale),
            (edge_y as f64 + 0.5) / f64::from(scale),
        );
        let distance = (point.0 - 32.0).hypot(point.1 - 30.0);
        let u = (distance - 20.0).clamp(0.0, 1.0);
        let expected = 255.0 * (1.0 - u * u * (3.0 - 2.0 * u));
        let alpha = pixels[(edge_y * width + edge_x) * 4 + 3];
        assert!(
            (f64::from(alpha) - expected).abs() <= 2.0,
            "inherited rounded AA uses original geometry exactly once: {alpha} vs {expected}"
        );
        let at = ((45.0 * scale) as usize * width + (60.0 * scale) as usize) * 4;
        if srgb {
            assert_eq!(
                &pixels[at..at + 4],
                &[0x33, 0xdd, 0x77, 255],
                "strict Z: later Rect covers analytic face"
            );
        }
        std::fs::write(
            directory.join(format!("analytic-{scale}x-clip-z.rgba")),
            pixels,
        )
        .unwrap();
        println!("HAL386 GPU evidence {} scale={scale}", directory.display());
    }
}

#[cfg(target_os = "macos")]
#[test]
fn analytic_surface_parameter_slots_two_surfaces_metal() {
    use crate::core::paint::{
        RoundedCorners, SurfaceAnalytic, SurfaceGradient, analytic_surface_tests::fixture,
    };
    let (mut renderer, mut compositor, first) =
        setup_with_size_and_scale(96, 64, 1.0).expect("real Metal");
    let second = compositor.create_surface(1, RectF::new(0.0, 0.0, 96.0, 64.0), SceneKey::BASE);
    renderer
        .register_headless_render_target(second, (192, 128), 2.0)
        .unwrap();
    let a = fixture(1.0, 0.0, 0.6);
    let mut base = a.base().definition().clone();
    for stop in &mut base.stops {
        stop.color = crate::core::paint::Color::from_linear(0.22, 0.025, 0.01, 1.0);
    }
    let b = SurfaceAnalytic::try_smoked_glass(
        SurfaceGradient::new(base).unwrap(),
        a.lights(),
        a.parameters(),
        None,
    )
    .unwrap();
    let make = |m: &SurfaceAnalytic| {
        let mut scene = SceneBuilder::new();
        scene
            .push_analytic_surface(
                RectF::new(4.0, 4.0, 88.0, 56.0),
                RoundedCorners::uniform(8.0),
                m,
                10,
                None,
            )
            .unwrap();
        let mut scene = scene.build();
        scene.clear_color = Color::TRANSPARENT;
        scene
    };
    let scenes = HashMap::from([(first, make(&a)), (second, make(&b))]);
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |id| if id == first { 1.0 } else { 2.0 },
            0,
        )
        .unwrap();
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    let offset_a = compiled.surface_data[&first].analytic_surface_params_offsets[&1];
    let offset_b = compiled.surface_data[&second].analytic_surface_params_offsets[&1];
    assert_ne!(
        offset_a, offset_b,
        "equal ordinal1 on different actual surfaces must not alias"
    );
    let pa = read_compiled_surface(&renderer, &compiled, first).unwrap();
    let pb = read_compiled_surface(&renderer, &compiled, second).unwrap();
    let ca = &pa[(32 * 96 + 48) * 4..][..4];
    let cb = &pb[(64 * 192 + 96) * 4..][..4];
    assert_ne!(ca, cb);
    assert!(cb[0] > ca[0]);
    assert_eq!(ca[3], 255);
    assert_eq!(cb[3], 255);
    println!(
        "HAL386 two-surface slots {first:?}/ordinal1={offset_a}, {second:?}/ordinal1={offset_b}; 1x={ca:?} 2x={cb:?}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn local_ambient_glow_gpu_reference_clip_z_and_1x_2x() {
    use crate::core::paint::{
        Color as PaintColor, LocalAmbientGlowAppearance, LocalGlowLobe,
        local_ambient_glow_reference,
    };
    let a = LocalGlowLobe::try_new(
        Point::new(0.3, 0.5),
        Point::new(40.0, 30.0),
        Point::new(0.8, 0.6),
        PaintColor::from_linear(0.8, 0.15, 0.03, 0.6),
        0.8,
    )
    .unwrap();
    let b = LocalGlowLobe::try_new(
        Point::new(0.7, 0.6),
        Point::new(44.0, 26.0),
        Point::new(0.6, -0.8),
        PaintColor::from_linear(0.04, 0.3, 0.9, 0.75),
        0.9,
    )
    .unwrap();
    let cases = [
        LocalAmbientGlowAppearance::try_new(&[]).unwrap(),
        LocalAmbientGlowAppearance::try_new(&[a]).unwrap(),
        LocalAmbientGlowAppearance::try_new(&[a, b]).unwrap(),
        LocalAmbientGlowAppearance::try_new(&[b, a]).unwrap(),
    ];
    let rect = RectF::new(8.0, 8.0, 90.0, 70.0);
    let moved = RectF::new(1.0, -1.0, 90.0, 70.0);
    let directory = std::path::Path::new("target/evidence/hal388-s1a-20261113/gpu");
    std::fs::create_dir_all(directory).unwrap();
    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, id) = setup_with_size_and_scale(160, 120, scale)
            .expect("HAL388 requires real Metal readback");
        let gpu = renderer.gpu_resources();
        let info = gpu.adapter.get_info();
        assert_eq!(info.backend, wgpu::Backend::Metal);
        println!(
            "HAL388 actual GPU {:?} {} scale={scale} format={:?}",
            info.backend, info.name, gpu.format
        );
        let srgb = gpu.format.is_srgb();
        let mut outputs = Vec::new();
        for (index, appearance) in cases.iter().copied().enumerate() {
            let mut scene = SceneBuilder::new();
            scene.begin_clip(RectF::new(20.0, 20.0, 100.0, 70.0), None);
            scene.begin_scroll((7.0, 9.0));
            scene
                .push_local_ambient_glow(rect, appearance, 10, None)
                .unwrap();
            scene.end_scroll();
            scene.end_clip();
            let mut scene = scene.build();
            scene.clear_color = Color::TRANSPARENT;
            let rgba = render_scene_at_scale(&mut renderer, &compositor, id, scene, scale);
            let width = (160.0 * scale) as usize;
            let height = (120.0 * scale) as usize;
            for y in 0..height {
                for x in 0..width {
                    let p = Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                    let expected = if p.x >= 20.0 && p.x < 120.0 && p.y >= 20.0 && p.y < 90.0 {
                        local_ambient_glow_reference(appearance, moved, p)
                    } else {
                        [0.0; 4]
                    };
                    let pixel = &rgba[(y * width + x) * 4..][..4];
                    for channel in 0..4 {
                        let linear = expected[channel];
                        let encoded = if srgb && channel < 3 {
                            if linear <= 0.0031308 {
                                linear * 12.92
                            } else {
                                1.055 * linear.powf(1.0 / 2.4) - 0.055
                            }
                        } else {
                            linear
                        };
                        assert!(
                            (f32::from(pixel[channel]) - encoded * 255.0).abs() <= 2.0,
                            "scale={scale} case={index} ({x},{y}) channel={channel} actual={pixel:?} reference={expected:?}"
                        );
                    }
                    if expected == [0.0; 4] {
                        assert_eq!(pixel, [0, 0, 0, 0]);
                    }
                }
            }
            std::fs::write(directory.join(format!("glow-{scale}x-{index}.rgba")), &rgba).unwrap();
            outputs.push(rgba);
        }
        assert_eq!(
            outputs[2], outputs[3],
            "two-lobe order must be commutative on actual GPU"
        );
        let mut scene = SceneBuilder::new();
        let face = RectF::new(45.0, 35.0, 40.0, 28.0);
        scene.push_rect(face, Color::from_hex(0x121822), 20, None);
        scene
            .push_local_ambient_glow(rect, cases[2], 10, None)
            .unwrap();
        let mut scene = scene.build();
        scene.clear_color = Color::TRANSPARENT;
        let pixels = render_scene_at_scale(&mut renderer, &compositor, id, scene, scale);
        let center =
            (((48.0 * scale) as usize) * (160.0 * scale) as usize + (60.0 * scale) as usize) * 4;
        let expected = if srgb {
            [0x12, 0x18, 0x22, 255]
        } else {
            let c = Color::from_hex(0x121822);
            [
                (c.r * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.b * 255.0).round() as u8,
                255,
            ]
        };
        for (actual, expected) in pixels[center..center + 4].iter().zip(expected) {
            assert!((i16::from(*actual) - i16::from(expected)).abs() <= 1);
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn ui_typography_gpu_realizes_two_faces_at_logical_scales() {
    use super::retained_value_updates::{
        UiTypographyFixture, ui_typography_font_assets, ui_typography_texts,
    };
    use crate::core::paragraph::TextStyle;
    use crate::core::text_engine::TextEngine;
    use crate::platform::font_manager::FontWeight;
    for (width, height) in [(720, 480), (480, 320)] {
        for scale in [1.0, 1.25, 2.0] {
            let (mut renderer, compositor, surface) =
                setup_with_size_and_scale(width, height, scale)
                    .expect("ui_typography_ requires a real adapter and readback");
            let assets = ui_typography_font_assets();
            renderer.admit_font_assets(&assets).unwrap();
            renderer.text_engine_mut().set_scale(scale);
            let style = TextStyle::default()
                .with_family(assets[0].family_name.clone())
                .with_size(22.0)
                .with_cluster_gap(2.0)
                .unwrap();
            let mut fixture = UiTypographyFixture::new(
                style.clone(),
                style.clone().with_weight(FontWeight::Bold),
            );
            let frame = fixture.frame(
                renderer.text_engine_mut(),
                crate::core::math::Size::new(width as f32, height as f32),
            );
            assert!(frame.scene_built);
            let texts = ui_typography_texts(&frame.scene_root);
            let regular = texts
                .iter()
                .find(|text| text.text.starts_with("AV office Привет e"))
                .unwrap();
            let bold = texts
                .iter()
                .find(|text| text.text == "AV office Привет")
                .unwrap();
            let regular_binding = regular.binding.as_ref().unwrap();
            let bold_binding = bold.binding.as_ref().unwrap();
            assert_ne!(
                regular_binding.face().font_id(),
                bold_binding.face().font_id()
            );
            assert_ne!(regular_binding.face().data(), bold_binding.face().data());
            assert_eq!(regular_binding.face().weight(), FontWeight::Regular);
            assert_eq!(bold_binding.face().weight(), FontWeight::Bold);
            let mut scenes = HashMap::new();
            scenes.insert(surface, control_order_scene(&frame));
            let plan = DrawPlanBuilder::new()
                .build_multi(
                    &scenes,
                    compositor.snapshot_multi(),
                    &mut MeshCache::new(),
                    |_| scale,
                    0,
                )
                .expect("scene must satisfy DrawPlan Build admission");
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("typography compile must be renderable");
            assert!(
                compiled
                    .text_realization(surface)
                    .unwrap()
                    .outcome()
                    .is_complete()
            );
            let pixels = read_compiled_surface(&renderer, &compiled, surface)
                .expect("typography encode/readback required");
            let physical_width = (width as f32 * scale) as usize;
            for text in texts.iter().filter(|text| text.binding.is_some()) {
                let run = renderer
                    .text_engine_mut()
                    .shape_bound_with_keys(
                        &text.text,
                        text.font_size,
                        text.binding.as_ref().unwrap(),
                    )
                    .unwrap()
                    .0;
                let x0 = (text.position.x * scale).floor().max(0.0) as usize;
                let x1 = ((text.position.x + run.width) * scale)
                    .ceil()
                    .min(width as f32 * scale) as usize;
                let y0 = (text.position.y * scale).floor().max(0.0) as usize;
                let y1 = ((text.position.y + run.height) * scale)
                    .ceil()
                    .min(height as f32 * scale) as usize;
                let ink = (y0..y1)
                    .flat_map(|y| (x0..x1).map(move |x| (y * physical_width + x) * 4))
                    .filter(|&offset| {
                        pixels[offset..offset + 3]
                            .iter()
                            .any(|&channel| channel > 180)
                    })
                    .count();
                assert!(
                    ink > 20,
                    "actual face text must have visible readback ink: {} at {}x{} DPR{}",
                    text.text,
                    width,
                    height,
                    scale
                );
            }
            let fresh = UiTypographyFixture::new(
                style.clone(),
                style.clone().with_weight(FontWeight::Bold),
            )
            .frame(
                renderer.text_engine_mut(),
                crate::core::math::Size::new(width as f32, height as f32),
            );
            let mut fresh_scenes = HashMap::new();
            fresh_scenes.insert(surface, control_order_scene(&fresh));
            let fresh_plan = DrawPlanBuilder::new()
                .build_multi(
                    &fresh_scenes,
                    compositor.snapshot_multi(),
                    &mut MeshCache::new(),
                    |_| scale,
                    1,
                )
                .expect("scene must satisfy DrawPlan Build admission");
            let fresh_compiled = renderer
                .compile_multi(&fresh_plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            assert_eq!(
                pixels,
                read_compiled_surface(&renderer, &fresh_compiled, surface).unwrap()
            );
        }
    }
}

fn setup() -> Option<(WgpuRenderer, Compositor, SurfaceId)> {
    setup_with_size(WIDTH, HEIGHT)
}

fn setup_with_size(width: u32, height: u32) -> Option<(WgpuRenderer, Compositor, SurfaceId)> {
    setup_with_size_and_scale(width, height, 1.0)
}

pub(super) fn setup_with_size_and_scale(
    width: u32,
    height: u32,
    scale_factor: f32,
) -> Option<(WgpuRenderer, Compositor, SurfaceId)> {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())?;
    let viewport = RectF::new(0.0, 0.0, width as f32, height as f32);
    let mut compositor = Compositor::new(viewport);
    let surface_id = compositor.create_surface(0, viewport, SceneKey::BASE);
    renderer
        .register_headless_render_target(
            surface_id,
            (
                (width as f32 * scale_factor) as u32,
                (height as f32 * scale_factor) as u32,
            ),
            scale_factor,
        )
        .expect("register SPEC-HAL-359 target");
    Some((renderer, compositor, surface_id))
}

fn control_order_scene(frame: &crate::core::app::RetainedSceneTestFrame) -> SceneGraph {
    let mut builder = SceneBuilder::new();
    builder.push_retained_fragment(frame.scene_root.clone().into());
    builder.build()
}

fn render_control_order_frame(
    renderer: &mut WgpuRenderer,
    compositor: &Compositor,
    surface: SurfaceId,
    frame: &crate::core::app::RetainedSceneTestFrame,
    host: crate::utils::id::TreeWidgetId,
    binding: Option<&crate::renderer::image_binding::ImageBinding>,
) -> Vec<u8> {
    let scenes = HashMap::from([(surface, control_order_scene(frame))]);
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    let source = &plan.surface_data[&surface];
    assert!(source.primitives.iter().any(|primitive| matches!(&primitive.data,
        crate::renderer::batcher::PrimitiveData::Text { backgrounds, .. } if !backgrounds.is_empty())));
    if let Some(binding) = binding {
        let images: Vec<_> = source
            .primitives
            .iter()
            .filter(|primitive| {
                matches!(
                    primitive.data,
                    crate::renderer::batcher::PrimitiveData::Image { .. }
                )
            })
            .collect();
        assert_eq!(images.len(), 3);
        assert!(images.iter().all(|primitive| primitive.key.texture_id
            == Some(binding.identity().texture_id())
            && primitive.key.image_generation == Some(binding.identity().generation())));
    }
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("control order compiled target");
    let data = compiled.surface_compiled_data(surface).unwrap();
    assert!(!matches!(
        data.text_realization().outcome(),
        crate::renderer::TextRealizationOutcome::Unavailable(_)
    ));
    assert!(
        data.draw_commands
            .windows(2)
            .all(|commands| commands[0].order() <= commands[1].order())
    );
    let mut intervals = Vec::new();
    for command in &data.draw_commands {
        if let Some(member) = command
            .order()
            .members()
            .iter()
            .find(|member| member.scope == host)
        {
            if intervals.last() != Some(&member.member_index) {
                intervals.push(member.member_index);
            }
        } else if command.z_index() >= LAYER_OVERLAY {
            assert!(
                command.order().members().is_empty(),
                "original overlay class ignores main prefixes"
            );
        }
    }
    assert_eq!(
        intervals,
        vec![0, 1, 2],
        "closed member intervals survive mixed-family compilation"
    );
    assert!(
        data.draw_commands
            .iter()
            .any(|command| matches!(command, SurfaceDrawCommand::Path { .. }))
    );
    let mut saw_background_run = false;
    for command in &data.draw_commands {
        if let SurfaceDrawCommand::Text {
            quad_start,
            quad_count,
            ..
        } = command
        {
            let quads =
                &data.text_quads[*quad_start as usize..(*quad_start + *quad_count) as usize];
            let backgrounds = quads
                .iter()
                .take_while(|quad| quad.downsample_u < 0.0)
                .count();
            if backgrounds > 0 {
                saw_background_run = true;
                assert!(backgrounds < quads.len());
                assert!(
                    quads[backgrounds..]
                        .iter()
                        .all(|quad| quad.downsample_u >= 0.0)
                );
            }
        }
    }
    assert!(
        saw_background_run,
        "real Label selection background and glyphs stay one contiguous run"
    );
    let mut fill_members = Vec::new();
    for batch in &data.frame_batches.batches {
        if batch.key.pipeline == crate::renderer::batcher::PipelineId::Rect {
            if let Some(member) = batch
                .key
                .control_subtree_order
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .find(|member| member.scope == host)
            {
                if !fill_members.contains(&member.member_index) {
                    fill_members.push(member.member_index);
                }
            }
        }
    }
    assert!(
        fill_members.contains(&0) && fill_members.contains(&1),
        "same-pipeline/material fills cannot merge across member/text/path intervals"
    );
    read_compiled_surface(renderer, &compiled, surface).expect("actual control-order GPU readback")
}

#[test]
fn control_subtree_order_gpu_covers_earlier_text_and_focus() {
    use super::retained_value_updates::{ControlOrderFixture, control_order_member_fragments};
    let (mut renderer, compositor, surface) =
        setup_with_size(320, 220).expect("control-subtree GPU proof requires adapter");
    renderer.register_image_rgba8_srgb(359_206, 1, 1, &[255, 255, 255, 255]);
    let binding = renderer.admit_image(359_206).unwrap();
    let mut f = ControlOrderFixture::new(Some(&binding));
    assert!(f.frame().scene_built);
    f.select_label();
    let before = f.frame();
    assert!(before.scene_built);
    assert_eq!(f.image_dependency_counts(), Some((3, 1)));
    let host = f.tree.widget_id_for_node(f.host).unwrap();
    let members = control_order_member_fragments(&before.scene_root, host);
    let pixels = render_control_order_frame(
        &mut renderer,
        &compositor,
        surface,
        &before,
        host,
        Some(&binding),
    );
    let mut reference = SceneBuilder::new();
    reference.begin_clip(RectF::new(8.0, 8.0, 240.0, 168.0), None);
    reference.push_retained_fragment(members[1].clone());
    reference.end_clip();
    let root = f.tree.widget_id_for_node(f.root).unwrap();
    let root_members = control_order_member_fragments(&before.scene_root, root);
    reference.push_retained_fragment(root_members[2].clone());
    let reference = render_scene(&mut renderer, &compositor, surface, reference.build());
    for y in 44..116 {
        for x in 20..188 {
            assert_eq!(
                pixel_at(&pixels, 320, x, y),
                pixel_at(&reference, 320, x, y),
                "later real Button covers earlier text/focus at ({x},{y})"
            );
        }
    }
    let white_before = (32..128)
        .flat_map(|y| (8..208).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel_at(&pixels, 320, x, y) == [255, 255, 255, 255])
        .count();
    assert!(
        white_before > 16,
        "later Button/Label content is visibly above its own fill"
    );
    f.reverse();
    let after = f.frame();
    assert!(after.scene_built);
    let after_members = control_order_member_fragments(&after.scene_root, host);
    assert!(Arc::ptr_eq(&members[0], &after_members[1]));
    assert!(Arc::ptr_eq(&members[1], &after_members[0]));
    let reversed = render_control_order_frame(
        &mut renderer,
        &compositor,
        surface,
        &after,
        host,
        Some(&binding),
    );
    let changed = (44..116)
        .flat_map(|y| (20..188).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel_at(&pixels, 320, x, y) != pixel_at(&reversed, 320, x, y))
        .count();
    assert!(
        changed > 100,
        "reorder changes actual GPU execution despite unchanged child Arcs"
    );
    let red = (44..116)
        .flat_map(|y| (20..188).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel_at(&reversed, 320, x, y) == [255, 0, 0, 255])
        .count();
    assert!(
        red > 16,
        "earlier Button's own focus projection becomes visible after explicit reorder"
    );
    assert_eq!(f.image_dependency_counts(), Some((3, 1)));
    assert_eq!(binding.availability(), Ok(()));
}

#[test]
fn control_subtree_order_gpu_preserves_workspace_clip_and_overlay() {
    use super::retained_value_updates::ControlOrderFixture;
    let (mut renderer, compositor, surface) =
        setup_with_size(320, 220).expect("control-subtree GPU proof requires adapter");
    let mut f = ControlOrderFixture::new(None);
    assert!(f.frame().scene_built);
    f.select_label();
    let before = f.frame();
    let host = f.tree.widget_id_for_node(f.host).unwrap();
    let pixels =
        render_control_order_frame(&mut renderer, &compositor, surface, &before, host, None);
    assert_eq!(
        pixel_at(&pixels, 320, 220, 108),
        [255, 0, 255, 255],
        "declarative preview inside workspace"
    );
    assert_eq!(
        pixel_at(&pixels, 320, 260, 108),
        [0, 0, 0, 255],
        "preview cannot escape workspace clip"
    );
    assert_eq!(
        pixel_at(&pixels, 320, 224, 128),
        [0, 255, 255, 255],
        "unrelated original overlay stays above preview"
    );
    assert_eq!(
        pixel_at(&pixels, 320, 244, 160),
        [192, 0, 0, 255],
        "strict external member stays above nested local Z"
    );
    let overlay_owner = f.tree.widget_id_for_node(f.overlay).unwrap();
    assert_eq!(
        f.tree.hit_test(224.0, 128.0).unwrap().widget_id,
        overlay_owner
    );
    assert!(
        f.tree.hit_test(220.0, 108.0).is_none(),
        "preview is noninteractive"
    );
    let external_hit = f.tree.hit_test(244.0, 160.0).unwrap();
    assert!(f.tree.is_descendant_or_same(
        f.tree.find_by_id(external_hit.widget_id).unwrap(),
        f.external
    ));
    f.reverse();
    let after = f.frame();
    let reversed =
        render_control_order_frame(&mut renderer, &compositor, surface, &after, host, None);
    for (x, y) in [(220, 108), (260, 108), (224, 128), (244, 160)] {
        assert_eq!(pixel_at(&pixels, 320, x, y), pixel_at(&reversed, 320, x, y));
    }
    assert_eq!(
        f.tree.hit_test(224.0, 128.0).unwrap().widget_id,
        overlay_owner
    );
}

fn render_workspace_preview_scene(
    renderer: &mut WgpuRenderer,
    compositor: &Compositor,
    surface: SurfaceId,
    scene: SceneGraph,
    host: crate::utils::id::TreeWidgetId,
    binding: &crate::renderer::image_binding::ImageBinding,
    live: bool,
) -> Vec<u8> {
    use crate::core::paint::LAYER_MAIN_AFFORDANCE;
    let scenes = HashMap::from([(surface, scene)]);
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    let source = &plan.surface_data[&surface];
    let images: Vec<_> = source
        .primitives
        .iter()
        .filter(|primitive| {
            matches!(
                primitive.data,
                crate::renderer::batcher::PrimitiveData::Image { .. }
            )
        })
        .collect();
    assert_eq!(images.len(), if live { 4 } else { 3 });
    assert!(images.iter().all(|primitive| primitive.key.texture_id
        == Some(binding.identity().texture_id())
        && primitive.key.image_generation == Some(binding.identity().generation())));
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("workspace preview compiled target");
    let data = compiled.surface_compiled_data(surface).unwrap();
    assert!(!matches!(
        data.text_realization().outcome(),
        crate::renderer::TextRealizationOutcome::Unavailable(_)
    ));
    assert!(
        data.draw_commands
            .windows(2)
            .all(|pair| pair[0].order() <= pair[1].order())
    );
    assert!(
        data.draw_commands
            .iter()
            .any(|command| matches!(command, SurfaceDrawCommand::Path { .. }))
    );
    let mut intervals = Vec::new();
    let mut affordance_command = None;
    let mut chrome_command = None;
    let mut text_background_run = false;
    for (index, command) in data.draw_commands.iter().enumerate() {
        if let Some(member) = command
            .order()
            .members()
            .iter()
            .find(|member| member.scope == host)
        {
            let interval = (member.z_index, member.member_index);
            if intervals.last() != Some(&interval) {
                intervals.push(interval);
            }
            if member.z_index == LAYER_MAIN_AFFORDANCE {
                affordance_command = Some(index);
            }
        }
        if command.z_index() == LAYER_SCROLLBAR {
            chrome_command = Some(index);
        }
        if command.z_index() >= LAYER_OVERLAY {
            assert!(command.order().members().is_empty());
        }
        if let SurfaceDrawCommand::Text {
            quad_start,
            quad_count,
            ..
        } = command
        {
            let quads =
                &data.text_quads[*quad_start as usize..(*quad_start + *quad_count) as usize];
            let backgrounds = quads
                .iter()
                .take_while(|quad| quad.downsample_u < 0.0)
                .count();
            if backgrounds > 0 {
                text_background_run = true;
                assert!(backgrounds < quads.len());
                assert!(
                    quads[backgrounds..]
                        .iter()
                        .all(|quad| quad.downsample_u >= 0.0)
                );
            }
        }
    }
    let mut expected = vec![(LAYER_CONTENT, 0), (LAYER_CONTENT, 1), (LAYER_CONTENT, 2)];
    if live {
        expected.push((LAYER_MAIN_AFFORDANCE, 0));
    }
    assert_eq!(
        intervals, expected,
        "complete ordinary intervals precede the one decorative interval"
    );
    assert!(
        text_background_run,
        "original Label selection/glyph run remains intact"
    );
    if live {
        assert!(affordance_command.unwrap() < chrome_command.expect("enclosing ScrollArea chrome"));
    }
    read_compiled_surface(renderer, &compiled, surface)
        .expect("workspace preview actual encode/readback")
}

#[test]
fn workspace_preview_composition_gpu_keeps_outline_above_bars_below_chrome() {
    use super::retained_value_updates::{
        ControlOrderFixture, workspace_preview_key_count, workspace_preview_outline_count,
    };
    let (mut renderer, compositor, surface) =
        setup_with_size(320, 220).expect("workspace preview GPU proof requires a real adapter");
    renderer.register_image_rgba8_srgb(359_207, 1, 1, &[255, 255, 255, 255]);
    let binding = renderer.admit_image(359_207).unwrap();
    let (mut f, handle) = ControlOrderFixture::workspace_preview(Some(&binding), true);
    let keys = f.preview_keys(handle);
    let host = f.tree.widget_id_for_node(f.host).unwrap();
    let top = f.tree.widget_id_for_node(f.buttons[1]).unwrap();
    f.fused_button(true, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 0]);
    f.fused_button(false, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 1]);
    f.select_label();
    let ordinary = f.frame();
    assert!(ordinary.scene_built);
    let before = render_workspace_preview_scene(
        &mut renderer,
        &compositor,
        surface,
        control_order_scene(&ordinary),
        host,
        &binding,
        false,
    );
    f.fused_button(true, 16.0, 112.0);
    f.fused_move(32.0, 96.0);
    f.fused_move(64.0, 64.0);
    f.assert_preview_capture(handle);
    let live = f.frame();
    assert!(live.scene_built);
    assert_eq!(workspace_preview_key_count(&live.scene_root, &keys), 1);
    assert_eq!(workspace_preview_outline_count(&live.scene_root), 4);
    assert!(!f.tree.node(handle).unwrap().dirty_paint);
    assert_eq!(f.image_dependency_counts(), Some((4, 1)));
    f.tree
        .rebuild_hit_test_grid(RectF::new(0.0, 0.0, 320.0, 220.0));
    assert!(f.tree.has_current_hit_test_grid());
    let canonical = f.tree.hit_test_canonical(64.0, 64.0).unwrap();
    let accelerated = f.tree.hit_test(64.0, 64.0).unwrap();
    assert_eq!(
        (canonical.widget_id, canonical.local_pos),
        (accelerated.widget_id, accelerated.local_pos)
    );
    assert_eq!(canonical.widget_id, top);
    assert_eq!(f.click_counts(), [0, 1]);
    let pixels = render_workspace_preview_scene(
        &mut renderer,
        &compositor,
        surface,
        control_order_scene(&live),
        host,
        &binding,
        true,
    );
    for y in 57..63 {
        for x in 57..188 {
            assert_eq!(
                pixel_at(&pixels, 320, x, y),
                [255, 255, 0, 255],
                "lower handle outline covers later Button fill/text at ({x},{y})"
            );
        }
    }
    for y in 101..104 {
        assert_eq!(
            pixel_at(&pixels, 320, 60, y),
            [255, 255, 0, 255],
            "outline covers the later Button's own focus border"
        );
    }
    assert_ne!(
        pixel_at(&pixels, 320, 196, 60),
        [255, 255, 0, 255],
        "intermediate bar clip is preserved"
    );
    assert_ne!(
        pixel_at(&pixels, 320, 60, 128),
        [255, 255, 0, 255],
        "outline cannot escape the original clip/scroll domain"
    );
    for &(x, y) in &[(242, 80), (100, 170), (250, 150), (188, 116), (250, 60)] {
        assert_eq!(
            pixel_at(&pixels, 320, x, y),
            pixel_at(&before, 320, x, y),
            "enclosing chrome, outer member, menu and outside pixels retain precedence at ({x},{y})"
        );
    }
    assert_eq!(pixel_at(&pixels, 320, 188, 116), [0, 255, 255, 255]);
    assert_eq!(
        f.tree.hit_test(188.0, 116.0).unwrap().widget_id,
        f.tree.widget_id_for_node(f.overlay).unwrap()
    );
    let source_arc = f.preview_arc(handle);
    let dependencies = f.image_dependency_counts();
    f.tree
        .clear_control_subtree_main_affordances(f.host)
        .unwrap();
    assert!(f.tree.has_current_hit_test_grid());
    let unexported = f.frame();
    assert!(unexported.scene_built);
    assert!(Arc::ptr_eq(&source_arc, &f.preview_arc(handle)));
    // The same source remains below the later bar when ordinary placement is restored.
    let restored = render_scene(
        &mut renderer,
        &compositor,
        surface,
        control_order_scene(&unexported),
    );
    assert_ne!(pixel_at(&restored, 320, 60, 60), [255, 255, 0, 255]);
    f.tree
        .rebuild_hit_test_grid(RectF::new(0.0, 0.0, 320.0, 220.0));
    assert!(f.tree.has_current_hit_test_grid());
    f.tree
        .set_control_subtree_main_affordances(f.host, &[handle])
        .unwrap();
    assert!(f.tree.has_current_hit_test_grid());
    let redeclared = f.frame();
    assert!(redeclared.scene_built);
    assert!(Arc::ptr_eq(&source_arc, &f.preview_arc(handle)));
    assert_eq!(dependencies, f.image_dependency_counts());
    let redeclared = render_workspace_preview_scene(
        &mut renderer,
        &compositor,
        surface,
        control_order_scene(&redeclared),
        host,
        &binding,
        true,
    );
    assert_eq!(
        pixels, redeclared,
        "pure composition restores exactly the same visible scene"
    );
    let fresh_scene = f.fresh_preview_scene();
    assert_eq!(workspace_preview_key_count(&fresh_scene.root, &keys), 1);
    assert_eq!(workspace_preview_outline_count(&fresh_scene.root), 4);
    let fresh = render_workspace_preview_scene(
        &mut renderer,
        &compositor,
        surface,
        fresh_scene,
        host,
        &binding,
        true,
    );
    assert_eq!(
        pixels, fresh,
        "fresh capture and retained composition share ordering and canonical scopes"
    );
    f.cancel_preview();
    f.assert_preview_cancelled(handle);
    let cancelled = f.frame();
    assert!(
        cancelled.scene_built,
        "first canonical cancelled scene is used directly"
    );
    assert_eq!(workspace_preview_key_count(&cancelled.scene_root, &keys), 0);
    assert_eq!(workspace_preview_outline_count(&cancelled.scene_root), 0);
    let cancelled_pixels = render_workspace_preview_scene(
        &mut renderer,
        &compositor,
        surface,
        control_order_scene(&cancelled),
        host,
        &binding,
        false,
    );
    assert_ne!(pixel_at(&cancelled_pixels, 320, 60, 60), [255, 255, 0, 255]);
    assert_eq!(f.click_counts(), [0, 1]);
    f.fused_button(false, 64.0, 64.0);
    f.fused_button(true, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 1]);
    f.fused_button(false, 64.0, 64.0);
    assert_eq!(f.click_counts(), [0, 2]);
}

fn base_scene() -> SceneBuilder {
    let mut builder = SceneBuilder::new();
    builder.push_rect(
        RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        Color::BLACK,
        0,
        None,
    );
    builder
}

#[cfg(target_os = "macos")]
fn measure_thin_film_shader(gpu: &crate::renderer::wgpu_renderer::GpuResources) -> f64 {
    use crate::renderer::pipelines::analytic_surface::tests::thin_film_reflectance_reference;

    let mut inputs = Vec::new();
    for elevation in [0.01_f64, 5.0, 37.0, 80.0, 90.0] {
        let ci = ((1.0 + elevation.to_radians().sin()) * 0.5).sqrt() as f32;
        for nm in [100.0, 320.0, 900.0] {
            for ior in [1.2, 1.4, 1.7] {
                inputs.push([ci, nm, ior]);
            }
        }
    }
    let vectors = inputs.iter().map(|[ci, nm, ior]| format!("vec3({ci:.9}, {nm:.1}, {ior:.9})"))
        .collect::<Vec<_>>().join(", ");
    // Invoke the production helper directly to observe raw f32 before rounding to RGBA8.
    let shader = format!("{}\n{}",
        include_str!("../renderer/shaders/glass_optics.wgsl"),
        format_args!(
            r#"
const inputs = array({vectors});
@group(0) @binding(0) var<storage, read_write> values: array<vec4<f32>>;
@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let p = inputs[id.x];
    values[id.x] = vec4(glass_film_reflectance(p.x, p.y, p.z), 0.0);
}}
"#,
        ));
    let device = &gpu.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("HAL394 reflectance precision test"),
        source: wgpu::ShaderSource::Wgsl(shader.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let size = inputs.len() as u64 * 16;
    let buffer = |usage| device.create_buffer(&wgpu::BufferDescriptor {
        label: None, size, usage, mapped_at_creation: false,
    });
    let result = buffer(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
    let readback = buffer(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: result.as_entire_binding() }],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &binding, &[]);
        pass.dispatch_workgroups(inputs.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&result, 0, &readback, 0, size);
    let submission = gpu.queue.submit([encoder.finish()]);
    let mapped = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    mapped.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = mapped.get_mapped_range().expect("mapped readback range");
    let values: &[[f32; 4]] = bytemuck::cast_slice(&bytes);
    let mut maximum_error = 0.0_f64;
    for ([ci, nm, ior], samples) in inputs.into_iter().zip(values) {
        let expected = thin_film_reflectance_reference(ci.into(), nm.into(), ior.into());
        for (&result, expected) in samples.iter().zip(expected) {
            assert!(result.is_finite() && (-0.00001..=1.00001).contains(&result));
            maximum_error = maximum_error.max((f64::from(result) - expected).abs());
        }
    }
    assert!(maximum_error <= 1.0e-5, "raw HAL394 WGSL reflectance error: {maximum_error}");
    maximum_error
}

#[cfg(target_os = "macos")]
#[test]
fn frosted_glass_thin_film_matches_smoked_and_reference_on_metal_1x_2x() {
    use crate::core::paint::{
        AnalyticSurfaceLight, Color as PaintColor, FrostedGlassAppearance, GlassSurfaceOptics,
        GlassSurfaceOpticsParameters, RoundedCorners as PaintCorners, ThinFilmParams,
    };
    use crate::renderer::pipelines::analytic_surface::tests::{coating_reference, distance};

    let source = |elevation: f64, azimuth: f64, center, color, strength| {
        let (z, xy) = elevation.to_radians().sin_cos();
        let (y, x) = azimuth.to_radians().sin_cos();
        AnalyticSurfaceLight::try_new(
            [(xy * x) as f32, (xy * y) as f32, z as f32], color, strength, center,
            Point::new(0.40, 0.45), Point::new(1.0, 0.0),
        ).unwrap()
    };
    let center = Point::new(0.5, 0.5);
    let white = source(5.0, 0.0, center, PaintColor::WHITE, 0.8);
    let unlit = source(80.0, 0.0, center, PaintColor::WHITE, 0.0);
    let facing = source(80.0, 0.0, center, PaintColor::WHITE, 0.8);
    let azimuth = source(5.0, 90.0, center, PaintColor::WHITE, 0.8);
    let shifted = source(5.0, 0.0, Point::new(0.73, 0.5), PaintColor::WHITE, 0.8);
    let black = source(5.0, 0.0, center, PaintColor::BLACK, 0.8);
    let colored = [
        source(5.0, 0.0, center, PaintColor::from_linear(0.3, 0.8, 0.5, 1.0), 0.8),
        source(80.0, 65.0, Point::new(0.8, 0.4), PaintColor::from_linear(0.8, 0.55, 0.4, 1.0), 0.65),
    ];
    let optics = |lights, strength: Option<f32>| {
        GlassSurfaceOptics::try_new(
            GlassSurfaceOpticsParameters {
                bevel_width: 8.0, bevel_slope: 0.8, roughness: 1.0,
                normal_reflectance: 0.5, reflection_strength: 1.0, rim_strength: 0.65, rim_falloff: 1.5,
            },
            lights, PaintColor::WHITE, PaintColor::WHITE,
        ).unwrap().try_with_thin_film(strength.map(|s| ThinFilmParams::try_new(s, 320.0, 1.4).unwrap())).unwrap()
    };
    let oblique = optics([white, unlit], Some(1.0));
    let opposite = optics([azimuth, unlit], Some(1.0));
    let displaced = optics([shifted, unlit], Some(1.0));
    let hue = |value: &GlassSurfaceOptics| {
        let rgb = coating_reference(value, [0.5, 0.5], -16.0, [0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
        let peak = rgb[..3].iter().copied().fold(0.0_f64, f64::max);
        [rgb[0] / peak, rgb[1] / peak, rgb[2] / peak]
    };
    for control in [&opposite, &displaced] {
        for (a, b) in hue(&oblique).into_iter().zip(hue(control)) {
            assert!((a - b).abs() < 1.0e-8, "position or azimuth must preserve flat hue");
        }
    }

    let rect = RectF::new(5.0, 5.0, 86.0, 54.0);
    let corners = PaintCorners::uniform(12.0);
    let clip = RectF::new(12.0, 3.0, 83.0, 58.0);
    let neutral = Color::from_linear(0.08, 0.08, 0.08, 1.0);
    let underlay = glass_material_lab_producer::neutral_smoked_glass();
    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(96, 64, scale).expect("HAL394 requires actual Metal");
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.adapter.get_info().backend, wgpu::Backend::Metal);
        assert!(gpu.format.is_srgb());
        if scale == 1.0 {
            eprintln!("HAL394 Metal raw maximum reflectance error {}", measure_thin_film_shader(&gpu));
        }
        let mut render = |coat, frosted| {
            let mut scene = SceneBuilder::new();
            scene.push_rect(RectF::new(0.0, 0.0, 96.0, 64.0), neutral, 0, None);
            scene.with_clip(clip, Some(crate::core::scene::RoundedCorners::uniform(8.0)), |scene| {
                if frosted {
                    let glass = FrostedGlassAppearance::try_new(1.25, PaintColor::TRANSPARENT, 1.0)
                        .unwrap().try_with_optics(coat, 0.0, 1.0).unwrap();
                    scene.push_frosted_glass(rect, corners, &glass, 4, None).unwrap();
                } else {
                    let glass = underlay.clone().with_glass_optics(coat);
                    scene.push_analytic_surface(rect, corners, &glass, 4, None).unwrap();
                }
            });
            scene.push_rect(RectF::new(24.0, 17.0, 2.0, 28.0), Color::WHITE, 5, None);
            render_scene_at_scale(&mut renderer, &compositor, id, scene.build(), scale)
        };
        let width = (96.0 * scale) as usize;
        let pixel = |pixels: &[u8], x: f32, y: f32| -> [u8; 4] {
            let index = (((y * scale) as usize) * width + (x * scale) as usize) * 4;
            pixels[index..index + 4].try_into().unwrap()
        };
        let mut maximum_error = 0;
        for frosted in [false, true] {
            let baseline = render(None, frosted);
            let no_film = render(Some(optics([white, unlit], None)), frosted);
            let zero_film = render(Some(optics([white, unlit], Some(0.0))), frosted);
            assert!(no_film == zero_film, "zero and absent film pixels must be identical");
            for inert in [[black, unlit], [unlit; 2]] {
                assert!(baseline == render(Some(optics(inert, Some(1.0))), frosted));
            }
            let mut samples = Vec::new();
            for coat in [oblique, opposite, displaced, optics([facing, unlit], Some(1.0)),
                optics(colored, Some(0.75))]
            {
                let pixels = render(Some(coat), frosted);
                assert_eq!(pixel(&pixels, 9.0, 32.0), pixel(&baseline, 9.0, 32.0));
                assert_eq!(pixel(&pixels, 25.0, 32.0), [255; 4]);
                let channels = [(48.0, 32.0), (88.0, 32.0)].map(|(x, y)| {
                    let point = Point::new(((x * scale).floor() + 0.5) / scale,
                        ((y * scale).floor() + 0.5) / scale);
                    let (d, n) = distance(rect, corners, point);
                    let u = f64::from((point.x - rect.x) / rect.width);
                    let v = f64::from((point.y - rect.y) / rect.height);
                    let expected = coating_reference(&coat, [u, v], d, n, [0.08, 0.08, 0.08, 1.0]);
                    let actual = pixel(&pixels, x, y);
                    maximum_error = maximum_error.max(thin_film_pixel_error(actual, expected));
                    actual
                });
                samples.push(channels);
            }
            assert_eq!(samples[0][0], samples[1][0], "flat azimuth invariance");
            assert_ne!(samples[0][1], samples[1][1], "bevel direction remains active");
            assert_ne!(samples[0][0], samples[2][0], "center still changes strength");
            assert!(samples[0][0][0] > samples[0][0][2] && samples[3][0][2] > samples[3][0][0]);
            eprintln!("HAL394 {scale}× frosted={frosted}: center and bevel {samples:?}");
        }
        assert!(maximum_error <= 2, "{scale}× maximum coating error: {maximum_error}");
        eprintln!("HAL394 coating {scale}× max sRGB code error {maximum_error}");
    }
}

#[cfg(target_os="macos")]
#[test]
fn frosted_glass_s2_metal_signed_lens_coating_emission_and_current_prefix_1x_2x() {
    use crate::core::paint::{Color as PaintColor,FrostedGlassAppearance as Glass,RoundedCorners as Corners,
        analytic_surface_tests::{coating,emission}};
    use crate::core::text_engine::TextEngine;
    use std::sync::Arc;
    let directory=std::path::PathBuf::from(format!("target/evidence/hal-optics-s2/frost-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut receipts=Vec::new();
    for scale in [1.0_f32,2.0] {
        let (mut renderer,compositor,id)=setup_with_size_and_scale(72,56,scale).expect("S2 requires real Metal");
        let info=renderer.gpu_resources().adapter.get_info();assert_eq!(info.backend,wgpu::Backend::Metal);
        assert!(renderer.gpu_resources().format.is_srgb(),"this oracle declares its RGBA8 sRGB transfer");
        let width=(72.0*scale) as usize;let height=(56.0*scale) as usize;
        let mut background=SceneBuilder::new();
        background.push_rect(RectF::new(0.0,0.0,72.0,56.0),Color::from_linear(0.08,0.18,0.3,0.8),0,None);
        background.push_circle(Point::new(23.0,24.0),15.0,Color::from_linear(0.6,0.12,0.07,0.7),1,None);
        let mut path=Path::new();path.move_to(8.0,5.0);path.line_to(65.0,40.0);path.line_to(9.0,44.0);path.close();
        background.push_path(path,Color::from_linear(0.03,0.6,0.25,0.8),2,None);
        renderer.register_image_rgba8_srgb(385_002,2,2,&[255,40,120,255,20,190,240,255,20,230,50,255,240,180,20,255]);
        background.push_image(RectF::new(38.0,9.0,24.0,23.0),385_002,RectF::new(0.0,0.0,1.0,1.0),Color::WHITE,3,None);
        renderer.text_engine_mut().set_scale(scale);let run=renderer.text_engine_mut().shape("Lower",10.0);
        renderer.text_engine_mut().draw(&run,7.0,17.0,PaintColor::WHITE,4,&mut background);
        let background:Arc<[crate::core::scene::SceneNode]>=Arc::from(background.build().root);
        let rect=RectF::new(8.5,7.5,35.0,29.0);let corners=Corners{tl:8.0,tr:2.0,bl:4.0,br:0.0};
        let mut outputs=Vec::new();
        for (case,(displacement,sigma,opacity,emitting,direction)) in [
            (0.0,2.0,1.0,false,0.6),(-12.0,2.0,1.0,false,0.6),(12.0,2.0,1.0,false,0.6),
            (0.0,2.0,0.5,false,-0.6),(0.0,0.0,0.0,true,0.6),(32.0,24.0,1.0,true,0.6),
            (-32.0,24.0,0.5,false,0.6)].into_iter().enumerate() {
            let material=Glass::try_new(sigma,PaintColor::from_linear(0.03,0.04,0.06,0.25),0.94).unwrap()
                .try_with_optics(Some(coating(direction)),displacement,opacity).unwrap()
                .with_surface_emission(if emitting{Some(emission())}else{None});
            let mut leaf=SceneBuilder::new();leaf.push_frosted_glass(rect,corners,&material,10,None).unwrap();
            let second=Glass::try_new(0.75,PaintColor::from_linear(0.2,0.06,0.1,0.6),0.8).unwrap();
            let r2=RectF::new(30.0,19.0,28.0,27.0);
            leaf.push_frosted_glass(r2,Corners::uniform(3.0),&second,11,None).unwrap();
            let local:Arc<[crate::core::scene::SceneNode]>=Arc::from(leaf.build().root);let same=local.clone();
            for moved in [false,true] {
                let mut scene=SceneBuilder::new();scene.push_retained_fragment(background.clone());
                if moved {scene.push_rect(RectF::new(18.0,4.0,2.0,46.0),Color::WHITE,5,None);}
                let mut source=scene.build();source.clear_color=Color::TRANSPARENT;
                let base=render_scene_at_scale(&mut renderer,&compositor,id,source,scale);
                let mut expected:Vec<_>=base.chunks_exact(4).map(|p|[frost_decode(p[0] as f64/255.0),
                    frost_decode(p[1] as f64/255.0),frost_decode(p[2] as f64/255.0),p[3] as f64/255.0]).collect();
                frost_cpu_oracle(&mut expected,width,height,scale,rect,corners,&material);
                frost_cpu_oracle(&mut expected,width,height,scale,r2,Corners::uniform(3.0),&second);
                let mut scene=SceneBuilder::new();scene.push_retained_fragment(background.clone());
                if moved {scene.push_rect(RectF::new(18.0,4.0,2.0,46.0),Color::WHITE,5,None);}
                scene.push_retained_fragment(local.clone());let mut scene=scene.build();scene.clear_color=Color::TRANSPARENT;
                let pixels=render_scene_at_scale(&mut renderer,&compositor,id,scene,scale);
                let mut worst=0_i16;
                for (actual,e) in pixels.chunks_exact(4).zip(expected) {for ch in 0..4 {
                    let encoded=if ch==3 {e[ch]}else{frost_encode(e[ch])};
                    worst=worst.max((actual[ch] as i16-(encoded.clamp(0.0,1.0)*255.0).round() as i16).abs());
                }}
                assert!(worst<=2,"S2 DPR={scale} case={case} shift={displacement} sigma={sigma} max_error={worst}");
                assert!(Arc::ptr_eq(&same,&local));
                let stem=format!("{scale}x-case{case}-moved{moved}");
                std::fs::write(directory.join(format!("{stem}.rgba")),&pixels).unwrap();
                let mut ppm=format!("P6\n{width} {height}\n255\n").into_bytes();for pixel in pixels.chunks_exact(4){ppm.extend_from_slice(&pixel[..3]);}
                std::fs::write(directory.join(format!("{stem}.ppm")),ppm).unwrap();
                receipts.push(serde_json::json!({"scale":scale,"case":case,"moved":moved,"shift":displacement,
                    "sigma":sigma,"radius":(sigma*scale*3.0).ceil(),"opacity":opacity,"emission":emitting,
                    "max_encoded_error":worst,"backend":format!("{:?}",info.backend),"adapter":info.name,"size":[width,height]}));
                if !moved {outputs.push(pixels);}
            }
        }
        assert_ne!(outputs[0],outputs[1]);assert_ne!(outputs[1],outputs[2]);
    }
    std::fs::write(directory.join("receipts.json"),serde_json::to_vec_pretty(&receipts).unwrap()).unwrap();
    println!("HAL_OPTICS_S2 real Metal evidence {}",directory.display());
}

#[cfg(target_os="macos")]
#[test]
fn frosted_glass_s2_metal_emission_original_ancestor_once_and_removal_1x_2x() {
    use crate::core::paint::{Color as PaintColor,FrostedGlassAppearance as Glass,RoundedCorners as Corners,
        SurfaceEmission,analytic_surface_tests::emission_parameters};
    use crate::renderer::pipelines::analytic_surface::tests::{distance,emission_reference};
    use std::sync::Arc;
    for scale in [1.0_f32,2.0] {
        let (mut renderer,compositor,id)=setup_with_size_and_scale(72,56,scale).expect("actual Metal required");
        assert_eq!(renderer.gpu_resources().adapter.get_info().backend,wgpu::Backend::Metal);
        let rect=RectF::new(12.0,10.0,44.0,32.0);let corners=Corners{tl:8.0,tr:3.0,bl:5.0,br:0.0};
        let mut p=emission_parameters();p.face_strength=0.0;p.edge_strength=0.0;p.halo_strength=1.0;p.falloff=1.0;p.directionality=0.0;
        let emission=SurfaceEmission::try_new(PaintColor::WHITE,p).unwrap();
        let material=Glass::try_new(0.0,PaintColor::TRANSPARENT,1.0).unwrap().try_with_optics(None,0.0,0.0).unwrap()
            .with_surface_emission(Some(emission));
        let fragment = |appearance: &Glass| -> Arc<[SceneNode]> {
            let mut leaf = SceneBuilder::new();
            leaf.push_frosted_glass(rect, corners, appearance, 10, None).unwrap();
            Arc::from(leaf.build().root)
        };
        let local = fragment(&material);
        let original = local.clone();
        let feathered = fragment(&material.clone().try_with_edge_feather(8.0).unwrap());
        let width=(72.0*scale) as usize;let height=(56.0*scale) as usize;let mut first=None;
        for (radius, leaf) in [(12.0, &local), (0.0, &local), (12.0, &local), (12.0, &feathered)] {
            let mut scene=SceneBuilder::new();
            scene.begin_clip(RectF::new(4.0,4.0,64.0,48.0),Some(crate::core::scene::RoundedCorners::uniform(radius)));
            scene.begin_clip(RectF::new(5.0,5.0,62.0,46.0),None);
            scene.push_retained_fragment(leaf.clone());scene.end_clip();scene.end_clip();
            let mut scene=scene.build();scene.clear_color=Color::TRANSPARENT;
            let pixels=render_scene_at_scale(&mut renderer,&compositor,id,scene,scale);let mut partial=0;let mut halo=0;
            for y in 0..height {for x in 0..width {
                let point=Point::new((x as f32+0.5)/scale,(y as f32+0.5)/scale);
                let mut a=if (5.0..67.0).contains(&point.x) && (5.0..51.0).contains(&point.y) {1.0}else{0.0};
                if radius>0.0 && a>0.0 {
                    let lx=f64::from(point.x)-4.0;let ly=f64::from(point.y)-4.0;let r=f64::from(radius);
                    let cx=if lx<r {Some(r)}else if lx>64.0-r {Some(64.0-r)}else{None};
                    let cy=if ly<r {Some(r)}else if ly>48.0-r {Some(48.0-r)}else{None};
                    if let (Some(cx),Some(cy))=(cx,cy) {let u=((lx-cx).hypot(ly-cy)-r).clamp(0.0,1.0);a=1.0-u*u*(3.0-2.0*u);}
                }
                let (d,g)=distance(rect,corners,point);let e=emission_reference(&emission,d,g,0.0);
                let expected=(255.0*a*e[3]).round() as i16;let actual=pixels[(y*width+x)*4+3] as i16;
                assert!((actual-expected).abs()<=2,"one ancestor multiplication DPR={scale} pixel=({x},{y}) alpha={actual} expected={expected}");
                if a>0.1 && a<0.9 && e[3]>0.1 {partial+=1;}
                if d>1.0 && actual>0 {halo+=1;}
            }}
            assert!(halo>10);if radius>0.0 {assert!(partial>0);}
            if let Some(first)=&first {if radius>0.0 {assert_eq!(first,&pixels);}}else{first=Some(pixels);}
            assert!(Arc::ptr_eq(&local,&original));
        }
        let mut removed=SceneBuilder::new();removed.push_frosted_glass(rect,corners,&material.with_surface_emission(None),10,None).unwrap();
        let mut removed=removed.build();removed.clear_color=Color::TRANSPARENT;
        assert!(render_scene_at_scale(&mut renderer,&compositor,id,removed,scale).iter().all(|b|*b==0));
    }
}

fn render_scene(
    renderer: &mut WgpuRenderer,
    compositor: &Compositor,
    surface_id: SurfaceId,
    scene: SceneGraph,
) -> Vec<u8> {
    render_scene_at_scale(renderer, compositor, surface_id, scene, 1.0)
}

fn render_scene_at_scale(
    renderer: &mut WgpuRenderer,
    compositor: &Compositor,
    surface_id: SurfaceId,
    scene: SceneGraph,
    scale_factor: f32,
) -> Vec<u8> {
    let mut scenes = HashMap::new();
    scenes.insert(surface_id, scene);
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| scale_factor,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    let compiled = renderer
        .compile_multi(&plan)
        .expect("compile SPEC-HAL-359 scene")
        .expect_ready("compiled SPEC-HAL-359 target");
    if let crate::renderer::TextRealizationOutcome::Unavailable(failure) =
        compiled.text_realization(surface_id).unwrap().outcome()
    {
        // A primitive's Debug data may contain complete font backing bytes.
        // Keep this owner failure bounded; its typed indices identify the cut.
        panic!("expected complete SPEC-HAL-359 surface: {failure:?}");
    }
    read_compiled_surface(renderer, &compiled, surface_id).expect("complete SPEC-HAL-359 surface")
}

#[cfg(target_os="macos")]
#[test]
fn frosted_glass_real_metal_current_prefix_gaussian_and_replacement_1x_2x() {
    use crate::core::paint::{Color as PaintColor,FrostedGlassAppearance,RoundedCorners as Corners};
    use crate::core::text_engine::TextEngine;
    use std::sync::Arc;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let evidence = std::env::temp_dir().join(format!(
        "hal385-metal-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&evidence)
        .unwrap();
    std::fs::set_permissions(&evidence,std::fs::Permissions::from_mode(0o700)).unwrap();
    eprintln!("HAL385 Metal evidence at {}", evidence.display());
    for scale in [1.0_f32,2.0] {
        let (mut renderer,compositor,id)=setup_with_size_and_scale(72,56,scale).expect("HAL385 actual Metal adapter required");
        let info=renderer.gpu_resources().adapter.get_info();assert_eq!(info.backend,wgpu::Backend::Metal);
        let mut background=SceneBuilder::new();
        renderer.register_image_rgba8_srgb(385_001,2,2,&[255,30,100,255,30,200,240,255,10,240,60,255,220,190,10,255]);
        background.push_rect(RectF::new(0.0,0.0,72.0,56.0),Color::from_linear(0.08,0.15,0.23,0.7),0,None);
        for i in 0..24 {background.push_rect(RectF::new(i as f32*3.0,0.0,1.5,56.0),
            if i%2==0 {Color::from_linear(0.8,0.2,0.05,0.75)} else {Color::from_linear(0.03,0.65,0.8,0.6)},1,None);}
        background.push_circle(Point::new(19.0,24.0),8.0,Color::from_linear(0.4,0.8,0.2,0.8),2,None);
        background.push_image(RectF::new(35.0,10.0,22.0,19.0),385_001,RectF::new(0.0,0.0,1.0,1.0),Color::WHITE,2,None);
        renderer.text_engine_mut().set_scale(scale);
        let run=renderer.text_engine_mut().shape("Lower",10.0);
        renderer.text_engine_mut().draw(&run,9.0,17.0,PaintColor::WHITE,3,&mut background);
        let background:Arc<[crate::core::scene::SceneNode]>=Arc::from(background.build().root);
        let mut leaf=SceneBuilder::new();
        let first=FrostedGlassAppearance::try_new(2.25,PaintColor::from_linear(0.1,0.2,0.3,0.6),0.72).unwrap();
        let first_rect=RectF::new(8.5,7.5,35.0,29.0);
        let corners=Corners{tl:8.0,tr:2.0,bl:4.0,br:0.0};
        leaf.push_frosted_glass(first_rect,corners,&first,10,Some(crate::core::scene_identity::SceneNodeKey::new(385))).unwrap();
        let second=FrostedGlassAppearance::try_new(0.75,PaintColor::from_linear(0.3,0.1,0.06,0.5),0.8).unwrap();
        let second_rect=RectF::new(30.0,19.0,28.0,27.0);
        leaf.push_frosted_glass(second_rect,Corners::uniform(3.0),&second,11,None).unwrap();
        let local:Arc<[crate::core::scene::SceneNode]>=Arc::from(leaf.build().root);let original=local.clone();
        let scene=|glass:bool,shift:f32| {
            let mut s=SceneBuilder::new();s.push_retained_fragment(background.clone());
            if shift>0.0 {s.push_rect(RectF::new(17.0,8.0,18.0,34.0),Color::from_linear(0.7,0.03,0.1,1.0),4,None);}
            if glass {s.push_retained_fragment(local.clone());}
            let mut scene=s.build();scene.clear_color=Color::TRANSPARENT;scene
        };
        let width=(72.0*scale) as usize;let height=(56.0*scale) as usize;
        let mut last=None;
        for shift in [0.0,1.0,0.0] {
            let base=render_scene_at_scale(&mut renderer,&compositor,id,scene(false,shift),scale);
            let mut expected:Vec<[f64;4]>=base.chunks_exact(4).map(|p|[
                frost_decode(f64::from(p[0])/255.0),frost_decode(f64::from(p[1])/255.0),frost_decode(f64::from(p[2])/255.0),f64::from(p[3])/255.0]).collect();
            frost_cpu_oracle(&mut expected,width,height,scale,first_rect,corners,&first);
            frost_cpu_oracle(&mut expected,width,height,scale,second_rect,Corners::uniform(3.0),&second);
            let pixels=render_scene_at_scale(&mut renderer,&compositor,id,scene(true,shift),scale);
            let mut max_error=0i16;
            for (actual,expected) in pixels.chunks_exact(4).zip(&expected) {
                for channel in 0..4 {
                    let e=if channel==3 {expected[channel]}else{frost_encode(expected[channel])};
                    let code=(e.clamp(0.0,1.0)*255.0).round() as i16;
                    max_error=max_error.max((i16::from(actual[channel])-code).abs());
                }
                let decoded=[frost_decode(actual[0] as f64/255.0),frost_decode(actual[1] as f64/255.0),frost_decode(actual[2] as f64/255.0)];
                assert!(decoded.into_iter().all(|v|v<=actual[3] as f64/255.0+0.008),"premultiplied output including quantization");
            }
            assert!(max_error<=2,"HAL385 independent Gaussian/half-float/replacement oracle: DPR={scale} max_error={max_error} codes");
            assert_ne!(pixels,base,"genuine positive blur is not an opaque tint or identity");
            if shift==1.0 {assert_ne!(last.as_ref().unwrap(),&pixels,"unchanged local glass Arc reads the CURRENT changed prefix");}
            else if let Some(prior)=&last {assert_eq!(prior,&pixels,"restored source reproduces deterministic output");}
            if shift==0.0 && last.is_none() {last=Some(pixels.clone());}
            assert!(Arc::ptr_eq(&local,&original));
            println!("HAL385_METAL adapter={} backend={:?} DPR={scale} size={width}x{height} shift={shift} max_encoded_error={max_error} pool={:?}",info.name,info.backend,renderer.frosted_pool_for_test());
            if shift==0.0 {
                let name=format!("frost-{scale}x");
                std::fs::write(evidence.join(format!("{name}.rgba")),&pixels).unwrap();
                let mut ppm=format!("P6\n{width} {height}\n255\n").into_bytes();for p in pixels.chunks_exact(4){ppm.extend_from_slice(&p[..3]);}
                std::fs::write(evidence.join(format!("{name}.ppm")),ppm).unwrap();
            }
        }
        let warmed=renderer.frosted_pool_for_test();
        for _ in 0..3 {let _=render_scene_at_scale(&mut renderer,&compositor,id,scene(true,0.0),scale);}
        assert_eq!(renderer.frosted_pool_for_test(),warmed,"completed warmed workload must stabilize physical allocations and residency");
        // An unchanged sigma-zero/transmission-one request is byte-identical.
        let mut identity=SceneBuilder::new();identity.push_retained_fragment(background.clone());
        identity.push_frosted_glass(first_rect,corners,&FrostedGlassAppearance::try_new(0.0,PaintColor::WHITE,1.0).unwrap(),10,None).unwrap();
        let mut identity=identity.build();identity.clear_color=Color::TRANSPARENT;
        assert_eq!(render_scene_at_scale(&mut renderer,&compositor,id,identity,scale),render_scene_at_scale(&mut renderer,&compositor,id,scene(false,0.0),scale));
        let grain=crate::core::paint::SurfaceMicrograin::try_new(1.5,2.0/255.0,387).unwrap();
        for (sigma,transmission,alpha,grain) in [(0.0,0.0,0.0,None),(0.0,0.0,0.5,None),(0.0,0.0,1.0,None),(24.0,1.0,0.0,None),(2.25,0.4,0.6,Some(grain))] {
            let material=FrostedGlassAppearance::try_new(sigma,PaintColor::from_linear(0.07,0.2,0.12,alpha),transmission).unwrap().with_micrograin(grain);
            let base=render_scene_at_scale(&mut renderer,&compositor,id,scene(false,0.0),scale);
            let mut expected:Vec<_>=base.chunks_exact(4).map(|p|[frost_decode(p[0] as f64/255.0),frost_decode(p[1] as f64/255.0),frost_decode(p[2] as f64/255.0),p[3] as f64/255.0]).collect();
            frost_cpu_oracle(&mut expected,width,height,scale,first_rect,corners,&material);
            let mut s=SceneBuilder::new();s.push_retained_fragment(background.clone());
            s.push_frosted_glass(first_rect,corners,&material,10,None).unwrap();
            let mut s=s.build();s.clear_color=Color::TRANSPARENT;
            let output=render_scene_at_scale(&mut renderer,&compositor,id,s,scale);
            let mut worst=0;
            for (px,e) in output.chunks_exact(4).zip(expected) {for ch in 0..4 {
                let code=((if ch==3 {e[ch]}else{frost_encode(e[ch])}).clamp(0.0,1.0)*255.0).round() as i16;
                worst=worst.max((px[ch] as i16-code).abs());
            }}
            assert!(worst<=2,"HAL385 sigma={sigma} alpha={alpha} transmission={transmission} DPR={scale} max_error={worst}");
            println!("HAL385_METAL controls sigma={sigma} radius={} alpha={alpha} transmission={transmission} grain={} DPR={scale} max_encoded_error={worst}",(sigma*scale*3.0).ceil(),grain.is_some());
        }
        // Same admitted native run above the markers. The existing LCD shader's
        // alpha is max(channel coverage), not a claim of equal RGB coverage.
        // Bound OVER by the actual foreground reference's 8-bit quantization.
        let mut sharp=SceneBuilder::new();
        let run=renderer.text_engine_mut().shape("Sharp",12.0);
        renderer.text_engine_mut().draw(&run,12.0,12.0,PaintColor::WHITE,30,&mut sharp);
        let sharp:Arc<[crate::core::scene::SceneNode]>=Arc::from(sharp.build().root);
        let mut text_only=SceneBuilder::new();text_only.push_retained_fragment(sharp.clone());
        let mut text_only=text_only.build();text_only.clear_color=Color::TRANSPARENT;
        let glyphs=render_scene_at_scale(&mut renderer,&compositor,id,text_only,scale);
        let smooth=render_scene_at_scale(&mut renderer,&compositor,id,scene(true,0.0),scale);
        let mut composed=SceneBuilder::new();composed.push_retained_fragment(background.clone());
        composed.push_retained_fragment(local.clone());composed.push_retained_fragment(sharp);
        let mut composed=composed.build();composed.clear_color=Color::TRANSPARENT;
        let composed=render_scene_at_scale(&mut renderer,&compositor,id,composed,scale);
        let mut cores=0;
        for ((glyph,base),output) in glyphs.chunks_exact(4).zip(smooth.chunks_exact(4)).zip(composed.chunks_exact(4)) {
            let alpha_lo=((f64::from(glyph[3])-0.5)/255.0).max(0.0);
            let alpha_hi=((f64::from(glyph[3])+0.5)/255.0).min(1.0);
            for ch in 0..3 {
                let low=frost_decode(((f64::from(glyph[ch])-0.5)/255.0).max(0.0));
                let high=frost_decode(((f64::from(glyph[ch])+0.5)/255.0).min(1.0));
                let b=frost_decode(f64::from(base[ch])/255.0);
                let lower=frost_encode((low+b*(1.0-alpha_hi)).clamp(0.0,1.0))*255.0;
                let upper=frost_encode((high+b*(1.0-alpha_lo)).clamp(0.0,1.0))*255.0;
                assert!(f64::from(output[ch])+1.0>=lower && f64::from(output[ch])-1.0<=upper,
                    "sharp foreground OVER (including LCD and terminal quantization), not blurred source: glyph={glyph:?} base={base:?} output={output:?} channel={ch} interval={lower}..{upper}");
            }
            if glyph[3]==255 {cores+=1;}
        }
        assert!(cores>0,"actual native font produced sharp opaque foreground ink");
        println!("HAL385_METAL sharp_foreground DPR={scale} opaque_core_pixels={cores}");
    }
    println!("HAL385_METAL evidence={}",evidence.display());
}

#[cfg(target_os="macos")]
#[test]
fn frosted_glass_metal_inward_feather_matches_current_prefix_at_logical_width() {
    use crate::core::paint::{
        Color as PaintColor, FrostedGlassAppearance as Glass, RoundedCorners as Corners,
        analytic_surface_tests::coating,
    };
    use crate::renderer::pipelines::analytic_surface::tests::distance;

    let base_material =
        Glass::try_new(2.25, PaintColor::from_linear(0.66, 0.16, 0.05, 0.95), 0.65).unwrap();
    let coated = base_material
        .clone()
        .try_with_optics(Some(coating(0.6)), 2.0, 0.65)
        .unwrap();
    let viewport = RectF::new(0.0, 0.0, 72.0, 56.0);
    let ancestor = RectF::new(10.0, 9.0, 42.0, 36.0);
    let tiny = RectF::new(58.0, 24.0, 6.0, 6.0);
    let tiny_corners = Corners::uniform(1.0);
    let corners = Corners { tl: 6.0, tr: 2.0, bl: 4.0, br: 8.0 };

    for scale in [1.0_f32, 1.5, 2.0] {
        let (mut renderer, compositor, id) =
            setup_with_size_and_scale(72, 56, scale).expect("HAL-385 feather requires Metal");
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.adapter.get_info().backend, wgpu::Backend::Metal);
        assert!(gpu.format.is_srgb());
        let width = (72.0 * scale) as usize;
        let height = (56.0 * scale) as usize;
        // The left edge goes through a pixel center at each admitted scale.
        let rect = RectF::new(12.0 + 0.5 / scale, 10.0, 32.0, 32.0);
        for (material, clipped) in [(&base_material, false), (&coated, true)] {
            let fragment = |appearance: &Glass| -> Arc<[SceneNode]> {
                let mut leaf = SceneBuilder::new();
                if clipped {
                    leaf.begin_clip(
                        ancestor,
                        Some(crate::core::scene::RoundedCorners::uniform(18.0)),
                    );
                }
                leaf.push_frosted_glass(rect, corners, appearance, 10, None).unwrap();
                if clipped {
                    leaf.end_clip();
                } else {
                    // The second marker's kernel cannot reach the first body.
                    leaf.push_frosted_glass(tiny, tiny_corners, appearance, 11, None).unwrap();
                }
                Arc::from(leaf.build().root)
            };
            let hard = fragment(material);
            let feathered = fragment(&material.clone().try_with_edge_feather(8.0).unwrap());
            let explicit_zero = fragment(&material.clone().try_with_edge_feather(-0.0).unwrap());
            for shift in [0.0_f32, 1.0] {
                let render = |renderer: &mut WgpuRenderer, leaf: Option<&Arc<[SceneNode]>>| {
                    let mut scene = SceneBuilder::new();
                    scene.push_rect(viewport, Color::from_linear(0.04, 0.16, 0.28, 0.42), 0, None);
                    for stripe in 0..11 {
                        scene.push_rect(
                            RectF::new(0.0, stripe as f32 * 5.0 + 1.0 + shift, 72.0, 1.25),
                            Color::from_linear(0.1, 0.63, 0.38, 0.5),
                            1,
                            None,
                        );
                    }
                    if let Some(leaf) = leaf {
                        scene.push_retained_fragment(leaf.clone());
                    }
                    let mut scene = scene.build();
                    scene.clear_color = Color::TRANSPARENT;
                    render_scene_at_scale(renderer, &compositor, id, scene, scale)
                };
                let original = render(&mut renderer, None);
                let hard_pixels = render(&mut renderer, Some(&hard));
                let soft_pixels = render(&mut renderer, Some(&feathered));
                if shift == 0.0 {
                    assert_eq!(hard_pixels, render(&mut renderer, Some(&explicit_zero)));
                }

                let mut worst = 0;
                let mut has_curved_falloff = false;
                let mut has_inherited_partial = false;
                let mut has_tiny_falloff = false;
                for y in 0..height {
                    for x in 0..width {
                        let point = Point::new((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                        let mut signed = distance(rect, corners, point).0;
                        if !clipped {
                            signed = signed.min(distance(tiny, tiny_corners, point).0);
                        }
                        let i = (y * width + x) * 4;
                        let src = &original[i..i + 4];
                        let hard = &hard_pixels[i..i + 4];
                        let soft = &soft_pixels[i..i + 4];
                        if signed >= 0.0 {
                            assert_eq!(soft, src, "exterior uses the exact prefix at ({x}, {y})");
                        }
                        let t = (-signed / 8.0).clamp(0.0, 1.0);
                        let f = t * t * (3.0 - 2.0 * t);
                        for ch in 0..4 {
                            let decode = |c: u8| {
                                let code = f64::from(c) / 255.0;
                                if ch == 3 { code } else { frost_decode(code) }
                            };
                            // The hard readback already includes own AA and the ancestor once.
                            let linear = (1.0 - f) * decode(src[ch]) + f * decode(hard[ch]);
                            let code = if ch == 3 { linear } else { frost_encode(linear) };
                            worst = worst.max(soft[ch].abs_diff((code * 255.0).round() as u8));
                        }
                        if t > 0.0 && t < 1.0 && soft != hard && soft != src {
                            has_curved_falloff |= point.x < rect.x + corners.tl
                                && point.y < rect.y + corners.tl;
                            if clipped {
                                let mask_distance = distance(ancestor, Corners::uniform(18.0), point).0;
                                has_inherited_partial |= mask_distance > 0.0
                                    && mask_distance < 0.5 / f64::from(scale);
                            } else {
                                has_tiny_falloff |= point.x >= tiny.x && point.x < tiny.x + tiny.width;
                            }
                        }
                    }
                }
                assert!(worst <= 2, "feather {scale}x clipped={clipped} prefix shift={shift} error={worst}");
                if clipped {
                    assert!(has_inherited_partial, "{scale}x must exercise partially masked feather");
                } else {
                    assert!(has_curved_falloff && has_tiny_falloff);
                    let y = (28.0 * scale) as usize;
                    let alpha_index = |inset: f32| (y * width + ((12.0 + inset) * scale) as usize) * 4 + 3;
                    let alphas = [0.0, 2.0, 4.0, 6.0, 8.0].map(|inset| soft_pixels[alpha_index(inset)]);
                    assert!(alphas.windows(2).all(|pair| pair[0] < pair[1]), "{scale}x: {alphas:?}");
                    let edge = alpha_index(0.0);
                    assert_eq!(&soft_pixels[edge - 3..=edge], &original[edge - 3..=edge]);
                    assert_ne!(hard_pixels[edge], original[edge], "hard body has coverage at the boundary");
                    let full = alpha_index(8.0);
                    assert_eq!(&soft_pixels[full - 3..=full], &hard_pixels[full - 3..=full]);
                }
                println!("HAL385_FEATHER Metal {scale}x clipped={clipped} shift={shift} max_encoded_error={worst}");
            }
        }
    }
}

#[cfg(target_os="macos")]
#[test]
fn frosted_glass_real_metal_inherited_mask_retention_targets_and_refusals() {
    use crate::core::paint::{Color as PaintColor,FrostedGlassAppearance,RoundedCorners as Corners};
    use crate::renderer::{RendererCompileError,FrostedGlassCompileRefusal};
    use crate::renderer::draw_plan_builder::{DrawPlanBuildError,FrostedGlassBuildRefusal};
    use std::sync::Arc;
    for scale in [1.0_f32,2.0] {
        let (mut renderer,mut compositor,id)=setup_with_size_and_scale(72,56,scale).expect("actual Metal required");
        let material=FrostedGlassAppearance::try_new(1.5,PaintColor::from_linear(0.75,0.12,0.05,1.0),0.0).unwrap();
        let mut leaf=SceneBuilder::new();leaf.push_frosted_glass(RectF::new(0.0,0.0,72.0,56.0),Corners::uniform(0.0),&material,10,None).unwrap();
        let local:Arc<[crate::core::scene::SceneNode]>=Arc::from(leaf.build().root);let original=local.clone();
        let make=|color:Color,outer:f32,inner:f32| {
            let mut b=SceneBuilder::new();b.push_rect(RectF::new(0.0,0.0,72.0,56.0),color,0,None);
            b.begin_clip(RectF::new(4.0,4.0,64.0,48.0),Some(crate::core::scene::RoundedCorners::uniform(outer)));
            b.begin_clip(RectF::new(8.0,5.0,55.0,46.0),Some(crate::core::scene::RoundedCorners::uniform(inner)));
            b.push_retained_fragment(local.clone());b.end_clip();b.end_clip();b.build()
        };
        let source=Color::from_linear(0.1,0.6,0.25,1.0);
        let mut reference=None;
        let mut builder=DrawPlanBuilder::new();let mut cache=MeshCache::new();
        for (outer,inner) in [(12.0,0.0),(12.0,4.0),(0.0,0.0),(12.0,0.0)] {
            let result=builder.build_multi(&HashMap::from([(id,make(source,outer,inner))]),compositor.snapshot_multi(),&mut cache,|_|scale,0);
            if inner>0.0 {
                assert!(matches!(result,Err(DrawPlanBuildError::FrostedGlass{reason:FrostedGlassBuildRefusal::UnsupportedClip{depth:2},..})));continue;
            }
            let compiled=renderer.compile_multi(&result.unwrap()).unwrap().expect_ready("native Compile must be ready");
            let pixels=read_compiled_surface(&renderer,&compiled,id).unwrap();
            let width=(72.0*scale) as usize;let height=(56.0*scale) as usize;
            let mut worst=0;
            for y in 0..height {for x in 0..width {
                let px=(x as f64+0.5)/scale as f64;let py=(y as f64+0.5)/scale as f64;
                let mut m=0.0;
                if px>=8.0 && px<63.0 && py>=5.0 && py<51.0 {
                    m=1.0;
                    if outer>0.0 {
                        let local=(px-4.0,py-4.0);let r=outer as f64;
                        let center=if local.0<r && local.1<r {Some((r,r))}
                            else if local.0>64.0-r && local.1<r {Some((64.0-r,r))}
                            else if local.0<r && local.1>48.0-r {Some((r,48.0-r))}
                            else if local.0>64.0-r && local.1>48.0-r {Some((64.0-r,48.0-r))}else{None};
                        if let Some(center)=center {let u=((local.0-center.0).hypot(local.1-center.1)-r).clamp(0.0,1.0);m=1.0-u*u*(3.0-2.0*u);}
                    }
                }
                let tint=material.tint();let a=[source.r as f64,source.g as f64,source.b as f64];let b=[tint.r as f64,tint.g as f64,tint.b as f64];
                for ch in 0..3 {
                    // Source really resides in the 8-bit target before capture.
                    let base=frost_decode((frost_encode(a[ch])*255.0).round()/255.0);
                    let expected=(frost_encode((1.0-m)*base+m*b[ch])*255.0).round() as i16;
                    worst=worst.max((i16::from(pixels[(y*width+x)*4+ch])-expected).abs());
                }
                assert_eq!(pixels[(y*width+x)*4+3],255);
            }}
            assert!(worst<=2,"original rounded parent survives inner rectangular intersection, once: DPR={scale} outer={outer} error={worst}");
            if outer>0.0 {if let Some(ref old)=reference {assert_eq!(old,&pixels);}else{reference=Some(pixels);}}
            assert!(Arc::ptr_eq(&local,&original));
            println!("HAL385_MASK DPR={scale} outer={outer} inner={inner} max_encoded_error={worst}");
        }
        let second=compositor.create_surface(1,RectF::new(0.0,0.0,72.0,56.0),SceneKey::BASE);
        renderer.register_headless_render_target(second,((72.0*scale) as u32,(56.0*scale) as u32),scale).unwrap();
        let blue=Color::from_linear(0.03,0.1,0.8,1.0);
        let plan=builder.build_multi(&HashMap::from([(id,make(source,12.0,0.0)),(second,make(blue,12.0,0.0))]),compositor.snapshot_multi(),&mut cache,|_|scale,0).unwrap();
        let compiled=renderer.compile_multi(&plan).unwrap().expect_ready("native Compile must be ready");
        let first_pixels=read_compiled_surface(&renderer,&compiled,id).unwrap();
        let second_pixels=read_compiled_surface(&renderer,&compiled,second).unwrap();
        assert_ne!(first_pixels,second_pixels,"same ordinal on two surfaces has isolated backing and prefix");
        let allocations=renderer.frosted_pool_for_test().0;
        let large=compositor.create_surface(2,RectF::new(0.0,0.0,8192.0,8192.0),SceneKey::BASE);
        renderer.register_headless_render_target(large,(8192,8192),1.0).unwrap();
        let plan=builder.build_multi(&HashMap::from([(large,make(source,12.0,0.0))]),compositor.snapshot_multi(),&mut cache,|_|1.0,0).unwrap();
        assert!(matches!(renderer.compile_multi(&plan),Err(RendererCompileError::FrostedGlass{reason:FrostedGlassCompileRefusal::ResourceBudget{limit_bytes:268435456,..},..})),"budget refusal precedes backing allocation");
        assert_eq!(renderer.frosted_pool_for_test().0,allocations);
        renderer.unregister_render_target(large).unwrap();
        compositor.destroy_surface(large);
        renderer.register_image_rgba8_srgb(385_099,1,1,&[255,255,255,255]);
        let mut missing=SceneBuilder::new();missing.push_image(RectF::new(0.0,0.0,72.0,56.0),385_099,RectF::new(0.0,0.0,1.0,1.0),Color::WHITE,0,None);
        missing.push_retained_fragment(local.clone());
        let plan=builder.build_multi(&HashMap::from([(id,missing.build())]),compositor.snapshot_multi(),&mut cache,|_|scale,0).unwrap();
        renderer.unregister_image(385_099);
        assert!(matches!(renderer.compile_multi(&plan),Err(RendererCompileError::FrostedGlass{reason:FrostedGlassCompileRefusal::IncompletePrefix,..})),"withdrawn lower Image cannot become a silently incomplete blurred prefix");
    }
}

#[cfg(target_os="macos")]
#[test]
fn frosted_glass_raw_command_replacement_retains_unconfirmed_backing() {
    use crate::core::paint::{FrostedGlassAppearance,RoundedCorners as Corners,Color as PaintColor};
    let (mut renderer,compositor,id)=setup_with_size_and_scale(40,32,1.0).expect("actual Metal required");
    let mut scene=SceneBuilder::new();
    scene.push_frosted_glass(RectF::new(1.0,1.0,30.0,24.0),Corners::uniform(2.0),
        &FrostedGlassAppearance::try_new(1.0,PaintColor::WHITE,0.8).unwrap(),10,None).unwrap();
    let plan=DrawPlanBuilder::new().build_multi(&HashMap::from([(id,scene.build())]),compositor.snapshot_multi(),
        &mut MeshCache::new(),|_|1.0,0).unwrap();
    let compiled=renderer.compile_multi(&plan).unwrap().expect_ready("native Compile must be ready");
    let gpu=renderer.gpu_resources();
    let target=gpu.device.create_texture(&wgpu::TextureDescriptor{label:Some("HAL385 command provenance negative"),
        size:wgpu::Extent3d{width:40,height:32,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
        dimension:wgpu::TextureDimension::D2,format:gpu.format,usage:wgpu::TextureUsages::RENDER_ATTACHMENT,view_formats:&[]});
    let original=renderer.encode_compiled_target_for_test(&compiled,id,&target.create_view(&Default::default())).unwrap();
    let before=renderer.frosted_pool_for_test();
    // The original public-field mem::replace attack is now a compile-fail
    // boundary. Complete unrelated B through the same sealed submission owner
    // while keeping A, then exercise the explicit consuming raw-export threat.
    let replacement=gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor{label:Some("HAL385 replacement empty command")}).finish();
    renderer.submit_compiled_target_for_test(crate::renderer::frosted_glass::EncodedTargetCommands::unmanaged(replacement));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let successor=renderer.compile_multi(&plan).unwrap().expect_ready("native Compile must be ready");
    let after=renderer.frosted_pool_for_test();
    println!("HAL385_COMMAND_PROVENANCE original_still_owned=true original_submitted=false replacement_completed=true before={before:?} after={after:?}");
    assert!(after.0>before.0,"INV-385-CUSTODY: completion of a replaced empty command must not release/reuse the original still-owned unsubmitted command's backing");
    assert_eq!(renderer.frosted_pool_states_for_test()[2],1,"A remains encoded and held");
    let cancelled=gpu.device.create_command_encoder(&Default::default()).finish();
    drop(crate::renderer::frosted_glass::EncodedTargetCommands::unmanaged(cancelled));
    assert_eq!(renderer.frosted_pool_states_for_test()[2],1,"cancelling unrelated B cannot cancel A");
    let raw_original=original.into_raw();
    assert_eq!(renderer.frosted_pool_states_for_test()[6],1,"raw export quarantines before extraction");
    let reconstructed=gpu.device.create_command_encoder(&Default::default()).finish();
    renderer.submit_compiled_target_for_test(crate::renderer::frosted_glass::EncodedTargetCommands::unmanaged(reconstructed));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(renderer.frosted_pool_states_for_test()[6],1);
    // Even rewrapping and genuinely submitting original raw A has no reusable
    // custody authority. Its exported entry remains charged, never reattached.
    renderer.submit_compiled_target_for_test(crate::renderer::frosted_glass::EncodedTargetCommands::unmanaged(raw_original));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(renderer.frosted_pool_states_for_test()[6],1,"late actual raw completion cannot reauthenticate an exported lease");
    let final_plan=renderer.compile_multi(&plan).unwrap().expect_ready("native Compile must be ready");
    assert_eq!(renderer.frosted_pool_for_test(),after);
    assert_eq!(renderer.frosted_pool_states_for_test()[6],1);
    let exported_then_dropped=renderer.encode_compiled_target_for_test(&final_plan,id,&target.create_view(&Default::default())).unwrap().into_raw();
    drop(exported_then_dropped);
    assert_eq!(renderer.frosted_pool_states_for_test()[6],2,"dropping a second raw export cannot recover its authority either");
    let protected=renderer.compile_multi(&plan).unwrap().expect_ready("native Compile must be ready");
    let charged=renderer.frosted_pool_for_test();
    assert!(charged.0>after.0 && charged.1>after.1,"both actual exported backings stay charged and unavailable");
    println!("HAL385_SEALED_EXPORT late_raw_submit_reauthenticates=false raw_drop_reclaims=false original_unconfirmed=2 before={before:?} held_after_unrelated={after:?} after_two_exports={charged:?}");
    drop(protected);drop(final_plan);drop(successor);
}

fn frost_decode(v:f64)->f64 {if v<=0.04045 {v/12.92}else{((v+0.055)/1.055).powf(2.4)}}
fn frost_encode(v:f64)->f64 {if v<=0.0031308 {12.92*v}else{1.055*v.powf(1.0/2.4)-0.055}}
fn frost_half(v:f64)->f64 {
    if v==0.0 {return v;}
    let step=2.0_f64.powf(v.abs().log2().floor().max(-14.0)-10.0);
    (v/step).round_ties_even()*step
}
fn frost_cpu_oracle(pixels:&mut [[f64;4]],width:usize,height:usize,scale:f32,
    rect:RectF,c:crate::core::paint::RoundedCorners,a:&crate::core::paint::FrostedGlassAppearance) {
    use crate::renderer::pipelines::analytic_surface::tests::{coating_reference,emission_reference,distance as sdf};
    let d=f64::from(scale);let sigma=f64::from(a.sigma_logical())*d;let radius=(sigma*3.0).ceil() as i32;
    let margin=2.0+f64::from(a.halo_extent())*d;
    let left=(f64::from(rect.x)*d-margin).floor().max(0.0) as usize;
    let top=(f64::from(rect.y)*d-margin).floor().max(0.0) as usize;
    let right=(f64::from(rect.x+rect.width)*d+margin).ceil().min(width as f64) as usize;
    let bottom=(f64::from(rect.y+rect.height)*d+margin).ceil().min(height as f64) as usize;
    let reach=if a.refraction_logical()==0.0 {0}else{(f64::from(a.refraction_logical()).abs()*d).ceil() as usize+1};
    let qleft=left.saturating_sub(reach);let qright=right.saturating_add(reach).min(width);
    let qtop=top.saturating_sub(reach);let qbottom=bottom.saturating_add(reach).min(height);
    let weights:Vec<_>=(0..=radius).map(|k|if k==0 {1.0}else{((-0.5*(k as f64/sigma).powi(2)).exp() as f32) as f64}).collect();
    let original=pixels.to_vec();let mut horizontal=vec![[0.0;4];pixels.len()];
    for y in 0..height {for x in qleft..qright {
        let mut total=0.0;let mut color=[0.0;4];
        for k in -radius..=radius {let q=x as i32+k;if q<0 || q>=width as i32 {continue;}let w=weights[k.unsigned_abs() as usize];total+=w;for ch in 0..4{color[ch]+=original[y*width+q as usize][ch]*w;}}
        horizontal[y*width+x]=color.map(|v|frost_half(v/total));
    }}
    let mut field=vec![[0.0;4];pixels.len()];
    for y in qtop..qbottom {for x in qleft..qright {
        let mut color=[0.0;4];let mut total=0.0;
        for k in -radius..=radius {let q=y as i32+k;if q<0 || q>=height as i32{continue;}
            let w=weights[k.unsigned_abs() as usize];total+=w;
            for ch in 0..4{color[ch]+=horizontal[q as usize*width+x][ch]*w;}}
        field[y*width+x]=color.map(|v|frost_half(v/total));
    }}
    let distance=|x:usize,y:usize| {
        let qx=(x as f64+0.5)/d-f64::from(rect.x+rect.width*0.5);
        let qy=(y as f64+0.5)/d-f64::from(rect.y+rect.height*0.5);
        let r=f64::from(if qx<0.0 {if qy<0.0 {c.tl}else{c.bl}}else if qy<0.0{c.tr}else{c.br});
        let vx=qx.abs()-f64::from(rect.width)*0.5+r;let vy=qy.abs()-f64::from(rect.height)*0.5+r;
        vx.max(0.0).hypot(vy.max(0.0))+vx.max(vy).min(0.0)-r
    };
    let tint=a.tint();
    for y in top..bottom {for x in left..right {
        let mut rgb=[tint.r as f64,tint.g as f64,tint.b as f64];
        if tint.a>0.0 && a.transmission()<1.0 {if let Some(grain)=a.micrograin() {
            rgb=crate::renderer::pipelines::analytic_surface::tests::micrograin_color(rgb,grain,
                [x as f64+0.5-f64::from(rect.x)*d,y as f64+0.5-f64::from(rect.y)*d]);
        }}
        let p=[rgb[0]*tint.a as f64,rgb[1]*tint.a as f64,rgb[2]*tint.a as f64,tint.a as f64];
        let point=Point::new(((x as f64+0.5)/d) as f32,((y as f64+0.5)/d) as f32);
        let (signed,gradient)=sdf(rect,c,point);
        let mut filtered=field[y*width+x];
        if a.refraction_logical()!=0.0 {
            let optics=a.glass_optics().unwrap().parameters();
            let u=(1.0-(-signed).max(0.0)/f64::from(optics.bevel_width)).clamp(0.0,1.0);
            let edge=u*u*(3.0-2.0*u);
            let displacement=if signed<0.0 {f64::from(a.refraction_logical())*4.0*edge*(1.0-edge)*d}else{0.0};
            let px=(x as f64+0.5+displacement*gradient[0]).clamp(0.5,width as f64-0.5)-0.5;
            let py=(y as f64+0.5+displacement*gradient[1]).clamp(0.5,height as f64-0.5)-0.5;
            let lx=px.floor() as usize;let ly=py.floor() as usize;let fx=px-px.floor();let fy=py-py.floor();
            let tap=|x:usize,y:usize| {
                let x=x.min(width-1);let y=y.min(height-1);
                assert!(x>=qleft && x<qright && y>=qtop && y<qbottom,"CPU proof: displaced taps remain inside padded Q");
                field[y*width+x]
            };
            let a=tap(lx,ly);let b=tap(lx+1,ly);let c=tap(lx,ly+1);let e=tap(lx+1,ly+1);
            filtered=std::array::from_fn(|ch|(1.0-fy)*((1.0-fx)*a[ch]+fx*b[ch])+fy*((1.0-fx)*c[ch]+fx*e[ch]));
        }
        let ddx=(distance(x|1,y)-distance(x&!1,y)).abs();let ddy=(distance(x,y|1)-distance(x,y&!1)).abs();
        let aa=(ddx+ddy).max(0.5/d);let u=((distance(x,y)+aa)/(2.0*aa)).clamp(0.0,1.0);let m=1.0-u*u*(3.0-2.0*u);
        let g: [f64;4]=std::array::from_fn(|ch|a.transmission() as f64*filtered[ch]+(1.0-a.transmission() as f64)*p[ch]);
        let coated=if let Some(optics)=a.glass_optics() {coating_reference(optics,
            [f64::from(point.x-rect.x)/f64::from(rect.width),f64::from(point.y-rect.y)/f64::from(rect.height)],signed,gradient,g)} else {g};
        let emission=if let Some(e)=a.surface_emission(){emission_reference(e,signed,gradient,m)}else{[0.0;4]};
        for ch in 0..4 {
            let body=(1.0-f64::from(a.coverage_opacity())*m)*original[y*width+x][ch]+f64::from(a.coverage_opacity())*m*coated[ch];
            let value=emission[ch]+(1.0-emission[3])*body;
            let encoded=if ch==3{value}else{frost_encode(value)};
            let quantized=(encoded.clamp(0.0,1.0)*255.0).round()/255.0;
            pixels[y*width+x][ch]=if ch==3 {quantized}else{frost_decode(quantized)};
        }
    }}
}

pub(super) fn read_compiled_surface(
    renderer: &WgpuRenderer,
    compiled: &crate::renderer::MultiTargetDrawPlan,
    surface_id: SurfaceId,
) -> Option<Vec<u8>> {
    let gpu = renderer.gpu_resources();
    let descriptor = compiled
        .get_target(surface_id)
        .expect("compiled target descriptor");
    let (width, height) = descriptor.physical_size;
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SPEC-HAL-359 output"),
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
    let render = renderer.encode_compiled_target_for_test(
        compiled,
        surface_id,
        &output.create_view(&wgpu::TextureViewDescriptor::default()),
    )?;

    let unpadded_bytes_per_row = width * 4;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("SPEC-HAL-359 readback"),
        size: u64::from(padded_bytes_per_row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SPEC-HAL-359 copy"),
        });
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
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

    let done = Arc::new(AtomicBool::new(false));
    let succeeded = Arc::new(AtomicBool::new(false));
    let done_for_callback = done.clone();
    let succeeded_for_callback = succeeded.clone();
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        succeeded_for_callback.store(result.is_ok(), Ordering::Release);
        done_for_callback.store(true, Ordering::Release);
    });
    while !done.load(Ordering::Acquire) {
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }
    assert!(
        succeeded.load(Ordering::Acquire),
        "SPEC-HAL-359 readback failed"
    );

    let mapped = slice.get_mapped_range().expect("mapped readback range");
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height as usize {
        let start = row * padded_bytes_per_row as usize;
        let end = start + unpadded_bytes_per_row as usize;
        for pixel in mapped[start..end].chunks_exact(4) {
            match gpu.format {
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                    rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
                    rgba.extend_from_slice(pixel);
                }
                format => panic!("unsupported SPEC-HAL-359 target format {format:?}"),
            }
        }
    }
    drop(mapped);
    readback.unmap();
    Some(rgba)
}

fn text_realization_scene(text: &str) -> SceneGraph {
    let mut scene = base_scene();
    scene.push_text_probe(
        Point::new(4.0, 8.0),
        text.to_owned(),
        18.0,
        Color::WHITE,
        1,
        None,
    );
    scene.build()
}

fn invalid_text_realization_scene() -> SceneGraph {
    let mut scene = text_realization_scene("A");
    let node = scene
        .root
        .iter_mut()
        .find_map(|node| match node {
            SceneNode::Text(text) => Some(text),
            _ => None,
        })
        .unwrap();
    node.font_size = f32::NAN;
    scene
}

#[cfg(target_os = "macos")]
#[test]
fn complete_text_realization_missing_cluster_metal_1x_2x() {
    use crate::core::frame_realization::{AppFrameAttempts, RequiredTextFaultGuard};
    use crate::core::text_engine::TextEngine;
    use crate::renderer::TextRealizationOutcome;
    use crate::renderer::text_realization::SurfaceTextDemand;
    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(192, 80, scale).expect("actual Metal required");
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.text_engine_mut().set_scale(scale);
        let source=["A e\u{1ab0} B","A e\u{1df5} B","A e\u{10efd} B"].into_iter().find(|source| {
            let run=renderer.text_engine_mut().shape(source,18.0);
            let ranges=renderer.text_engine_mut().verified_glyph_source_ranges(&run).unwrap();
            run.glyphs.iter().enumerate().any(|(i,g)|g.id==0 && run.glyphs.iter().enumerate().any(|(j,h)|h.id!=0 && ranges[i]==ranges[j]))
        }).expect("real shaper must expose a combining source cluster containing missing and valid glyphs");
        let scene = text_realization_scene(source);
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .unwrap();
        let demand = SurfaceTextDemand::gather(
            &plan.surface_data[&surface].primitives,
            renderer.text_engine_mut(),
            scale,
        );
        assert!(demand.failure.is_none());
        let run = &demand.runs[0];
        assert_eq!(run.glyph_run.text, source);
        assert_eq!(
            run.markers.len(),
            1,
            "one marker for the whole actual source cluster"
        );
        assert!(
            run.keys
                .iter()
                .enumerate()
                .any(|(i, key)| key.glyph_id != 0 && run.suppressed[i]),
            "supported member of missing cluster is suppressed too"
        );
        let marker = run.markers[0];
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let result = compiled.text_realization(surface).unwrap();
        assert!(matches!(
            result.outcome(),
            TextRealizationOutcome::Degraded {
                missing_cluster_count: 1,
                ..
            }
        ));
        assert!(!result.outcome().is_complete());
        assert!(result.is_prepared_for_output());
        let mut guard = RequiredTextFaultGuard::default();
        let mut attempts = AppFrameAttempts::new();
        let attempt = attempts.next();
        guard.compiled(attempt, &compiled);
        guard.assert_output_allowed(attempt, surface);
        let pixels = read_compiled_surface(&renderer, &compiled, surface).unwrap();
        let width = (192.0 * scale) as usize;
        let x = ((4.0 + marker.0) * scale).floor() as usize;
        assert!(
            pixels
                .chunks_exact(4)
                .enumerate()
                .any(|(i, p)| i % width >= x
                    && i % width <= x + ((marker.1 - marker.0) * scale).ceil() as usize
                    && p[0] > 100),
            "actual marker pixels"
        );
        assert!(
            pixels
                .chunks_exact(4)
                .enumerate()
                .any(|(i, p)| i % width < x && p[0] > 100),
            "healthy text on same surface remains visible"
        );
        guard.submitted_and_presented(attempt, surface);
        let mut notices = Vec::new();
        guard.write_diagnostics(&mut notices).unwrap();
        let notices = String::from_utf8(notices).unwrap();
        assert!(notices.contains("disposition=Degraded"));
        assert!(!notices.contains("recovered"));
        let again = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let attempt = attempts.next();
        guard.compiled(attempt, &again);
        guard.submitted_and_presented(attempt, surface);
        let mut notices = Vec::new();
        guard.write_diagnostics(&mut notices).unwrap();
        assert!(notices.is_empty());
        let healthy = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, text_realization_scene("A B"))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .unwrap();
        let healthy = renderer
            .compile_multi(&healthy)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let attempt = attempts.next();
        guard.compiled(attempt, &healthy);
        let mut notices = Vec::new();
        guard.write_diagnostics(&mut notices).unwrap();
        assert!(notices.is_empty(), "Compile alone is not recovery");
        assert!(read_compiled_surface(&renderer, &healthy, surface).is_some());
        guard.submitted_and_presented(attempt, surface);
        guard.write_diagnostics(&mut notices).unwrap();
        assert!(
            String::from_utf8(notices)
                .unwrap()
                .contains("event=recovered")
        );
    }
}

fn compile_text_realization_scenes(
    renderer: &mut WgpuRenderer,
    compositor: &Compositor,
    scenes: HashMap<SurfaceId, SceneGraph>,
) -> crate::renderer::MultiTargetDrawPlan {
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready")
}

fn add_text_realization_surface(
    renderer: &mut WgpuRenderer,
    compositor: &mut Compositor,
    z: i32,
) -> SurfaceId {
    let id = compositor.create_surface(
        z,
        RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        SceneKey::BASE,
    );
    renderer
        .register_headless_render_target(id, (WIDTH, HEIGHT), 1.0)
        .unwrap();
    id
}

#[test]
fn complete_text_realization_distinguishes_no_ink_missing_and_empty_surfaces() {
    use crate::core::frame_realization::{AppFrameAttempts, RequiredTextFaultGuard};
    use crate::renderer::{
        TextRealizationFailureReason as Reason, TextRealizationOutcome as Outcome,
    };
    let (mut renderer, mut compositor, good) = setup().expect("SPEC-HAL-360 GPU required");
    let bad = add_text_realization_surface(&mut renderer, &mut compositor, 1);
    let empty = add_text_realization_surface(&mut renderer, &mut compositor, 2);
    let compiled = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([
            (good, text_realization_scene("A A")),
            (bad, text_realization_scene("A\u{10ffff}")),
        ]),
    );
    // Genuine failed Compile is reported without constructing an optional recorder/observer.
    let attempt = AppFrameAttempts::new().next();
    let mut faults = RequiredTextFaultGuard::default();
    faults.compiled(attempt, &compiled);
    faults.assert_output_allowed(attempt, good);
    faults.assert_output_allowed(attempt, empty);
    let mut diagnostic = Vec::new();
    faults.write_diagnostics(&mut diagnostic).unwrap();
    let diagnostic = String::from_utf8(diagnostic).unwrap();
    assert_eq!(diagnostic.lines().count(), 1);
    assert!(diagnostic.contains("event=first"));
    assert!(diagnostic.contains("reason=MissingGlyph"));
    let result = compiled.text_realization(good).unwrap();
    assert_eq!(result.surface_id(), good);
    assert_eq!(result.frame_epoch(), renderer.frame_epoch());
    assert_eq!(
        *result.outcome(),
        Outcome::Complete {
            primitive_count: 1,
            glyph_count: 3,
            no_ink_glyph_count: 1
        }
    );
    assert!(matches!(compiled.text_realization(bad).unwrap().outcome(),
        Outcome::Degraded {first_failure,missing_cluster_count:1,..} if first_failure.reason == Reason::MissingGlyph));
    assert_eq!(
        *compiled.text_realization(empty).unwrap().outcome(),
        Outcome::Complete {
            primitive_count: 0,
            glyph_count: 0,
            no_ink_glyph_count: 0
        }
    );
    assert!(
        read_compiled_surface(&renderer, &compiled, good)
            .unwrap()
            .chunks_exact(4)
            .any(|p| p[0] > 100)
    );
    faults.assert_output_allowed(attempt, bad);
    assert!(read_compiled_surface(&renderer, &compiled, bad).is_some());
    assert!(
        read_compiled_surface(&renderer, &compiled, empty)
            .unwrap()
            .chunks_exact(4)
            .all(|p| p[..3] == [0, 0, 0])
    );

    let only_bad = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([
            (good, invalid_text_realization_scene()),
            (bad, invalid_text_realization_scene()),
            (empty, invalid_text_realization_scene()),
        ]),
    );
    assert!(
        renderer.render_multi(only_bad).is_empty(),
        "refusal must precede headless target acquisition"
    );
}

#[test]
fn frame_realization_preserves_exact_multi_surface_compile_without_fabricating_output() {
    use crate::core::frame_realization::{
        AppFrameAttempts, AppFrameCompileOutcome, AppFrameRecorder,
    };
    use crate::renderer::TextRealizationOutcome;
    let (mut renderer, mut compositor, good) = setup().expect("SPEC-HAL-361 GPU required");
    let bad = add_text_realization_surface(&mut renderer, &mut compositor, 1);
    let empty = add_text_realization_surface(&mut renderer, &mut compositor, -1);
    let mut attempts = AppFrameAttempts::new();
    let attempt = attempts.next();
    let mut recorder = AppFrameRecorder::new(
        attempt,
        compositor.snapshot_multi().iter().map(|surface| surface.id),
    );
    let compiled = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([
            (good, text_realization_scene("A A")),
            (bad, text_realization_scene("A\u{10ffff}")),
        ]),
    );
    let epoch = renderer.frame_epoch();
    recorder.compiled(&compiled);
    let _successor = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([(good, text_realization_scene("B"))]),
    );
    assert!(
        renderer.render_multi(compiled).is_empty(),
        "superseded plan has no output"
    );
    let receipt = recorder.finish();
    assert_eq!(receipt.attempt_id(), attempt);
    assert_eq!(receipt.compile_outcome(), &AppFrameCompileOutcome::Compiled);
    assert_eq!(
        receipt
            .surfaces()
            .iter()
            .map(|surface| surface.surface_id())
            .collect::<Vec<_>>(),
        vec![empty, good, bad]
    );
    assert!(
        receipt
            .surface(good)
            .unwrap()
            .text_realization()
            .unwrap()
            .outcome()
            .is_complete()
    );
    assert!(matches!(
        receipt
            .surface(bad)
            .unwrap()
            .text_realization()
            .unwrap()
            .outcome(),
        TextRealizationOutcome::Degraded { .. }
    ));
    assert!(matches!(
        receipt
            .surface(empty)
            .unwrap()
            .text_realization()
            .unwrap()
            .outcome(),
        TextRealizationOutcome::Complete {
            primitive_count: 0,
            ..
        }
    ));
    for surface in receipt.surfaces() {
        let text = surface.text_realization().unwrap();
        assert_eq!(text.surface_id(), surface.surface_id());
        assert_eq!(text.frame_epoch(), epoch);
        assert!(!surface.output_produced());
        assert!(!surface.submitted());
        assert!(!surface.presented_called());
    }
}

#[test]
fn ui_scale_inactive_and_deferred_primary_preserve_secondary_compile() {
    let (mut renderer, mut compositor, primary) =
        setup().expect("UI scale target proof requires GPU");
    let secondary = add_text_realization_surface(&mut renderer, &mut compositor, 1);
    renderer.resize_render_target(primary, (0, HEIGHT)).unwrap();
    compositor.set_surface_bounds(primary, RectF::new(0.0, 0.0, 0.0, 0.0));
    assert!(
        compositor
            .snapshot_multi()
            .get(primary)
            .unwrap()
            .is_visible()
    );
    assert!(!renderer.target_has_active_extent(primary));
    assert!(renderer.custom_render_target_snapshot(primary).is_err());
    let scenes = || {
        HashMap::from([
            (primary, text_realization_scene("A")),
            (secondary, text_realization_scene("B")),
        ])
    };
    let inactive = compile_text_realization_scenes(&mut renderer, &compositor, scenes());
    assert_eq!(
        inactive
            .target_descriptors
            .iter()
            .map(|target| target.surface_id)
            .collect::<Vec<_>>(),
        vec![secondary]
    );
    assert!(inactive.text_realization(primary).is_none());
    assert!(
        inactive
            .text_realization(secondary)
            .unwrap()
            .outcome()
            .is_complete()
    );

    renderer
        .resize_render_target(primary, (WIDTH, HEIGHT))
        .unwrap();
    compositor.set_surface_bounds(primary, RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32));
    assert!(
        renderer.render_multi(inactive).is_empty(),
        "resize retires prior preparation"
    );
    assert!(renderer.target_has_active_extent(primary));
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes(),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    let deferred = renderer
        .compile_multi_for_surfaces(&plan, &std::collections::HashSet::from([secondary]))
        .unwrap()
        .expect_ready("native Compile must be ready");
    assert_eq!(
        deferred
            .target_descriptors
            .iter()
            .map(|target| target.surface_id)
            .collect::<Vec<_>>(),
        vec![secondary]
    );
    assert!(
        compositor
            .snapshot_multi()
            .get(primary)
            .unwrap()
            .is_visible()
    );
    let resumed = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    assert_eq!(resumed.target_descriptors.len(), 2);
    assert!(
        resumed
            .text_realization(primary)
            .unwrap()
            .outcome()
            .is_complete()
    );
    assert!(
        resumed
            .text_realization(secondary)
            .unwrap()
            .outcome()
            .is_complete()
    );
}

#[test]
fn complete_text_realization_preserves_pixels_across_growth_and_later_surface_pressure() {
    let (mut renderer, mut compositor, first) = setup().expect("SPEC-HAL-360 GPU required");
    renderer.set_text_atlas_limits_for_test(64, 512);
    let control = render_scene(
        &mut renderer,
        &compositor,
        first,
        text_realization_scene("AB"),
    );
    assert_eq!(renderer.text_atlas_size_for_test(), (64, 64));
    assert!(control.chunks_exact(4).any(|p| p[0] > 100));

    let second = add_text_realization_surface(&mut renderer, &mut compositor, 1);
    let compiled = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([
            (first, text_realization_scene("AB")),
            (
                second,
                text_realization_scene("CDEFGHIJKLMNOPQRSTUVWXYZ0123456789"),
            ),
        ]),
    );
    assert!(
        renderer.text_atlas_size_for_test().0 > 64,
        "control must exercise actual growth"
    );
    assert!(
        compiled
            .text_realization(first)
            .unwrap()
            .outcome()
            .is_complete()
    );
    assert!(
        compiled
            .text_realization(second)
            .unwrap()
            .outcome()
            .is_complete()
    );
    assert_eq!(
        read_compiled_surface(&renderer, &compiled, first).unwrap(),
        control
    );
    assert!(
        read_compiled_surface(&renderer, &compiled, second)
            .unwrap()
            .chunks_exact(4)
            .any(|p| p[0] > 100)
    );
}

#[test]
fn complete_text_realization_preserves_admitted_pixels_during_eviction_and_capacity_refusal() {
    use crate::renderer::{
        TextRealizationFailureReason as Reason, TextRealizationOutcome as Outcome,
    };
    let (mut renderer, mut compositor, first) = setup().expect("SPEC-HAL-360 GPU required");
    renderer.set_text_atlas_limits_for_test(64, 64);
    let control = render_scene(
        &mut renderer,
        &compositor,
        first,
        text_realization_scene("A"),
    );
    render_scene(
        &mut renderer,
        &compositor,
        first,
        text_realization_scene("Z"),
    );
    let (_, z_keys) = renderer.text_engine_mut().shape_with_keys("Z", 18.0);
    assert!(renderer.text_atlas_contains_for_test(&z_keys[0]));

    let pressure = add_text_realization_surface(&mut renderer, &mut compositor, 1);
    let independent = add_text_realization_surface(&mut renderer, &mut compositor, 2);
    let compiled = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([
            (first, text_realization_scene("A")),
            (pressure, text_realization_scene("BCDEFGHIJKLMN")),
            (independent, text_realization_scene("B")),
        ]),
    );
    assert!(
        matches!(compiled.text_realization(pressure).unwrap().outcome(),
        Outcome::Unavailable(failure) if failure.reason == Reason::AtlasCapacity)
    );
    assert!(
        !renderer.text_atlas_contains_for_test(&z_keys[0]),
        "control must actually evict an unprotected glyph"
    );
    assert_eq!(
        read_compiled_surface(&renderer, &compiled, first).unwrap(),
        control
    );
    assert!(read_compiled_surface(&renderer, &compiled, pressure).is_none());
    assert!(
        compiled
            .text_realization(independent)
            .unwrap()
            .outcome()
            .is_complete()
    );
    assert!(
        read_compiled_surface(&renderer, &compiled, independent)
            .unwrap()
            .chunks_exact(4)
            .any(|p| p[0] > 100)
    );
}

#[test]
fn complete_text_realization_rejects_superseded_plans_and_cleared_atlas() {
    let (mut renderer, compositor, surface) = setup().expect("SPEC-HAL-360 GPU required");
    let old = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([(surface, text_realization_scene("A"))]),
    );
    let current = compile_text_realization_scenes(
        &mut renderer,
        &compositor,
        HashMap::from([(surface, text_realization_scene("B"))]),
    );
    assert!(read_compiled_surface(&renderer, &old, surface).is_none());
    assert!(renderer.render_multi(old).is_empty());
    assert!(read_compiled_surface(&renderer, &current, surface).is_some());
    renderer.set_scale_factor(surface, 2.0);
    assert!(read_compiled_surface(&renderer, &current, surface).is_none());
    assert!(renderer.render_multi(current).is_empty());
}

fn push_full_rect(builder: &mut SceneBuilder, color: Color, z_index: u32) {
    builder.push_rect(
        RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        color,
        z_index,
        None,
    );
}

fn push_probe_text(builder: &mut SceneBuilder, z_index: u32) {
    builder.push_text_probe(
        Point::new(12.0, 40.0),
        "HALCYON".to_string(),
        28.0,
        Color::WHITE,
        z_index,
        None,
    );
}

fn push_full_path(builder: &mut SceneBuilder, color: Color, z_index: u32) {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(WIDTH as f32, 0.0);
    path.line_to(WIDTH as f32, HEIGHT as f32);
    path.line_to(0.0, HEIGHT as f32);
    path.close();
    builder.push_path(path, color, z_index, None);
}

#[test]
fn text_only_surface_renders_without_background_geometry() {
    let Some((mut renderer, compositor, surface_id)) = setup() else {
        return;
    };
    let mut builder = SceneBuilder::new();
    push_probe_text(&mut builder, 1);

    let pixels = render_scene(&mut renderer, &compositor, surface_id, builder.build());

    assert!(
        pixels
            .chunks_exact(4)
            .any(|pixel| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0),
        "text-only surface must produce visible glyph coverage"
    );
}

fn find_popup_row(
    nodes: &[SceneNode],
    translation: Point,
) -> Option<(&TextNode, &crate::core::scene::TextBackgroundSpan, Point)> {
    nodes.iter().find_map(|node| match node {
        SceneNode::Text(text) if text.z_index == LAYER_OVERLAY + 2 => text
            .backgrounds
            .iter()
            .find(|background| background.width >= 150.0 && background.height <= 24.0)
            .map(|background| (text, background, translation)),
        SceneNode::RetainedFragment(fragment) => find_popup_row(&fragment.children, translation),
        SceneNode::Clip(clip) => find_popup_row(&clip.children, translation),
        SceneNode::Layer(layer) => find_popup_row(&layer.children, translation),
        SceneNode::Scroll(scroll) => find_popup_row(
            &scroll.children,
            Point::new(
                translation.x - scroll.offset.0,
                translation.y - scroll.offset.1,
            ),
        ),
        _ => None,
    })
}

fn pixel_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    pixels[offset..offset + 4].try_into().expect("RGBA pixel")
}

#[test]
fn surface_fill_gradient_preserves_linear_color_and_fractional_alpha() {
    use crate::core::paint::{
        ColorStop, GradientDef, GradientKind, PaintSink, SurfaceFill, SurfaceGradient,
    };

    let (mut renderer, compositor, surface) = setup().expect("headless material test adapter");
    let color = crate::core::paint::Color::from_srgb(0.5, 0.7, 0.9, 0.5);
    let mut scene = base_scene();
    scene.push_rect(RectF::new(0.0, 0.0, 48.0, 64.0), color.into(), 1, None);
    scene.push_surface_fill(
        RectF::new(48.0, 0.0, 48.0, 64.0),
        &SurfaceFill::Gradient(
            SurfaceGradient::new(GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop { offset: 0.0, color },
                    ColorStop { offset: 1.0, color },
                ],
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
            })
            .unwrap(),
        ),
        1,
        None,
    );
    let pixels = render_scene(&mut renderer, &compositor, surface, scene.build());
    let solid = pixel_at(&pixels, WIDTH, 24, 32);
    let gradient = pixel_at(&pixels, WIDTH, 72, 32);
    for channel in 0..4 {
        assert!(
            solid[channel].abs_diff(gradient[channel]) <= 1,
            "identical linear colors must agree (gradient dither <= one byte): {solid:?} vs {gradient:?}"
        );
    }
}

#[test]
fn surface_fill_registered_image_uses_existing_premultiplied_pipeline() {
    use crate::core::paint::{Color as PaintColor, PaintSink, SurfaceFill, SurfaceImage};
    let (mut renderer, compositor, surface) = setup().expect("headless material test adapter");
    let gpu = renderer.gpu_resources();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SurfaceFill current Image binding"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&[0x3800u16, 0, 0, 0x3800]),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    renderer.register_image_from_view(
        263_040,
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
    );
    let mut scene = base_scene();
    scene.push_surface_fill(
        RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        &SurfaceFill::Image(SurfaceImage {
            source: 263_040,
            uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
            tint: PaintColor::from_linear(1.0, 1.0, 1.0, 0.5),
        }),
        1,
        None,
    );
    let pixels = render_scene(&mut renderer, &compositor, surface, scene.build());
    assert_eq!(pixel_at(&pixels, WIDTH, 48, 32), [137, 0, 0, 255]);
}

#[test]
fn surface_material_gradient_respects_current_rounded_clip_in_main_and_overlay() {
    use crate::core::paint::{ColorStop, GradientDef, GradientKind};
    use crate::core::scene::RoundedCorners;

    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, surface) = setup_with_size_and_scale(WIDTH, HEIGHT, scale)
            .expect("headless material test adapter");
        renderer.set_gpu_rounded_clips_enabled(true);
        for z in [1, LAYER_OVERLAY + 1] {
            let render = |renderer: &mut WgpuRenderer, gradient| {
                let mut scene = base_scene();
                scene.begin_clip(
                    RectF::new(8.0, 8.0, 80.0, 48.0),
                    Some(RoundedCorners {
                        tl: 18.0,
                        tr: 0.0,
                        bl: 6.0,
                        br: 14.0,
                    }),
                );
                let rect = RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
                if gradient {
                    scene.push_gradient_rect(
                        rect,
                        0.0,
                        GradientDef {
                            kind: GradientKind::Radial,
                            stops: vec![
                                ColorStop {
                                    offset: 0.0,
                                    color: crate::core::paint::Color::WHITE,
                                },
                                ColorStop {
                                    offset: 1.0,
                                    color: crate::core::paint::Color::WHITE,
                                },
                            ],
                            p0: (0.5, 0.5),
                            p1: (1.0, 0.5),
                        },
                        z,
                        None,
                    );
                } else {
                    scene.push_rect(rect, Color::WHITE, z, None);
                }
                scene.end_clip();
                render_scene_at_scale(renderer, &compositor, surface, scene.build(), scale)
            };
            let expected = render(&mut renderer, false);
            let actual = render(&mut renderer, true);
            assert_eq!(actual.len(), expected.len());
            for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                assert!(
                    actual.abs_diff(*expected) <= 1,
                    "rounded coverage differs at byte {index}, scale {scale}, z {z}: {actual} vs {expected}"
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn mail_outline_real_textfield_metal_1x_2x() {
    use crate::core::event::{InteractionDomains, KeyboardAuthorityMode};
    use crate::widgets::text_field::TextField;

    fn carets(nodes: &[SceneNode], out: &mut Vec<RectF>) {
        for node in nodes {
            match node {
                SceneNode::Text(n) => out.extend(n.backgrounds.iter().map(|span| {
                    RectF::new(
                        n.position.x + span.x_offset,
                        n.position.y + span.y_offset,
                        span.width,
                        span.height,
                    )
                })),
                SceneNode::Clip(n) => carets(&n.children, out),
                SceneNode::RetainedFragment(n) => carets(&n.children, out),
                _ => {}
            }
        }
    }
    let directory = std::path::PathBuf::from(format!(
        "target/evidence/mail-outline/metal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(256, 64, scale).unwrap();
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.text_engine_mut().set_scale(scale);
        let theme = Theme::dark();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                TextField::new()
                    .with_placeholder("Your email")
                    .with_leading_icon(IconId::MAIL),
            ),
            None,
            0,
        );
        let owner = tree.node(root).unwrap().id;
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        macro_rules! frame {
            () => {
                harness.frame_with_optional_custom_render_target(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(256.0, 64.0)),
                    &theme,
                    renderer.text_engine_mut(),
                    &builtin_registry(),
                    Some(target),
                    Some((&domains, WindowId::PRIMARY)),
                )
            };
        }
        frame!();
        let rect = tree.get_absolute_paint_rect(root).unwrap();
        let size = theme.typography.input_font_size;
        let left = crate::core::math::snap_to_physical(
            rect.x + theme.geometry.text_field.padding_x,
            scale,
        );
        let top = crate::core::math::snap_to_physical(rect.y + (rect.height - size) * 0.5, scale);
        let icon_center = Point::new(left + size * 0.5, top + size * 0.5);
        assert_eq!(
            tree.hit_test(icon_center.x, icon_center.y)
                .unwrap()
                .widget_id,
            owner
        );
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: icon_center,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: icon_center,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        ] {
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
        harness.advance_transition_time(0.010);
        let frame = frame!();
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        let mut caret = Vec::new();
        carets(&frame.scene_root, &mut caret);
        assert_eq!(caret.len(), 1, "one genuine caret in the empty field");
        assert!(
            caret[0].x >= left + size + theme.geometry.text_field.padding_x - 1.0 / scale,
            "MAIL must not touch the real text viewport/caret: {caret:?}"
        );
        let pixels = render_scene_at_scale(
            &mut renderer,
            &compositor,
            surface,
            control_order_scene(&frame),
            scale,
        );
        let width = (256.0 * scale) as u32;
        let mut ppm = format!("P6\n{width} {}\n255\n", (64.0 * scale) as u32).into_bytes();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(directory.join(format!("captured-{scale}.ppm")), ppm).unwrap();
        let sample = |x: f32, y: f32| {
            pixel_at(
                &pixels,
                width,
                ((left + x * size / 24.0) * scale) as u32,
                ((top + y * size / 24.0) * scale) as u32,
            )
        };
        for (x, y) in [(7.5, 15.0), (12.0, 16.0), (16.5, 15.0), (12.0, 8.0)] {
            let actual = sample(x, y);
            let expected = pixel_at(
                &pixels,
                width,
                ((rect.right() - 10.0) * scale) as u32,
                ((top + y * size / 24.0) * scale) as u32,
            );
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "MAIL interior is filled at ({x},{y}), DPR{scale}: actual={actual:?} face={expected:?}; captures={directory:?}"
            );
        }
        for (x0, y0, x1, y1) in [
            (3.0, 7.0, 5.0, 17.0),
            (19.0, 7.0, 21.0, 17.0),
            (5.0, 17.0, 19.0, 19.0),
            (7.0, 8.0, 17.0, 14.0),
        ] {
            let mut ink = 0;
            for y in ((top + y0 * size / 24.0) * scale).floor() as u32
                ..((top + y1 * size / 24.0) * scale).ceil() as u32
            {
                for x in ((left + x0 * size / 24.0) * scale).floor() as u32
                    ..((left + x1 * size / 24.0) * scale).ceil() as u32
                {
                    ink += usize::from(pixel_at(&pixels, width, x, y)[0] > 180);
                }
            }
            assert!(
                ink > 0,
                "missing frame/flap ink at DPR{scale}, region ({x0},{y0},{x1},{y1})"
            );
        }
        let caret_pixel = pixel_at(
            &pixels,
            width,
            (caret[0].x * scale).round() as u32,
            ((caret[0].y + caret[0].height * 0.5) * scale).round() as u32,
        );
        assert!(
            caret_pixel[0] > 180,
            "the original separate caret must remain visible"
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn input_adornment_states_and_icons_real_metal_1x_2x() {
    use crate::core::appearance::InputSurfaceAppearance;
    use crate::core::event::{InteractionDomains, PointerButtonsState};
    use crate::core::layout::EdgeInsets;
    use crate::core::paint::{
        Color as PaintColor, ColorStop, GradientDef, GradientKind, SurfaceGradient,
    };
    use crate::widgets::text_field::TextField;

    let gradient = |color| {
        SurfaceGradient::new(GradientDef {
            kind: GradientKind::Linear,
            stops: vec![
                ColorStop { offset: 0.0, color },
                ColorStop { offset: 1.0, color },
            ],
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
        })
        .unwrap()
    };
    let clear = PaintColor::TRANSPARENT;
    let face = PaintColor::from_hex(0x181A1C);
    let appearance = InputSurfaceAppearance::try_new(
        gradient(clear),
        clear,
        clear,
        clear,
        face,
        PaintColor::from_hex(0x454749),
        14.0,
    )
    .unwrap()
    .with_state_surfaces(
        gradient(face.with_alpha(0.4)),
        gradient(face.with_alpha(0.8)),
    );
    let directory = std::path::PathBuf::from(format!(
        "target/evidence/input-adornment/metal-{}",
        std::process::id(),
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(200, 120, scale).unwrap();
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.text_engine_mut().set_scale(scale);
        let mut combo = ComboBox::simple(vec!["RU".into(), "EN".into()])
            .with_input_surface_appearance(appearance.clone())
            .with_leading_icon(IconId::GLOBE);
        combo.set_input_text("RU".into()).unwrap();
        combo.set_text_input_enabled(false);
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets::all(8.0))),
            None,
            0,
        );
        let combo = tree.add_child(root, Box::new(combo), None, 0);
        let field = tree.add_child(
            root,
            Box::new(
                TextField::new()
                    .with_text("Почта")
                    .with_leading_icon(IconId::MAIL),
            ),
            None,
            1,
        );
        let field_owner = tree.node(field).unwrap().id;
        let combo_owner = tree.node(combo).unwrap().id;
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        let mut samples = Vec::new();
        let mut unchanged_field = None;
        for state in ["idle", "hover", "active"] {
            if state == "hover" {
                let rect = tree.get_absolute_paint_rect(combo).unwrap();
                router.route_input_event(
                    &mut tree,
                    FusedInputEvent::PointerMove {
                        window_id: WindowId::PRIMARY,
                        time: std::time::Instant::now(),
                        pos: crate::core::math::Point::new(
                            rect.x + rect.width * 0.5,
                            rect.y + rect.height * 0.5,
                        ),
                        delta: (0.0, 0.0),
                        device: PointerDevice::Mouse,
                        buttons: PointerButtonsState::default(),
                    },
                    Some(root),
                    renderer.text_engine_mut(),
                    Some(harness.animation_context()),
                    domains.primary_state_mut(),
                    &mut bindings,
                );
            } else if state == "active" {
                router.request_focus_for_widget(
                    &mut tree,
                    combo_owner,
                    domains.primary_state_mut(),
                );
            }
            let frame = harness.frame_with_optional_custom_render_target(
                &mut tree,
                root,
                Constraints::tight(Size::new(200.0, 120.0)),
                &Theme::light(),
                renderer.text_engine_mut(),
                &builtin_registry(),
                Some(target),
                Some((&domains, WindowId::PRIMARY)),
            );
            let field_local = harness.local_fragment(field_owner).unwrap();
            let icons = builtin_registry();
            let mail = icons.get(IconId::MAIL).expect("registered native envelope");
            assert_eq!(mail.name, "mail");
            assert_eq!(icons.len(), 20);
            if let Some(previous) = &unchanged_field {
                assert!(std::sync::Arc::ptr_eq(previous, &field_local));
            } else {
                unchanged_field = Some(field_local);
            }
            let mut scene = control_order_scene(&frame);
            scene.clear_color = Color::WHITE;
            let pixels = render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
            let width = (200.0 * scale) as u32;
            let field_rect = tree.get_absolute_paint_rect(field).unwrap();
            let theme = Theme::light();
            let size = theme.typography.input_font_size;
            let left = field_rect.x + theme.geometry.text_field.padding_x;
            let top = field_rect.y + (field_rect.height - size) * 0.5;
            let dark_ink = ((top * scale).ceil() as u32..((top + size) * scale).floor() as u32)
                .flat_map(|y| {
                    ((left * scale).ceil() as u32..((left + size) * scale).floor() as u32)
                        .map(move |x| (x, y))
                })
                .filter(|&(x, y)| pixel_at(&pixels, width, x, y)[0] < 128)
                .count();
            assert!(
                dark_ink >= (8.0 * scale) as usize,
                "real in-field MAIL ink at DPR {scale}"
            );
            let rect = tree.get_absolute_paint_rect(combo).unwrap();
            samples.push(
                pixel_at(
                    &pixels,
                    width,
                    ((rect.x + rect.width * 0.5) * scale) as u32,
                    ((rect.y + 3.0) * scale) as u32,
                )[0],
            );
            let mut ppm = format!("P6\n{width} {}\n255\n", (120.0 * scale) as u32).into_bytes();
            for pixel in pixels.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(directory.join(format!("{state}-{scale}.ppm")), ppm).unwrap();
        }
        assert_eq!(samples[0], 255, "idle alpha reveals the real backdrop");
        assert!(
            samples[0] > samples[1] && samples[1] > samples[2],
            "actual Metal state surfaces: {samples:?}"
        );
    }
    eprintln!("input-adornment Metal readbacks: {}", directory.display());
}

#[cfg(target_os = "macos")]
#[test]
fn combo_product_policy_and_contour_real_metal() {
    use crate::core::appearance::InputSurfaceAppearance;
    use crate::core::event::{InteractionDomains, KeyboardAuthorityMode};
    use crate::core::layout::EdgeInsets;
    use crate::core::paint::{
        Color as PaintColor, ColorStop, GradientDef, GradientKind, PaintSink, SurfaceFill,
        SurfaceGradient, SurfaceImage,
    };
    use crate::core::popup::{PopupInteractionPolicy, PopupWidthPolicy};
    use crate::core::scene::RoundedCorners;
    use crate::widgets::text_field::TextField;

    fn has_border(nodes: &[SceneNode], color: Color) -> bool {
        nodes.iter().any(|node| match node {
            SceneNode::BorderRect(border) => border.border_color == color,
            SceneNode::RetainedFragment(fragment) => has_border(&fragment.children, color),
            SceneNode::Clip(clip) => has_border(&clip.children, color),
            SceneNode::Scroll(scroll) => has_border(&scroll.children, color),
            SceneNode::Layer(layer) => has_border(&layer.children, color),
            _ => false,
        })
    }
    fn save(directory: &std::path::Path, name: &str, pixels: &[u8], width: u32) {
        let height = pixels.len() / 4 / width as usize;
        let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
    }
    fn assert_smooth_tangents(pixels: &[u8], width: u32, rect: RectF, radius: f32, scale: f32) {
        let left = (rect.x * scale).round() as u32;
        let top = (rect.y * scale).round() as u32;
        let right = (rect.right() * scale).round() as u32 - 1;
        let bottom = (rect.bottom() * scale).round() as u32 - 1;
        let horizontal = ((radius + 1.0) * scale).ceil() as u32;
        // The pill's vertical tangent is at its midpoint. Stop before crossing it.
        let vertical = horizontal.min((bottom - top) / 2);
        for (x, y, dx, dy, length) in [
            (left, top, 1_i32, 0_i32, horizontal),
            (right, top, -1, 0, horizontal),
            (left, bottom, 1, 0, horizontal),
            (right, bottom, -1, 0, horizontal),
            (left, top, 0, 1, vertical),
            (right, top, 0, 1, vertical),
            (left, bottom, 0, -1, vertical),
            (right, bottom, 0, -1, vertical),
        ] {
            let mut previous = 255_u8;
            for step in 0..=length {
                let px = (x as i32 + dx * step as i32) as u32;
                let py = (y as i32 + dy * step as i32) as u32;
                let red = pixel_at(pixels, width, px, py)[0];
                // Dark on white must gain coverage continuously toward the flat
                // edge. The former outside-corner/inside-edge AA jumped upward.
                assert!(
                    red <= previous.saturating_add(2),
                    "tangent coverage bump at {px},{py}, scale {scale}: {previous} -> {red}"
                );
                previous = red;
            }
        }
        assert!(
            pixel_at(
                pixels,
                width,
                (left + right) / 2,
                top + (2.0 * scale) as u32
            )[0] < 80,
            "an opaque dark face, not an empty clip"
        );
    }

    let directory = std::path::PathBuf::from(format!(
        "target/evidence/combo-product-policy/metal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let face = PaintColor::from_hex(0x171E25);
    let focused = PaintColor::from_hex(0x9A958E);
    let captured = PaintColor::from_hex(0xE2D5BF);
    let gradient = GradientDef {
        kind: GradientKind::Linear,
        stops: vec![
            ColorStop {
                offset: 0.0,
                color: face,
            },
            ColorStop {
                offset: 1.0,
                color: face,
            },
        ],
        p0: (0.0, 0.0),
        p1: (0.0, 1.0),
    };
    let appearance = InputSurfaceAppearance::try_new(
        SurfaceGradient::new(gradient.clone()).unwrap(),
        PaintColor::TRANSPARENT,
        PaintColor::TRANSPARENT,
        PaintColor::TRANSPARENT,
        face,
        PaintColor::from_hex(0x3B444B),
        14.0,
    )
    .unwrap()
    .try_with_focus_colors(focused, captured)
    .unwrap();
    let mut receipts = Vec::new();
    for scale in [2.0_f32, 1.64] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(200, 200, scale).expect("real Metal required");
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.text_engine_mut().set_scale(scale);
        let width = (200.0 * scale) as u32;
        let mut combo = ComboBox::simple(vec!["RU".into(), "EN".into()])
            .with_input_surface_appearance(appearance.clone())
            .with_interaction_policy(PopupInteractionPolicy {
                width: PopupWidthPolicy::MatchTrigger,
            })
            .with_leading_icon(IconId::GLOBE);
        combo.set_input_text("RU".into()).unwrap();
        combo.set_text_input_enabled(false);
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets {
                left: 8.0,
                top: 8.0,
                right: 96.0,
                bottom: 0.0,
            })),
            None,
            0,
        );
        let combo_node = tree.add_child(root, Box::new(combo), None, 0);
        let field = tree.add_child(
            root,
            Box::new(TextField::new().with_input_surface_appearance(appearance.clone())),
            None,
            1,
        );
        let field_owner = tree.node(field).unwrap().id;
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        macro_rules! frame {
            () => {
                harness.frame_with_optional_custom_render_target(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(200.0, 200.0)),
                    &Theme::light(),
                    renderer.text_engine_mut(),
                    &builtin_registry(),
                    Some(target),
                    Some((&domains, WindowId::PRIMARY)),
                )
            };
        }
        let idle = frame!();
        let rect = tree.get_absolute_paint_rect(combo_node).unwrap();
        assert_eq!(rect.width, 96.0);
        let mut scene = control_order_scene(&idle);
        scene.clear_color = Color::WHITE;
        let pixels = render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
        save(&directory, &format!("combo-idle-{scale}"), &pixels, width);
        assert_smooth_tangents(&pixels, width, rect, 14.0_f32.min(rect.height * 0.5), scale);

        // The same HAL012 colors project actual Focus/Capture in TextField too.
        router.request_focus_for_widget(&mut tree, field_owner, domains.primary_state_mut());
        frame!();
        assert!(has_border(
            &harness.local_fragment(field_owner).unwrap(),
            focused.into()
        ));
        let field_rect = tree.get_absolute_paint_rect(field).unwrap();
        let pos = Point::new(
            field_rect.x + field_rect.width * 0.5,
            field_rect.y + field_rect.height * 0.5,
        );
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        ] {
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
        let frame = frame!();
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        assert!(has_border(
            &harness.local_fragment(field_owner).unwrap(),
            captured.into()
        ));
        let pixels = render_scene_at_scale(
            &mut renderer,
            &compositor,
            surface,
            control_order_scene(&frame),
            scale,
        );
        save(
            &directory,
            &format!("textfield-captured-{scale}"),
            &pixels,
            width,
        );

        // All three existing rounded-clip consumers must use the same law.
        let gpu = renderer.gpu_resources();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rounded clip contour regression"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[23, 30, 37, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        renderer.register_image_from_view(
            263_041,
            texture.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        let clip = RectF::new(8.0, 80.0, 96.0, 28.0);
        let mut reference: Option<Vec<u8>> = None;
        for (kind, name) in ["rect", "gradient", "image"].into_iter().enumerate() {
            let mut scene = SceneBuilder::new();
            scene.begin_clip(clip, Some(RoundedCorners::uniform(14.0)));
            match kind {
                0 => scene.push_rect(clip, face.into(), 1, None),
                1 => scene.push_gradient_rect(clip, 0.0, gradient.clone(), 1, None),
                _ => scene.push_surface_fill(
                    clip,
                    &SurfaceFill::Image(SurfaceImage {
                        source: 263_041,
                        uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
                        tint: PaintColor::WHITE,
                    }),
                    1,
                    None,
                ),
            }
            scene.end_clip();
            let mut scene = scene.build();
            scene.clear_color = Color::WHITE;
            let pixels = render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
            save(&directory, &format!("clip-{name}-{scale}"), &pixels, width);
            assert_smooth_tangents(&pixels, width, clip, 14.0, scale);
            if let Some(reference) = &reference {
                for (index, (actual, expected)) in pixels.iter().zip(reference).enumerate() {
                    assert!(
                        actual.abs_diff(*expected) <= 2,
                        "{name} coverage differs at {index}, scale {scale}"
                    );
                }
            } else {
                reference = Some(pixels);
            }
        }
        receipts.push(format!("scale={scale} backend=Metal trigger={rect:?} native_text=Complete contour=PASS shared_TextField_focus_capture=PASS"));
    }
    std::fs::write(directory.join("receipt.txt"), receipts.join("\n")).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn combo_leading_icon_size_preserves_layout_and_real_metal_ink() {
    use crate::core::event::InteractionDomains;
    use crate::core::layout::EdgeInsets;
    use crate::core::popup::{PopupInteractionPolicy, PopupWidthPolicy};
    use crate::core::widget::Widget;
    use crate::widgets::{container::Container, text_field::TextField};
    use std::{cell::Cell, rc::Rc, time::Instant};

    fn collect<'a>(nodes: &'a [SceneNode], output: &mut Vec<&'a SceneNode>) {
        for node in nodes {
            output.push(node);
            match node {
                SceneNode::Clip(clip) => collect(&clip.children, output),
                SceneNode::RetainedFragment(fragment) => collect(&fragment.children, output),
                _ => {}
            }
        }
    }

    const WIDTH: u32 = 160;
    const HEIGHT: u32 = 250;
    let mut theme = Theme::light();
    theme.typography.input_font_size = 16.0;
    let geom = theme.geometry.text_field;
    let pad = geom.padding_x;
    let affordance = theme.geometry.combo_box.affordance_width;
    let narrow = affordance + pad * 3.0 + theme.typography.caret_thickness + 12.0;
    let icons = builtin_registry();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(WIDTH, HEIGHT, scale).expect("Metal test target");
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.text_engine_mut().set_scale(scale);
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets::all(8.0)).with_spacing(6.0)),
            None,
            0,
        );
        let mount = |tree: &mut WidgetTree, width, widget: Box<dyn Widget>| {
            let holder = tree.add_child(
                root,
                Box::new(Container::empty().with_width(width).with_height(38.0)),
                None,
                0,
            );
            let node = tree.add_child(holder, widget, None, 0);
            tree.set_flex(
                node,
                FlexParams {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: 0.0,
                },
            );
            node
        };
        let commits = Rc::new(Cell::new(0));
        let cases = [
            (IconId::GLOBE, None, 88.0, 16.0, 20.0),
            (IconId::GLOBE, Some(24.0), 88.0, 24.0, 20.0),
            (IconId::BRIEFCASE, Some(24.0), 88.0, 24.0, 18.0),
            (IconId::BRIEFCASE, Some(24.0), narrow, 12.0, 18.0),
        ];
        let combos = cases.map(|(icon, requested, width, size, contour)| {
            let mut combo = ComboBox::simple_with_values(vec![
                ("AA".into(), "a".into()),
                ("BB".into(), "b".into()),
            ])
            .with_leading_icon(icon)
            .with_interaction_policy(PopupInteractionPolicy {
                width: PopupWidthPolicy::MatchTrigger,
            });
            if let Some(requested) = requested {
                combo = combo.with_leading_icon_size(requested);
            }
            combo.set_text_input_enabled(false);
            combo.set_input_text("AA".into()).unwrap();
            let count = commits.clone();
            combo.set_on_committed(move |_| count.set(count.get() + 1));
            (mount(&mut tree, width, Box::new(combo)), width, size, contour)
        });
        let mail = mount(
            &mut tree,
            88.0,
            Box::new(
                TextField::new()
                    .with_text("AA")
                    .with_leading_icon(IconId::MAIL)
                    .with_leading_icon_size(24.0),
            ),
        );
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        macro_rules! frame {
            ($now:expr) => {{
                harness.advance_transition_time($now);
                harness.frame_with_optional_custom_render_target(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(WIDTH as f32, HEIGHT as f32)),
                    &theme,
                    renderer.text_engine_mut(),
                    &icons,
                    Some(target),
                    Some((&domains, WindowId::PRIMARY)),
                )
            }};
        }
        macro_rules! click {
            ($position:expr) => {{
                let pos = $position;
                for event in [
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    },
                    FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    },
                ] {
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
            }};
        }
        let initial = frame!(0.0);
        let mut scene = control_order_scene(&initial);
        scene.clear_color = Color::WHITE;
        let pixels = render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
        for (node, width, size, contour) in combos.into_iter().chain([(mail, 88.0, 24.0, 18.0)]) {
            let rect = tree.get_absolute_paint_rect(node).unwrap();
            assert_eq!((rect.width, rect.height), (width, 38.0));
            let x = ((rect.x + pad) * scale).round() as u32;
            let y = ((rect.y + (rect.height - size) * 0.5) * scale).round() as u32;
            let px_width = (WIDTH as f32 * scale) as u32;
            let ink: Vec<_> = (x..x + (size * scale) as u32)
                .filter(|&x| {
                    (y..y + (size * scale) as u32)
                        .any(|y| pixel_at(&pixels, px_width, x, y)[0] < 128)
                })
                .collect();
            let span = ink.last().expect("real icon ink") + 1 - ink[0];
            let expected = contour * size / 24.0 * scale;
            assert!(
                (span as f32 - expected).abs() <= 1.0,
                "native {size}px icon at DPR {scale}: {span} ink pixels, expected {expected}"
            );
            if node == mail {
                continue;
            }

            let owner = tree.node(node).unwrap().id;
            let local = harness.local_fragment(owner).unwrap();
            let mut nodes = Vec::new();
            collect(&local, &mut nodes);
            let text_x = rect.x + size + 2.0 * pad;
            let text_right = rect.right() - affordance - pad;
            assert!(nodes.iter().any(|node| matches!(node, SceneNode::Text(t)
                if t.text == "AA" && t.position.x == text_x)));
            assert!(nodes.iter().any(|node| matches!(node, SceneNode::Clip(c)
                if c.rect.x == text_x && c.rect.right() == text_right)));
            assert!(nodes.iter().any(|node| matches!(node, SceneNode::Path(p)
                if p.is_stroke && p.path.bounds().x + 3.0 == rect.right() - affordance * 0.5)));
            let child = tree.node(node).unwrap().first_child.expect("sealed TextField");
            let child_rect = tree.get_absolute_paint_rect(child).unwrap();
            assert_eq!(child_rect.x, rect.x + size + pad);
            assert_eq!(child_rect.right(), rect.right() - affordance);
            let icon_hit = tree
                .hit_test(rect.x + pad + size * 0.5, rect.y + 19.0)
                .unwrap();
            assert_eq!(icon_hit.widget_id, owner);
        }

        let node = combos[1].0;
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        let combo = |tree: &WidgetTree| -> (RectF, Option<String>) {
            let widget = tree
                .node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .unwrap();
            (ComboBox::test_popup_local_rect(tree, node), widget.committed_value_string())
        };
        click!(Point::new(rect.right() - affordance * 0.5, rect.y + 19.0));
        frame!(0.010);
        frame!(0.100);
        let (popup, value) = combo(&tree);
        assert_eq!(value, None);
        assert_eq!(popup.width, rect.width);
        assert!(popup.height > 0.0);
        click!(Point::new(
            rect.x + popup.x + 10.0,
            rect.y + popup.y + popup.height * 0.75,
        ));
        assert_eq!(combo(&tree).1.as_deref(), Some("b"));
        assert_eq!(commits.get(), 1);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn input_icon_offset_and_chevron_match_reference_at_2x() {
    use crate::core::event::InteractionDomains;
    use crate::core::layout::EdgeInsets;
    use crate::core::popup::{PopupInteractionPolicy, PopupWidthPolicy};
    use crate::core::theme::ControlChromeState;
    use crate::core::widget::Widget;
    use crate::renderer::path::PathCommand::{LineTo, MoveTo};
    use crate::widgets::{container::Container, text_field::TextField};
    use std::time::Instant;

    const WIDTH: u32 = 112;
    const HEIGHT: u32 = 218;
    const SCALE: f32 = 2.0;
    let mut theme = Theme::light();
    theme.typography.input_font_size = 16.0;
    let pad = theme.geometry.text_field.padding_x;
    let affordance_width = theme.geometry.combo_box.affordance_width;
    let icons = builtin_registry();
    let (mut renderer, compositor, surface) =
        setup_with_size_and_scale(WIDTH, HEIGHT, SCALE).expect("real Metal test target");
    assert_eq!(
        renderer.gpu_resources().adapter.get_info().backend,
        wgpu::Backend::Metal
    );
    renderer.text_engine_mut().set_scale(SCALE);
    let target = renderer.custom_render_target_snapshot(surface).unwrap();

    let dark_ink = |pixels: &[u8], rect: RectF| {
        let px = |logical: f32| (logical * SCALE).round() as u32;
        (px(rect.y)..px(rect.bottom()))
            .flat_map(|y| (px(rect.x)..px(rect.right())).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel_at(pixels, WIDTH * 2, x, y)[0] < 128)
            .collect::<Vec<_>>()
    };
    let cases = [
        (IconId::GLOBE, 16.0, "RU"),
        (IconId::GLOBE, 22.0, "SE"),
        (IconId::BRIEFCASE, 22.0, "Co"),
        (IconId::MAIL, 22.0, "Email"),
    ];
    let mut reference = None;
    for adjusted in [false, true] {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets::all(8.0)).with_spacing(4.0)),
            None,
            0,
        );
        let nodes = cases.map(|(icon, size, label)| {
            let widget: Box<dyn Widget> = if icon == IconId::MAIL {
                let field = TextField::new()
                    .with_text(label)
                    .with_leading_icon(icon)
                    .with_leading_icon_size(size);
                Box::new(if adjusted {
                    field.with_leading_icon_y_offset(-1.0)
                } else {
                    field
                })
            } else {
                let mut combo = ComboBox::simple(vec![label.into(), "EN".into()])
                    .with_leading_icon(icon)
                    .with_leading_icon_size(size)
                    .with_interaction_policy(PopupInteractionPolicy {
                        width: PopupWidthPolicy::MatchTrigger,
                    });
                combo.set_text_input_enabled(false);
                combo.set_input_text(label.into()).unwrap();
                Box::new(if adjusted {
                    combo
                        .with_leading_icon_y_offset(-1.0)
                        .with_chevron_half_extents(5.0, 2.75)
                } else {
                    combo
                })
            };
            let holder = tree.add_child(
                root,
                Box::new(Container::empty().with_width(88.0).with_height(38.0)),
                None,
                0,
            );
            let node = tree.add_child(holder, widget, None, 0);
            tree.set_flex(
                node,
                FlexParams {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: 0.0,
                },
            );
            node
        });
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        macro_rules! frame {
            ($time:expr) => {{
                harness.advance_transition_time($time);
                harness.frame_with_optional_custom_render_target(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(WIDTH as f32, HEIGHT as f32)),
                    &theme,
                    renderer.text_engine_mut(),
                    &icons,
                    Some(target),
                    Some((&domains, WindowId::PRIMARY)),
                )
            }};
        }
        let initial = frame!(0.0);
        let mut scene = control_order_scene(&initial);
        scene.clear_color = Color::WHITE;
        let pixels = render_scene_at_scale(&mut renderer, &compositor, surface, scene, SCALE);
        let rects = nodes.map(|node| tree.get_absolute_paint_rect(node).unwrap());
        if !adjusted {
            reference = Some((rects, pixels));
            continue;
        }
        let (reference_rects, reference_pixels) = reference.as_ref().unwrap();
        assert_eq!(&rects, reference_rects);
        for ((node, rect), (icon, size, label)) in nodes.into_iter().zip(rects).zip(cases) {
            assert_eq!((rect.width, rect.height), (88.0, 38.0));
            let is_combo = icon != IconId::MAIL;
            let icon_rect = RectF::new(rect.x + pad, rect.y + 4.0, size, rect.height - 8.0);
            let original = dark_ink(reference_pixels, icon_rect);
            assert!(!original.is_empty(), "missing reference icon for {label}");
            let shifted: Vec<_> = original.into_iter().map(|(x, y)| (x, y - 2)).collect();
            assert_eq!(
                dark_ink(&pixels, icon_rect),
                shifted,
                "icon offset for {label}"
            );
            let text_x = icon_rect.right() + pad;
            let text_right = rect.right() - pad - if is_combo { affordance_width } else { 0.0 };
            let label_rect = RectF::new(text_x, rect.y + 4.0, text_right - text_x, 30.0);
            let label_ink = dark_ink(&pixels, label_rect);
            assert!(!label_ink.is_empty(), "missing label {label}");
            assert_eq!(label_ink, dark_ink(reference_pixels, label_rect));
            let owner = tree.node(node).unwrap().id;
            let hit = tree
                .hit_test(rect.x + pad + size * 0.5, rect.y + 19.0)
                .unwrap();
            assert_eq!(hit.widget_id, owner);
            if is_combo {
                let arrow_rect = RectF::new(
                    rect.right() - affordance_width + 2.0,
                    rect.y + 8.0,
                    affordance_width - 4.0,
                    22.0,
                );
                let arrow = dark_ink(&pixels, arrow_rect);
                let span_x = arrow.iter().map(|p| p.0).max().unwrap()
                    - arrow.iter().map(|p| p.0).min().unwrap() + 1;
                let span_y = arrow.last().unwrap().1 - arrow.first().unwrap().1 + 1;
                assert!(
                    (20..=22).contains(&span_x) && (10..=14).contains(&span_y),
                    "{label} chevron ink is {span_x}×{span_y} at 2×"
                );
                let original = dark_ink(reference_pixels, arrow_rect);
                assert!(
                    arrow.len() > original.len(),
                    "{label} selected geometry must enlarge the path"
                );
            }
        }

        let compact = nodes[0];
        let rect = rects[0];
        let owner = tree.node(compact).unwrap().id;
        let pos = Point::new(rect.right() - affordance_width * 0.5, rect.y + 19.0);
        assert_eq!(tree.hit_test(pos.x, pos.y).unwrap().widget_id, owner);
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        frame!(0.010);
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        let focus_color: Color = theme
            .input_chrome(ControlChromeState::Focused)
            .chrome
            .border_color
            .into();
        let mut focus_edge = false;
        visit_scene_nodes(&harness.local_fragment(owner).unwrap(), &mut |node| {
            focus_edge |= matches!(node, SceneNode::RoundedRect(edge) if edge.color == focus_color && edge.rect == rect);
        });
        assert!(focus_edge, "closed ComboBox keeps its independent Focus cue");

        macro_rules! route {
            ($event:expr) => {
                router.route_input_event(
                    &mut tree,
                    $event,
                    Some(root),
                    renderer.text_engine_mut(),
                    Some(harness.animation_context()),
                    domains.primary_state_mut(),
                    &mut bindings,
                )
            };
        }
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        ] {
            route!(event);
        }
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        frame!(0.020);
        frame!(0.100);
        let open = harness.local_fragment(owner).unwrap();
        let capture_color: Color = theme
            .input_chrome(ControlChromeState::Captured)
            .chrome
            .border_color
            .into();
        let mut capture_edge = false;
        let mut upward = false;
        visit_scene_nodes(&open, &mut |node| {
            if let SceneNode::Path(path) = node {
                let bounds = path.path.bounds();
                capture_edge |= path.is_stroke && path.color == capture_color && bounds.width > 80.0;
                upward |= path.is_stroke && matches!(path.path.commands.as_slice(),
                    [MoveTo(x1, y1), LineTo(x2, y2), LineTo(x3, y3)]
                    if (*x1, *x2, *x3) == (pos.x - 5.0, pos.x, pos.x + 5.0)
                        && (*y1, *y2, *y3) == (pos.y + 2.75, pos.y - 2.75, pos.y + 2.75));
            }
        });
        assert!(
            capture_edge && upward,
            "open popup retains the capture cue and flips the same chevron"
        );
        let combo = tree
            .node(compact)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ComboBox>()
            .unwrap();
        assert_eq!(ComboBox::test_popup_local_rect(&tree, compact).width, rect.width);
        assert_eq!(combo.committed_value_string(), None);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn combo_input_surface_appearance_borderless_and_rounded_popup_real_metal() {
    use crate::core::appearance::InputSurfaceAppearance;
    use crate::core::event::{InteractionDomains, KeyboardAuthorityMode, PointerButtonsState};
    use crate::core::layout::EdgeInsets;
    use crate::core::paint::{
        Color as PaintColor, ColorStop, GradientDef, GradientKind, SurfaceGradient,
    };
    use crate::core::popup::{PopupInteractionPolicy, PopupWidthPolicy};
    use crate::widgets::container::Container;
    use std::{cell::Cell, rc::Rc, sync::Arc};

    fn leaves(nodes: &[SceneNode]) -> Vec<&SceneNode> {
        let mut out = Vec::new();
        for node in nodes {
            out.push(node);
            match node {
                SceneNode::RetainedFragment(n) => out.extend(leaves(&n.children)),
                SceneNode::Clip(n) => out.extend(leaves(&n.children)),
                SceneNode::Scroll(n) => out.extend(leaves(&n.children)),
                SceneNode::Layer(n) => out.extend(leaves(&n.children)),
                _ => {}
            }
        }
        out
    }
    fn no_perimeter(nodes: &[SceneNode], overlay: bool) {
        assert!(
            !leaves(nodes).iter().any(|node| match node {
                SceneNode::BorderRect(n) => n.border_color.a > 0.0,
                SceneNode::Path(n) if n.is_stroke =>
                    overlay || n.z_index != crate::core::paint::LAYER_CONTENT + 4,
                _ => false,
            }),
            "the selected instance must not borrow a default perimeter"
        );
    }
    fn caret_extent(nodes: &[SceneNode]) -> f32 {
        leaves(nodes)
            .into_iter()
            .find_map(|node| match node {
                SceneNode::Path(n)
                    if n.is_stroke && n.z_index == crate::core::paint::LAYER_CONTENT + 4 =>
                {
                    n.mesh.as_ref().map(|mesh| mesh.bounds.height)
                }
                _ => None,
            })
            .expect("the original path caret has real stroke geometry")
    }
    fn save(directory: &std::path::Path, name: &str, pixels: &[u8], width: u32) {
        let height = pixels.len() / 4 / width as usize;
        let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
    }
    let gradient = |color| {
        SurfaceGradient::new(GradientDef {
            kind: GradientKind::Linear,
            stops: vec![
                ColorStop { offset: 0.0, color },
                ColorStop { offset: 1.0, color },
            ],
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
        })
        .unwrap()
    };
    let clear = PaintColor::TRANSPARENT;
    let face = PaintColor::from_hex(0x171E25);
    let appearance = InputSurfaceAppearance::try_new(
        gradient(clear),
        clear,
        clear,
        clear,
        face,
        PaintColor::from_hex(0x455449),
        14.0,
    )
    .unwrap()
    .with_state_surfaces(
        gradient(face.with_alpha(0.12)),
        gradient(face.with_alpha(0.24)),
    )
    .try_with_focus_colors(clear, clear)
    .unwrap();
    assert_eq!(
        appearance.surface_for_interaction(false, true, false),
        appearance.surface_for_interaction(false, false, true),
        "existing appearances retain the Active material for an open popup"
    );
    let appearance = appearance.with_open_surface(gradient(face));
    // The shared value cannot make the TextField's required fallback disappear.
    assert!(
        appearance
            .focus_border(crate::core::theme::ControlChromeState::Focused)
            .is_none()
    );
    let directory = std::path::PathBuf::from(format!(
        "target/evidence/combo-borderless/metal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(200, 160, scale).unwrap();
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.text_engine_mut().set_scale(scale);
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        let mut combo = ComboBox::simple(vec!["RU".into(), "EN".into()])
            .with_input_surface_appearance(appearance.clone())
            .with_leading_icon(IconId::GLOBE)
            .with_interaction_policy(PopupInteractionPolicy {
                width: PopupWidthPolicy::MatchTrigger,
            });
        combo.set_text_input_enabled(false);
        combo.set_input_text("RU".into()).unwrap();
        combo.set_on_committed(move |_| count.set(count.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets {
                        left: 8.0,
                        top: 8.0,
                        right: 104.0,
                        bottom: 0.0,
                    })
                    .with_spacing(64.0),
            ),
            None,
            0,
        );
        let holder = tree.add_child(
            root,
            Box::new(Container::empty().with_width(88.0).with_height(38.0)),
            None,
            0,
        );
        let node = tree.add_child(holder, Box::new(combo), None, 0);
        tree.set_flex(
            node,
            FlexParams {
                grow: 1.0,
                shrink: 1.0,
                basis: 0.0,
            },
        );
        let peer = tree.add_child(
            root,
            Box::new(ComboBox::simple(vec!["Default".into()])),
            None,
            1,
        );
        let owner = tree.node(node).unwrap().id;
        let peer_owner = tree.node(peer).unwrap().id;
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        let theme = Theme::light();
        macro_rules! frame {
            ($time:expr) => {{
                harness.advance_transition_time($time);
                harness.frame_with_optional_custom_render_target(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(200.0, 160.0)),
                    &theme,
                    renderer.text_engine_mut(),
                    &builtin_registry(),
                    Some(target),
                    Some((&domains, WindowId::PRIMARY)),
                )
            }};
        }
        macro_rules! route {
            ($event:expr) => {
                router.route_input_event(
                    &mut tree,
                    $event,
                    Some(root),
                    renderer.text_engine_mut(),
                    Some(harness.animation_context()),
                    domains.primary_state_mut(),
                    &mut bindings,
                )
            };
        }
        macro_rules! click {
            ($pos:expr) => {
                route!(FusedInputEvent::PointerDown {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos: $pos,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                    click_count: 1
                });
                route!(FusedInputEvent::PointerUp {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos: $pos,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary
                });
            };
        }
        let key = |key_code| FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        };
        macro_rules! pixels {
            ($frame:expr, $name:expr) => {{
                let mut scene = control_order_scene(&$frame);
                scene.clear_color = Color::WHITE;
                let pixels =
                    render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
                save(
                    &directory,
                    &format!("{}-{scale}", $name),
                    &pixels,
                    (200.0 * scale) as u32,
                );
                pixels
            }};
        }
        let idle = frame!(0.0);
        no_perimeter(&harness.local_fragment(owner).unwrap(), false);
        let idle_pixels = pixels!(idle, "idle");
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        assert_eq!((rect.width, rect.height), (88.0, 38.0));
        let width = (200.0 * scale) as u32;
        let trigger_pixel = |pixels: &[u8]| {
            pixel_at(
                pixels,
                width,
                ((rect.x + rect.width * 0.5) * scale) as u32,
                ((rect.y + 3.0) * scale) as u32,
            )
        };
        assert_eq!(trigger_pixel(&idle_pixels), [255, 255, 255, 255]);
        let pointer_move = |pos| FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            delta: (0.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState::default(),
        };
        route!(pointer_move(Point::new(
            rect.x + rect.width * 0.5,
            rect.y + 10.0
        )));
        let hovered = frame!(0.005);
        no_perimeter(&harness.local_fragment(owner).unwrap(), false);
        let hovered_pixels = pixels!(hovered, "hovered-closed");
        route!(pointer_move(Point::new(180.0, 140.0)));
        frame!(0.007);
        let peer_fragment = harness.local_fragment(peer_owner).unwrap();
        assert!(leaves(&peer_fragment).iter().any(|node| matches!(node,
            SceneNode::RoundedRect(n) if n.radius == 2.0
                && n.color == theme.input_chrome(crate::core::theme::ControlChromeState::Idle).chrome.border_color.into())));
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        let focused = frame!(0.010);
        no_perimeter(&harness.local_fragment(owner).unwrap(), false);
        let focused_caret = caret_extent(&harness.local_fragment(owner).unwrap());
        let focused_pixels = pixels!(focused, "focused");
        let arrow = Point::new(rect.right() - 3.0, rect.y + rect.height * 0.5);
        click!(arrow);
        route!(key(KeyCode::Home));
        route!(key(KeyCode::Down));
        let opening = frame!(0.020);
        let opening_pixels = pixels!(opening, "opening");
        let open = frame!(0.060);
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        no_perimeter(&harness.local_fragment(owner).unwrap(), false);
        assert!(
            caret_extent(&harness.local_fragment(owner).unwrap()) > focused_caret,
            "the captured caret must have a distinct real stroke extent"
        );
        let overlay = harness.overlay_fragment(owner).unwrap();
        no_perimeter(&overlay, true);
        assert!(
            leaves(&overlay)
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Text(n) => Some(n),
                    _ => None,
                })
                .all(|text| text.backgrounds.is_empty())
        );
        let popup = ComboBox::test_popup_local_rect(&tree, node);
        assert_eq!(popup.width, rect.width);
        let popup = RectF::new(
            rect.x + popup.x,
            rect.y + popup.y,
            popup.width,
            popup.height,
        );
        let open_pixels = pixels!(open, "open-last-row-highlight");
        let open_trigger = trigger_pixel(&open_pixels);
        let popup_face = pixel_at(
            &open_pixels,
            width,
            ((popup.x + popup.width * 0.5) * scale) as u32,
            ((popup.y + 3.0) * scale) as u32,
        );
        assert_eq!(
            open_trigger, popup_face,
            "opaque trigger joins the real popup material"
        );
        assert_eq!(
            trigger_pixel(&opening_pixels),
            open_trigger,
            "the first opening frame already uses the separate open material"
        );
        assert!(
            trigger_pixel(&idle_pixels)[0] > trigger_pixel(&hovered_pixels)[0]
                && trigger_pixel(&hovered_pixels)[0] > trigger_pixel(&focused_pixels)[0]
                && trigger_pixel(&focused_pixels)[0] > open_trigger[0],
            "Idle, Hover, closed Focus and Open must have independent real pixels"
        );
        let join_y = (popup.y * scale).round() as u32;
        for y in join_y - 2..join_y + 2 {
            for x in ((rect.x + 2.0) * scale).ceil() as u32
                ..((rect.right() - 2.0) * scale).floor() as u32
            {
                let actual = pixel_at(&open_pixels, width, x, y);
                assert!(
                    actual
                        .iter()
                        .zip(open_trigger)
                        .all(|(a, b)| a.abs_diff(b) <= 1),
                    "internal join must retain opaque material at DPR{scale}, ({x},{y}): {actual:?} vs {open_trigger:?}"
                );
            }
        }
        let trigger_y = ((rect.bottom() - 4.0) * scale).floor() as u32;
        let popup_y = ((popup.y + 8.0) * scale).floor() as u32;
        for offset in 0..(2.0 * scale).ceil() as u32 {
            for x in [
                (rect.x * scale).round() as u32 + offset,
                (rect.right() * scale).round() as u32 - 1 - offset,
            ] {
                let trigger_edge = pixel_at(&open_pixels, width, x, trigger_y);
                let popup_edge = pixel_at(&open_pixels, width, x, popup_y);
                assert!(
                    trigger_edge
                        .iter()
                        .zip(popup_edge)
                        .all(|(a, b)| a.abs_diff(b) <= 1),
                    "joined lateral coverage differs at DPR{scale}, x{x}: trigger={trigger_edge:?} popup={popup_edge:?}"
                );
            }
        }
        for x in [popup.x + 0.5, popup.right() - 0.5] {
            let pixel = pixel_at(
                &open_pixels,
                width,
                (x * scale) as u32,
                ((popup.bottom() - 0.5) * scale) as u32,
            );
            assert_eq!(
                pixel,
                [255, 255, 255, 255],
                "row fill escaped the real lower rounded clip"
            );
        }
        let middle = pixel_at(
            &open_pixels,
            width,
            ((popup.x + popup.width * 0.5) * scale) as u32,
            ((popup.bottom() - 2.0) * scale) as u32,
        );
        assert!(
            middle[0] < 150,
            "the clipped last-row fill must still be present"
        );
        // The original commit releases Capture and preserves focus. The selected
        // appearance must follow that transition without retaining a perimeter.
        click!(Point::new(popup.x + 20.0, popup.y + 12.0));
        assert_eq!(calls.get(), 1);
        frame!(0.100);
        let closing = frame!(0.115);
        let closing_pixels = pixels!(closing, "closing");
        assert_eq!(
            trigger_pixel(&closing_pixels),
            open_trigger,
            "the open material remains while the real popup is closing"
        );
        let closed = frame!(0.150);
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        no_perimeter(&harness.local_fragment(owner).unwrap(), false);
        let closed_pixels = pixels!(closed, "committed-closed");
        assert_eq!(
            closed_pixels, focused_pixels,
            "the original focus-only pixels must return after Capture ends"
        );
        let mut cue_difference = 0;
        for y in ((rect.y + 5.0) * scale) as u32..((rect.bottom() - 5.0) * scale) as u32 {
            for x in ((rect.right() - 20.0) * scale) as u32..((rect.right() - 4.0) * scale) as u32 {
                cue_difference += usize::from(
                    pixel_at(&focused_pixels, width, x, y) != pixel_at(&open_pixels, width, x, y),
                );
            }
        }
        assert!(
            cue_difference > 0,
            "borderless Focus and Capture must remain visibly distinct on the real caret"
        );
        click!(arrow);
        for code in [KeyCode::Home, KeyCode::Down, KeyCode::Enter] {
            route!(key(code));
        }
        assert_eq!(calls.get(), 2);
        assert_eq!(
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .unwrap()
                .input_text(),
            "EN"
        );
        frame!(0.200);
        let final_frame = frame!(0.250);
        no_perimeter(&harness.local_fragment(owner).unwrap(), false);
        pixels!(final_frame, "keyboard-commit-closed");
        let quiet = harness.local_fragment(owner).unwrap();
        let quiet_peer = harness.local_fragment(peer_owner).unwrap();
        frame!(0.260);
        assert!(Arc::ptr_eq(&quiet, &harness.local_fragment(owner).unwrap()));
        assert!(Arc::ptr_eq(
            &quiet_peer,
            &harness.local_fragment(peer_owner).unwrap()
        ));
        check_frosted_combo_display(&mut renderer, &compositor, surface, scale, &directory);
    }
}

#[cfg(target_os = "macos")]
fn check_frosted_combo_display(
    renderer: &mut WgpuRenderer,
    compositor: &Compositor,
    surface: SurfaceId,
    scale: f32,
    directory: &std::path::Path,
) {
    use crate::core::appearance::InputSurfaceAppearance;
    use crate::core::event::InteractionDomains;
    use crate::core::paint::{
        Color as PaintColor, ColorStop, FrostedGlassAppearance, GradientDef, GradientKind,
        SurfaceGradient,
    };
    use crate::core::popup::{PopupInteractionPolicy, PopupWidthPolicy};
    use crate::core::widget_tree::ControlSubtreePlacement;
    use crate::widgets::combo_box::ComboBoxGlassError;
    use crate::widgets::container::Container;

    fn leaves(nodes: &[SceneNode]) -> Vec<&SceneNode> {
        let mut out = Vec::new();
        for node in nodes {
            out.push(node);
            match node {
                SceneNode::RetainedFragment(n) => out.extend(leaves(&n.children)),
                SceneNode::Clip(n) => out.extend(leaves(&n.children)),
                SceneNode::Scroll(n) => out.extend(leaves(&n.children)),
                SceneNode::Layer(n) => out.extend(leaves(&n.children)),
                _ => {}
            }
        }
        out
    }
    fn without_glass(nodes: &[SceneNode]) -> Vec<SceneNode> {
        nodes
            .iter()
            .filter_map(|node| {
                if matches!(node, SceneNode::FrostedGlass(_)) {
                    return None;
                }
                let mut node = node.clone();
                match &mut node {
                    SceneNode::RetainedFragment(n) => {
                        n.children = without_glass(&n.children).into()
                    }
                    SceneNode::Clip(n) => n.children = without_glass(&n.children).into(),
                    SceneNode::Scroll(n) => n.children = without_glass(&n.children).into(),
                    SceneNode::Layer(n) => n.children = without_glass(&n.children).into(),
                    _ => {}
                }
                Some(node)
            })
            .collect()
    }
    fn foreground_part(nodes: &[SceneNode], foreground: bool) -> Vec<SceneNode> {
        nodes
            .iter()
            .filter_map(|node| {
                let mut node = node.clone();
                match &mut node {
                    SceneNode::RetainedFragment(n) => {
                        n.children = foreground_part(&n.children, foreground).into()
                    }
                    SceneNode::Clip(n) => {
                        n.children = foreground_part(&n.children, foreground).into()
                    }
                    SceneNode::Scroll(n) => {
                        n.children = foreground_part(&n.children, foreground).into()
                    }
                    SceneNode::Layer(n) => {
                        n.children = foreground_part(&n.children, foreground).into()
                    }
                    SceneNode::Text(_) | SceneNode::Path(_) if !foreground => return None,
                    SceneNode::Text(_) | SceneNode::Path(_) => {}
                    _ if foreground => return None,
                    _ => {}
                }
                Some(node)
            })
            .collect()
    }
    let flat = |color| {
        SurfaceGradient::new(GradientDef {
            kind: GradientKind::Linear,
            stops: vec![
                ColorStop { offset: 0.0, color },
                ColorStop { offset: 1.0, color },
            ],
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
        })
        .unwrap()
    };
    let clear = PaintColor::TRANSPARENT;
    let tint = PaintColor::from_hex(0x252D34);
    let appearance = InputSurfaceAppearance::try_new(
        flat(clear),
        clear,
        clear,
        clear,
        tint.with_alpha(0.08),
        PaintColor::from_hex(0x3D4A55).with_alpha(0.16),
        14.0,
    )
    .unwrap()
    .with_state_surfaces(flat(tint.with_alpha(0.05)), flat(tint.with_alpha(0.08)))
    .with_open_surface(flat(tint.with_alpha(0.08)))
    .try_with_focus_colors(clear, clear)
    .unwrap();
    let glass = FrostedGlassAppearance::try_new(1.5, clear, 1.0).unwrap();
    assert!(matches!(
        ComboBox::simple(vec!["RU".into()]).try_with_frosted_glass(glass.clone()),
        Err(ComboBoxGlassError::RequiresDisplayMode)
    ));
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let mut combo = ComboBox::simple(vec!["RU".into(), "EN".into()])
        .with_input_surface_appearance(appearance)
        .with_interaction_policy(PopupInteractionPolicy {
            width: PopupWidthPolicy::MatchTrigger,
        })
        .with_leading_icon(IconId::GLOBE);
    combo.set_text_input_enabled(false);
    combo.set_input_text("RU".into()).unwrap();
    combo.set_on_committed(move |_| count.set(count.get() + 1));
    let combo = combo.try_with_frosted_glass(glass).unwrap();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(Flex::column().with_padding(crate::core::layout::EdgeInsets::all(8.0))),
        None,
        0,
    );
    let holder = tree.add_child(
        root,
        Box::new(Container::empty().with_width(88.0).with_height(38.0)),
        None,
        0,
    );
    let node = tree.add_child(holder, Box::new(combo), None, 0);
    tree.set_control_subtree_order(
        root,
        &[holder],
        ControlSubtreePlacement::Main {
            z_index: LAYER_CONTENT,
        },
    )
    .unwrap();
    let owner = tree.node(node).unwrap().id;
    let mut harness = RetainedSceneTestHarness::new();
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let mut bindings = BindingRegistry::new();
    let target = renderer.custom_render_target_snapshot(surface).unwrap();
    let theme = Theme::light();
    macro_rules! frame {
        ($time:expr) => {{
            harness.advance_transition_time($time);
            harness.frame_with_optional_custom_render_target(
                &mut tree,
                root,
                Constraints::tight(Size::new(200.0, 160.0)),
                &theme,
                renderer.text_engine_mut(),
                &builtin_registry(),
                Some(target),
                Some((&domains, WindowId::PRIMARY)),
            )
        }};
    }
    macro_rules! route {
        ($event:expr) => {
            router.route_input_event(
                &mut tree,
                $event,
                Some(root),
                renderer.text_engine_mut(),
                Some(harness.animation_context()),
                domains.primary_state_mut(),
                &mut bindings,
            )
        };
    }
    macro_rules! render {
        ($frame:expr, $name:expr, $rect:expr) => {{
            let mut scene = control_order_scene(&$frame);
            scene.clear_color = Color::WHITE;
            let mut source = SceneBuilder::new();
            for x in 0..200 {
                source.push_rect(
                    RectF::new(x as f32, 0.0, 1.0, 160.0),
                    Color::from_hex(if x % 2 == 0 { 0x3080C0 } else { 0xE8B060 }),
                    0,
                    None,
                );
            }
            let mut background = source.build().root;
            background.append(&mut scene.root);
            scene.root = background;
            let mut reference = control_order_scene(&$frame);
            reference.clear_color = Color::WHITE;
            reference.root = without_glass(&scene.root);
            let unblurred = render_scene_at_scale(renderer, compositor, surface, reference, scale);
            let mut foreground = control_order_scene(&$frame);
            foreground.clear_color = Color::TRANSPARENT;
            foreground.root = foreground_part(&scene.root, true);
            let glyphs = render_scene_at_scale(renderer, compositor, surface, foreground, scale);
            let mut backdrop = control_order_scene(&$frame);
            backdrop.clear_color = scene.clear_color;
            backdrop.root = foreground_part(&scene.root, false);
            let backdrop = render_scene_at_scale(renderer, compositor, surface, backdrop, scale);
            let actual = render_scene_at_scale(renderer, compositor, surface, scene, scale);
            let width = (200.0 * scale) as u32;
            let rect: RectF = $rect;
            let y = ((rect.y + 3.0) * scale).round() as u32;
            let xs = ((rect.x + 20.0) * scale).round() as u32
                ..((rect.right() - 20.0) * scale).round() as u32;
            let blurred: Vec<_> = xs
                .clone()
                .map(|x| pixel_at(&actual, width, x, y)[0])
                .collect();
            let sharp: Vec<_> = xs.map(|x| pixel_at(&unblurred, width, x, y)[0]).collect();
            assert!(
                sharp.iter().max().unwrap() - sharp.iter().min().unwrap() > 80,
                "reference must contain real high-frequency backdrop"
            );
            assert!(
                blurred.iter().max().unwrap() - blurred.iter().min().unwrap() < 12,
                "positive native backdrop blur required, not alpha-only tint: {blurred:?}"
            );
            // Reuse HAL385's foreground OVER oracle: native LCD alpha is the
            // maximum channel coverage, not proof that all RGB channels are
            // opaque when their encoded values happen to equal the ink color.
            let mut cores = 0;
            for ((glyph, base), output) in glyphs
                .chunks_exact(4)
                .zip(backdrop.chunks_exact(4))
                .zip(actual.chunks_exact(4))
            {
                let alpha_lo = ((f64::from(glyph[3]) - 0.5) / 255.0).max(0.0);
                let alpha_hi = ((f64::from(glyph[3]) + 0.5) / 255.0).min(1.0);
                for ch in 0..3 {
                    let low = frost_decode(((f64::from(glyph[ch]) - 0.5) / 255.0).max(0.0));
                    let high = frost_decode(((f64::from(glyph[ch]) + 0.5) / 255.0).min(1.0));
                    let b = frost_decode(f64::from(base[ch]) / 255.0);
                    let lower = frost_encode((low + b * (1.0 - alpha_hi)).clamp(0.0, 1.0)) * 255.0;
                    let upper = frost_encode((high + b * (1.0 - alpha_lo)).clamp(0.0, 1.0)) * 255.0;
                    assert!(
                        f64::from(output[ch]) + 1.0 >= lower
                            && f64::from(output[ch]) - 1.0 <= upper,
                        "sharp foreground OVER, not blurred ink: glyph={glyph:?} base={base:?} output={output:?} channel={ch} interval={lower}..{upper}"
                    );
                }
                if glyph[3] == 255 {
                    cores += 1;
                }
            }
            assert!(
                cores >= (8.0 * scale) as usize,
                "actual glyph/icon cores are required"
            );
            let mut ppm = format!("P6\n{width} {}\n255\n", (160.0 * scale) as u32).into_bytes();
            for p in actual.chunks_exact(4) {
                ppm.extend_from_slice(&p[..3]);
            }
            std::fs::write(directory.join(format!("glass-{}-{scale}.ppm", $name)), ppm).unwrap();
        }};
    }
    let closed = frame!(0.0);
    let rect = tree.get_absolute_paint_rect(node).unwrap();
    assert_eq!((rect.width, rect.height), (88.0, 38.0));
    assert_eq!(
        leaves(&harness.local_fragment(owner).unwrap())
            .iter()
            .filter(|n| matches!(n, SceneNode::FrostedGlass(_)))
            .count(),
        1
    );
    render!(closed, "closed", rect);
    let arrow = Point::new(rect.right() - 3.0, rect.y + rect.height * 0.5);
    for event in [
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: arrow,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: arrow,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
    ] {
        route!(event);
    }
    let opening = frame!(0.010);
    assert!(
        leaves(&harness.local_fragment(owner).unwrap())
            .iter()
            .all(|n| !matches!(n, SceneNode::FrostedGlass(_) | SceneNode::Text(_)))
    );
    assert_eq!(
        leaves(&harness.overlay_fragment(owner).unwrap())
            .iter()
            .filter(|n| matches!(n, SceneNode::FrostedGlass(_)))
            .count(),
        1,
        "the first opening frame must keep one glass body and crisp foreground"
    );
    render!(opening, "opening", rect);
    let open = frame!(0.050);
    let overlay = harness.overlay_fragment(owner).unwrap();
    let nodes = leaves(&overlay);
    let bodies: Vec<_> = nodes
        .iter()
        .filter_map(|n| {
            if let SceneNode::FrostedGlass(n) = n {
                Some(n)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        bodies.len(),
        1,
        "one common glass, not two overlapping captures"
    );
    let body = bodies[0].rect();
    assert_eq!((body.x, body.width), (rect.x, rect.width));
    assert!(body.height > rect.height);
    render!(open, "open", body);
    for key_code in [KeyCode::Home, KeyCode::Down, KeyCode::Enter] {
        route!(FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false
        });
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(
        tree.node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ComboBox>()
            .unwrap()
            .input_text(),
        "EN"
    );
    frame!(0.060);
    let closing = frame!(0.070);
    render!(closing, "closing", rect);
    let settled = frame!(0.120);
    assert_eq!(
        leaves(&harness.local_fragment(owner).unwrap())
            .iter()
            .filter(|n| matches!(n, SceneNode::FrostedGlass(_)))
            .count(),
        1
    );
    render!(settled, "committed-closed", rect);
}

#[test]
fn gradient_stop_endpoints_preserve_admitted_colors_without_extrapolation() {
    use crate::core::paint::{
        Color as PaintColor, ColorStop, GradientDef, GradientKind, PaintSink, SurfaceFill,
        SurfaceGradient,
    };

    let (mut renderer, compositor, surface) = setup().expect("headless Gradient adapter");
    let first = PaintColor::from_linear(0.2, 0.8, 0.1, 0.4);
    let last = PaintColor::from_linear(0.9, 0.1, 0.4, 0.8);
    for kind in [GradientKind::Linear, GradientKind::Radial] {
        let mut scene = base_scene();
        scene.push_rect(RectF::new(0.0, 0.0, 24.0, 64.0), first.into(), 1, None);
        scene.push_rect(RectF::new(24.0, 0.0, 24.0, 64.0), last.into(), 1, None);
        scene.push_surface_fill(
            RectF::new(48.0, 0.0, 48.0, 64.0),
            &SurfaceFill::Gradient(
                SurfaceGradient::new(GradientDef {
                    kind,
                    stops: vec![
                        ColorStop {
                            offset: 0.25,
                            color: first,
                        },
                        ColorStop {
                            offset: 0.75,
                            color: last,
                        },
                    ],
                    p0: if kind == GradientKind::Linear {
                        (0.0, 0.0)
                    } else {
                        (0.5, 0.5)
                    },
                    p1: if kind == GradientKind::Linear {
                        (0.0, 1.0)
                    } else {
                        (1.0, 0.5)
                    },
                })
                .expect("ordered finite non-endpoint stops are admitted"),
            ),
            1,
            None,
        );
        let pixels = render_scene(&mut renderer, &compositor, surface, scene.build());
        let probes = if kind == GradientKind::Linear {
            [(72, 4), (72, 60)]
        } else {
            [(72, 32), (94, 3)]
        };
        for (index, (x, y)) in probes.into_iter().enumerate() {
            let expected = pixel_at(&pixels, WIDTH, if index == 0 { 12 } else { 36 }, 32);
            let actual = pixel_at(&pixels, WIDTH, x, y);
            for channel in 0..4 {
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= 1,
                    "outside the stop interval {kind:?} must preserve its endpoint color/alpha: {actual:?} vs {expected:?}"
                );
            }
        }
    }
}

#[test]
fn gradient_stop_positive_narrow_interval_interpolates() {
    use crate::core::paint::{
        Color as PaintColor, ColorStop, GradientDef, GradientKind, PaintSink, SurfaceFill,
        SurfaceGradient,
    };

    let (mut renderer, compositor, surface) = setup().expect("headless Gradient adapter");
    let midpoint = 24.5 / 48.0;
    let mut scene = base_scene();
    scene.push_rect(
        RectF::new(0.0, 0.0, 48.0, 64.0),
        Color::from_linear(0.5, 0.5, 0.25, 1.0),
        1,
        None,
    );
    scene.push_surface_fill(
        RectF::new(48.0, 0.0, 48.0, 64.0),
        &SurfaceFill::Gradient(
            SurfaceGradient::new(GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop {
                        offset: midpoint - 0.00025,
                        color: PaintColor::from_linear(1.0, 0.0, 0.25, 1.0),
                    },
                    ColorStop {
                        offset: midpoint + 0.00025,
                        color: PaintColor::from_linear(0.0, 1.0, 0.25, 1.0),
                    },
                ],
                p0: (0.0, 0.0),
                p1: (1.0, 0.0),
            })
            .expect("positive finite narrow intervals are admitted"),
        ),
        1,
        None,
    );
    let pixels = render_scene(&mut renderer, &compositor, surface, scene.build());
    let expected = pixel_at(&pixels, WIDTH, 24, 32);
    let actual = pixel_at(&pixels, WIDTH, 72, 32);
    for channel in 0..4 {
        assert!(
            actual[channel].abs_diff(expected[channel]) <= 1,
            "a positive stop interval must interpolate its midpoint: {actual:?} vs {expected:?}"
        );
    }
}

#[test]
fn gradient_multi_surface_preserves_parameters_and_geometry() {
    use crate::core::paint::{Color as PaintColor, ColorStop, GradientDef, GradientKind};
    use crate::core::scene::RoundedCorners;

    for z in [1, LAYER_OVERLAY + 1] {
        let (mut renderer, mut compositor, first) = setup().expect("headless Gradient adapter");
        let second = add_text_realization_surface(&mut renderer, &mut compositor, 1);
        renderer.set_gpu_rounded_clips_enabled(true);
        let scene = |second: bool| {
            let mut scene = base_scene();
            scene.begin_clip(
                if second {
                    RectF::new(20.0, 10.0, 64.0, 42.0)
                } else {
                    RectF::new(6.0, 4.0, 80.0, 50.0)
                },
                Some(RoundedCorners::uniform(if second { 4.0 } else { 16.0 })),
            );
            scene.push_gradient_rect(
                RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
                0.0,
                GradientDef {
                    kind: if second {
                        GradientKind::Radial
                    } else {
                        GradientKind::Linear
                    },
                    stops: vec![
                        ColorStop {
                            offset: 0.0,
                            color: if second {
                                PaintColor::from_linear(0.05, 0.1, 0.9, 0.75)
                            } else {
                                PaintColor::from_linear(0.2, 0.65, 0.05, 0.5)
                            },
                        },
                        ColorStop {
                            offset: 1.0,
                            color: if second {
                                PaintColor::from_linear(0.5, 0.05, 0.7, 1.0)
                            } else {
                                PaintColor::from_linear(0.8, 0.1, 0.1, 1.0)
                            },
                        },
                    ],
                    p0: if second { (0.5, 0.5) } else { (0.0, 0.0) },
                    p1: if second { (1.0, 0.5) } else { (0.0, 1.0) },
                },
                z,
                None,
            );
            scene.end_clip();
            scene.build()
        };
        let first_scene = scene(false);
        let second_scene = scene(true);
        let first_control = render_scene(&mut renderer, &compositor, first, first_scene.clone());
        let second_control = render_scene(&mut renderer, &compositor, second, second_scene.clone());
        let scenes = HashMap::from([(first, first_scene), (second, second_scene)]);
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        for (surface, expected) in [(first, first_control), (second, second_control)] {
            let actual = read_compiled_surface(&renderer, &compiled, surface).unwrap();
            let mismatch = actual.iter().zip(&expected).position(|(a, b)| a != b);
            assert_eq!(
                mismatch, None,
                "later targets must not overwrite Gradient parameters or clips for {surface:?}, z {z}"
            );
        }
    }
}

#[test]
fn gradient_multi_surface_parameter_capacity_refuses_without_aliasing_and_recovers() {
    use crate::core::paint::{Color as PaintColor, ColorStop, GradientDef, GradientKind};

    let (mut renderer, mut compositor, first) = setup().expect("headless Gradient adapter");
    let second = add_text_realization_surface(&mut renderer, &mut compositor, 1);
    let scene = |count| {
        let mut scene = base_scene();
        for index in 0..count {
            scene.push_gradient_rect(
                RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
                0.0,
                GradientDef {
                    kind: GradientKind::Linear,
                    stops: vec![
                        ColorStop {
                            offset: 0.0,
                            color: PaintColor::WHITE,
                        },
                        ColorStop {
                            offset: 1.0,
                            color: PaintColor::BLACK,
                        },
                    ],
                    p0: (index as f32 / 513.0, 0.0),
                    p1: (1.0, 1.0),
                },
                1,
                None,
            );
        }
        scene.build()
    };
    let within_capacity = scene(512);
    let excess = scene(513);
    let scenes = HashMap::from([(first, within_capacity), (second, excess)]);
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    assert!(
        matches!(
            renderer.compile_multi(&plan),
            Err(crate::renderer::RendererCompileError::Surface(crate::renderer::RendererSurfaceError::OutOfMemory))
        ),
        "the bounded Gradient table must refuse instead of substituting another uniform"
    );
    assert_eq!(renderer.pending_staging_work_for_test(), (0, 0));
    let mut recovered = base_scene();
    recovered.push_rect(
        RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        Color::WHITE,
        1,
        None,
    );
    let pixels = render_scene(&mut renderer, &compositor, first, recovered.build());
    assert_eq!(pixel_at(&pixels, WIDTH, 48, 32), [255; 4]);
}

fn visit_scene_nodes(nodes: &[SceneNode], visit: &mut impl FnMut(&SceneNode)) {
    for node in nodes {
        visit(node);
        let children = match node {
            SceneNode::RetainedFragment(fragment) => &*fragment.children,
            SceneNode::Clip(clip) => &clip.children,
            SceneNode::Layer(layer) => &layer.children,
            SceneNode::Scroll(scroll) => &scroll.children,
            _ => continue,
        };
        visit_scene_nodes(children, visit);
    }
}

#[test]
fn tag_input_open_filter_and_chip_delete_keep_surface_renderable() {
    const TARGET_WIDTH: u32 = 400;
    const TARGET_HEIGHT: u32 = 200;
    const SCALE: f32 = 2.0;
    let (mut renderer, compositor, surface_id) =
        setup_with_size_and_scale(TARGET_WIDTH, TARGET_HEIGHT, SCALE)
            .expect("TagInput regression requires a GPU");
    let labels = ["gpu", "release", "safety", "telemetry", "audit"];
    let tag = TagInput::new()
        .with_items(
            labels
                .iter()
                .enumerate()
                .map(|(id, label)| TagItem {
                    id: id as u64,
                    label: (*label).to_string(),
                    group: None,
                    enabled: true,
                })
                .collect(),
        )
        .with_selection(&[0, 1, 2]);
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let tag_node = tree.add_child(root, Box::new(tag), None, 0);
    let theme = Theme::light();
    let icons = builtin_registry();
    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(TARGET_WIDTH as f32, TARGET_HEIGHT as f32));
    let mut paint = |tree: &mut WidgetTree,
                     renderer: &mut WgpuRenderer,
                     expected_rows: &[&str],
                     checked_rows: &[&str]| {
        let frame = harness.frame(
            tree,
            root,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let mut rows = Vec::new();
        let mut chip_close = None;
        visit_scene_nodes(&frame.scene_root, &mut |node| {
            if let SceneNode::Text(text) = node {
                assert!(
                    !text.text.contains(['\u{2610}', '\u{2611}']),
                    "TagInput checkbox state must not create font glyph demand"
                );
                if text.text == "\u{00d7}" && chip_close.is_none() {
                    chip_close = Some(Point::new(
                        text.position.x + 1.0,
                        text.position.y + theme.typography.font_size / 2.0,
                    ));
                }
                if text.z_index == LAYER_OVERLAY + 2 {
                    if let Some(background) = text.backgrounds.first() {
                        rows.push((
                            text.text.clone(),
                            Point::new(
                                text.position.x + background.x_offset,
                                text.position.y + background.y_offset,
                            ),
                        ));
                    }
                }
            }
        });
        assert_eq!(
            rows.iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>(),
            expected_rows,
            "the event must visibly update the popup rows"
        );
        let mut scene = SceneGraph::new();
        scene.root = frame.scene_root;
        scene.clear_color = theme.colors.surfaces.base0.into();
        let pixels = render_scene_at_scale(renderer, &compositor, surface_id, scene, SCALE);
        let physical_width = (TARGET_WIDTH as f32 * SCALE) as u32;
        for (label, origin) in rows {
            let background = pixel_at(
                &pixels,
                physical_width,
                ((origin.x + TARGET_WIDTH as f32 - 4.0) * SCALE) as u32,
                ((origin.y + 12.0) * SCALE) as u32,
            );
            let check_x = origin.x + theme.geometry.text_field.padding_x;
            let mut border_ink = 0;
            let mut center_ink = 0;
            for y in 3..21 {
                for x in 0..theme.typography.font_size.ceil() as u32 {
                    let ink = rgb_distance(
                        pixel_at(
                            &pixels,
                            physical_width,
                            ((check_x + x as f32) * SCALE) as u32,
                            ((origin.y + y as f32) * SCALE) as u32,
                        ),
                        background,
                    ) > 50;
                    border_ink += usize::from(ink);
                    if (5..9).contains(&x) && (10..14).contains(&y) {
                        center_ink += usize::from(ink);
                    }
                }
            }
            assert!(
                border_ink > 8,
                "{label}: checkbox must remain visible above its row background"
            );
            assert_eq!(
                center_ink > 0,
                checked_rows.contains(&label.as_str()),
                "{label}: rendered checkmark must match selection"
            );
        }
        chip_close
    };
    paint(&mut tree, &mut renderer, &[], &[]);
    let rect = tree
        .get_absolute_paint_rect(tag_node)
        .expect("TagInput rect");
    let dropdown = Point::new(rect.x + rect.width - 4.0, rect.y + rect.height / 2.0);
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();
    let mut dispatch =
        |tree: &mut WidgetTree, renderer: &mut WgpuRenderer, event: FusedInputEvent| {
            router.route_input_event(
                tree,
                event,
                Some(root),
                renderer.text_engine_mut(),
                None,
                &mut domain_state,
                &mut bindings,
            );
        };
    let pointer_down = |pos| FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    dispatch(&mut tree, &mut renderer, pointer_down(dropdown));
    paint(&mut tree, &mut renderer, &labels, &labels[..3]);
    dispatch(
        &mut tree,
        &mut renderer,
        FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: "tel".into(),
            modifiers: Modifiers::default(),
        },
    );
    paint(&mut tree, &mut renderer, &["telemetry"], &[]);
    for _ in 0..3 {
        dispatch(
            &mut tree,
            &mut renderer,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Backspace,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
        );
    }
    let chip_close = paint(&mut tree, &mut renderer, &labels, &labels[..3])
        .expect("selected chip close control");
    dispatch(&mut tree, &mut renderer, pointer_down(chip_close));
    paint(&mut tree, &mut renderer, &labels, &labels[1..3]);
    let selected = tree
        .node(tag_node)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<TagInput>()
        .unwrap()
        .committed_ids();
    assert_eq!(selected, vec![1, 2]);
    dispatch(&mut tree, &mut renderer, pointer_down(dropdown));
    paint(&mut tree, &mut renderer, &[], &[]);
    dispatch(&mut tree, &mut renderer, pointer_down(dropdown));
    paint(&mut tree, &mut renderer, &labels, &labels[1..3]);
}

#[test]
fn date_time_picker_routed_draft_commit_and_overlay_execute() {
    use crate::core::event::InteractionDomains;
    use crate::model::date_time::{DateTimeSelection, DateTimeValue};
    use crate::model::date_value::DateValue;
    use crate::model::time_value::{TimeDisplayMode, TimeValue};
    use crate::widgets::date_time_picker::DateTimePicker;

    const W: u32 = 900;
    const H: u32 = 560;
    let (mut renderer, compositor, surface) = setup_with_size_and_scale(W, H, 1.0)
        .expect("date/time picker owner proof requires a real adapter and readback");
    let date = |day| DateValue::new(2026, 11, day).unwrap();
    let point = |day, hour| DateTimeValue::new(date(day), TimeValue::new(hour, 7, None)).unwrap();
    let values = [
        DateTimeSelection::Date(date(2)),
        DateTimeSelection::Time(TimeValue::new(9, 7, None)),
        DateTimeSelection::DateTime(point(2, 9)),
        DateTimeSelection::DateRange {
            start: date(2),
            end: date(4),
        },
        DateTimeSelection::DateTimeRange {
            start: point(2, 9),
            end: point(4, 11),
        },
    ];
    let theme = Theme::light();
    let icons = builtin_registry();
    let constraints = Constraints::tight(Size::new(W as f32, H as f32));
    for value in values {
        let commits = Rc::new(Cell::new(0));
        let received = Rc::clone(&commits);
        let picker = DateTimePicker::new(value)
            .unwrap()
            .with_steps(1, 15)
            .unwrap()
            .with_display_mode(TimeDisplayMode::H12)
            .on_commit(move |_| received.set(received.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let node = tree.add_child(root, Box::new(picker), None, 0);
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let mut frame =
            |tree: &mut WidgetTree, renderer: &mut WgpuRenderer, domains: &InteractionDomains| {
                harness.frame_with_appearance(
                    tree,
                    root,
                    constraints,
                    &theme,
                    renderer.text_engine_mut(),
                    &icons,
                    domains,
                    WindowId::PRIMARY,
                )
            };
        frame(&mut tree, &mut renderer, &domains);
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        let mut dispatch = |tree: &mut WidgetTree,
                            renderer: &mut WgpuRenderer,
                            domains: &mut InteractionDomains,
                            event: FusedInputEvent| {
            // app.rs dispatch_engine_action owns Capture Escape before ordinary
            // EventRouter delivery. Exercise that same engine handler here;
            // this retained/GPU fixture is not a native App::run proof.
            if matches!(
                &event,
                FusedInputEvent::KeyDown {
                    key_code: KeyCode::Escape,
                    ..
                }
            ) {
                assert!(router.handle_engine_action(
                    tree,
                    crate::runtime::actions::ActionId::RELEASE_CAPTURE,
                    Some(root),
                    domains.primary_state_mut(),
                    renderer.text_engine_mut(),
                    None,
                ));
                return;
            }
            router.route_input_event(
                tree,
                event,
                Some(root),
                renderer.text_engine_mut(),
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
        };
        let pointer_down = |pos| FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        };
        let key = |key_code| FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        };
        let trigger = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        dispatch(
            &mut tree,
            &mut renderer,
            &mut domains,
            pointer_down(trigger),
        );
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        assert_eq!(
            domains.primary_state().focus_manager.focused(),
            Some(tree.node(node).unwrap().id)
        );
        let open = frame(&mut tree, &mut renderer, &domains);
        let mut apply = None;
        visit_scene_nodes(&open.scene_root, &mut |node| {
            if let SceneNode::Text(text) = node {
                if text.text == "Apply" {
                    assert_eq!(text.z_index, LAYER_OVERLAY + 4);
                    apply = Some(Point::new(
                        text.position.x + 2.0,
                        text.position.y + theme.typography.font_size / 2.0,
                    ));
                }
            }
        });
        assert!(
            apply.is_some(),
            "routed capture must realize the popup for {value:?}"
        );
        let mut scene = control_order_scene(&open);
        scene.clear_color = theme.colors.surfaces.base0.into();
        let output = render_scene_at_scale(&mut renderer, &compositor, surface, scene, 1.0);
        assert_eq!(output.len(), W as usize * H as usize * 4);

        // Route an actual edit, then Escape: the working selection survives but
        // neither the committed value nor the application callback changes.
        dispatch(
            &mut tree,
            &mut renderer,
            &mut domains,
            key(if value.mode().has_date() {
                KeyCode::Right
            } else {
                KeyCode::Down
            }),
        );
        let working = tree
            .node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<DateTimePicker>()
            .unwrap()
            .working();
        assert_ne!(working, value);
        dispatch(&mut tree, &mut renderer, &mut domains, key(KeyCode::Escape));
        let closed = frame(&mut tree, &mut renderer, &domains);
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert_eq!(commits.get(), 0);
        let picker = tree
            .node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<DateTimePicker>()
            .unwrap();
        assert_eq!(picker.value(), value);
        assert_eq!(picker.working(), working);
        visit_scene_nodes(&closed.scene_root, &mut |node| {
            if let SceneNode::Text(text) = node {
                assert_ne!(text.text, "Apply");
            }
        });
        dispatch(
            &mut tree,
            &mut renderer,
            &mut domains,
            pointer_down(trigger),
        );
        frame(&mut tree, &mut renderer, &domains);
        dispatch(&mut tree, &mut renderer, &mut domains, key(KeyCode::Enter));
        assert_eq!(
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<DateTimePicker>()
                .unwrap()
                .value(),
            working
        );
        assert_eq!(commits.get(), 1);
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
    }
}

#[cfg(target_os = "macos")]
mod cell_integrated_chrome_tests {
    use super::*;
    use crate::core::appearance::CellEditorPresentation;
    use crate::core::event::{InteractionDomains, PhysicalKeyCode};
    use crate::core::widget::Widget;
    use crate::core::widget_tree::NodeId;

    fn frame(
        renderer: &mut WgpuRenderer, surface: SurfaceId, tree: &mut WidgetTree, root: NodeId,
        h: &mut RetainedSceneTestHarness, domains: &mut InteractionDomains, router: &mut EventRouter,
    ) -> crate::core::app::RetainedSceneTestFrame {
        tree.process_composite_choice_work(router, domains.primary_state_mut(), renderer.text_engine_mut(), 0);
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        h.frame_with_optional_custom_render_target(tree, root, Constraints::tight(Size::new(360.0, 240.0)),
            &Theme::light(), renderer.text_engine_mut(), &builtin_registry(), Some(target), Some((domains, WindowId::PRIMARY)))
    }

    fn pixels(renderer: &mut WgpuRenderer, compositor: &Compositor, surface: SurfaceId,
        frame: &crate::core::app::RetainedSceneTestFrame, scale: f32) -> Vec<u8> {
        let plan = DrawPlanBuilder::new().build_multi(
            &HashMap::from([(surface, control_order_scene(frame))]), compositor.snapshot_multi(),
            &mut MeshCache::new(), |_| scale, 0).unwrap();
        let compiled = renderer.compile_multi(&plan).unwrap().expect_ready("native Compile must be ready");
        assert!(matches!(compiled.text_realization(surface).unwrap().outcome(),
            crate::renderer::TextRealizationOutcome::Complete { .. }), "HAL391 complete actual text required");
        read_compiled_surface(renderer, &compiled, surface).unwrap()
    }

    fn save(directory: &std::path::Path, name: &str, rgba: &[u8], scale: f32) {
        let mut ppm = format!("P6\n{} {}\n255\n", (360.0 * scale) as u32, (240.0 * scale) as u32).into_bytes();
        for pixel in rgba.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
        std::fs::write(directory.join(format!("{name}-{scale}x.ppm")), ppm).unwrap();
    }

    fn rounded_shell_count(nodes: &[SceneNode], rect: RectF) -> usize {
        let padding = Theme::light().geometry.text_field.padding_x;
        nodes.iter().map(|node| match node {
            // The arrow compartment and its intrinsic divider are not an
            // exterior shell. Test the actual surface spanning both regions.
            SceneNode::RoundedRect(r) if r.radius > 0.0
                && r.rect.x <= rect.x + padding && r.rect.right() >= rect.right() - padding => 1,
            SceneNode::RetainedFragment(f) => rounded_shell_count(&f.children, rect),
            SceneNode::Clip(c) => rounded_shell_count(&c.children, rect),
            SceneNode::Scroll(s) => rounded_shell_count(&s.children, rect),
            SceneNode::Layer(l) => rounded_shell_count(&l.children, rect),
            _ => 0,
        }).sum()
    }

    struct ChoiceSource {
        rows: Vec<RowId>,
        labels: HashMap<RowId, String>,
        values: HashMap<(RowId, ColumnId), String>,
        pending: Vec<DataInvalidationEvent>,
        commits: Rc<Cell<usize>>,
    }
    impl ChoiceSource {
        fn new() -> Self {
            let rows: Vec<_> = (0..4).map(|_| RowId::new()).collect();
            let labels = rows.iter().copied().zip(["Alpha", "Beta", "Gamma", "Delta"].map(str::to_owned)).collect();
            Self { rows, labels, values: HashMap::new(), pending: Vec::new(), commits: Rc::new(Cell::new(0)) }
        }
    }
    impl DataSource for ChoiceSource {
        fn row_ids(&self) -> &[RowId] { &self.rows }
        fn extent(&self) -> Extent { Extent::Exact(self.rows.len()) }
        fn mode(&self) -> UniverseMode { UniverseMode::Live }
        fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
            self.values.get(&(row, column)).or_else(|| self.labels.get(&row))
                .map_or(ValueState::Missing, |s| ValueState::Ready(CellValue::String(s.clone())))
        }
        fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
            CellCapabilities { visible: true, readable: true, editable: true, committable: true, redacted: false }
        }
        fn commit_cell(&mut self, row: RowId, column: ColumnId, value: CellValue) -> bool {
            let CellValue::String(value) = value else { return false; };
            if !self.labels.contains_key(&row) { return false; }
            self.values.insert((row, column), value);
            self.commits.set(self.commits.get() + 1);
            self.pending.push(DataInvalidationEvent::CellsInvalidated { rows: vec![row],
                columns: crate::widgets::tabular_view::data_adapter::ColumnInvalidationScope::Specific(vec![column]) });
            true
        }
        fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> { std::mem::take(&mut self.pending) }
    }

    fn route(tree: &mut WidgetTree, root: NodeId, renderer: &mut WgpuRenderer,
        domains: &mut InteractionDomains, router: &mut EventRouter, event: FusedInputEvent) {
        router.route_input_event(tree, event, Some(root), renderer.text_engine_mut(), None,
            domains.primary_state_mut(), &mut BindingRegistry::new());
    }

    fn click(tree: &mut WidgetTree, root: NodeId, renderer: &mut WgpuRenderer,
        domains: &mut InteractionDomains, router: &mut EventRouter, pos: Point) {
        route(tree, root, renderer, domains, router, FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY, time: Instant::now(), pos, device: PointerDevice::Mouse,
            button: PointerButton::Primary, click_count: 1,
        });
        route(tree, root, renderer, domains, router, FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY, time: Instant::now(), pos, device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        });
    }

    struct PropertySource {
        column: ColumnId,
        value: crate::engines::cell_source::PropertyValue,
        revision: u64,
    }
    impl crate::engines::cell_source::CellSource for PropertySource {
        fn cell(&self, row: u64, column: ColumnId) -> Option<crate::engines::cell_source::Cell> {
            (row == 1 && column == self.column)
                .then(|| crate::engines::cell_source::Cell::new(self.value.clone(), true))
        }
        fn commit_cell(&mut self, row: u64, column: ColumnId, value: crate::engines::cell_source::PropertyValue) -> bool {
            if row != 1 || column != self.column { return false; }
            self.value = value; self.revision += 1; true
        }
        fn revision(&self) -> u64 { self.revision }
    }
    impl crate::widgets::property_editor::PropertyDataSource for PropertySource {
        fn nodes(&self) -> Vec<crate::widgets::property_editor::PropertyNode> {
            vec![crate::widgets::property_editor::PropertyNode {
                node_id: 1, parent_id: None, label: "Property".into(),
                kind: crate::widgets::property_editor::PropertyNodeKind::Data,
            }]
        }
    }

    fn mount_session_consumer(tree: &mut WidgetTree, root: NodeId, hierarchical: bool)
        -> (NodeId, NodeId, ColumnId, Rc<std::cell::RefCell<PropertySource>>) {
        use crate::engines::cell_source::PropertyValue;
        use crate::widgets::property_editor::PropertyEditor;
        use crate::widgets::tree_view::TreeView;
        use crate::widgets::tabular_view::column::ColumnModel;
        let (widget, body, footer, column, source): (Box<dyn Widget>, _, _, _, _) = if hierarchical {
            let mut view = TreeView::new();
            let mut model = ColumnModel::new();
            let name = Column::new(200.0); let name_id = name.id; model.push(name);
            let value = Column::new(120.0); let column = value.id; model.push(value);
            view.set_columns(model); view.set_tree_column(name_id);
            view.set_tree_data(vec![
                crate::engines::hierarchical::NodeMeta {
                    node_id: 1, parent_index: None, depth: 0, has_children: false, label: "Property".into(),
                },
                crate::engines::hierarchical::NodeMeta {
                    node_id: 2, parent_index: None, depth: 0, has_children: true, label: "Parent".into(),
                },
                crate::engines::hierarchical::NodeMeta {
                    node_id: 3, parent_index: Some(1), depth: 1, has_children: false, label: "Child".into(),
                },
            ]);
            let source = Rc::new(std::cell::RefCell::new(PropertySource {
                column, value: PropertyValue::String("Hello мир".into()), revision: 0,
            }));
            view.set_cell_source(Some(source.clone()));
            let body: Box<dyn Widget> = Box::new(view.create_body_widget());
            let footer: Option<Box<dyn Widget>> = Some(Box::new(view.create_footer_widget()));
            (Box::new(view), body, footer, column, source)
        } else {
            let mut view = PropertyEditor::new();
            view.add_name_column("Name");
            let column = view.add_value_column("Value");
            let source = Rc::new(std::cell::RefCell::new(PropertySource {
                column, value: PropertyValue::String("Hello мир".into()), revision: 0,
            }));
            view.set_data_source(source.clone());
            let body: Box<dyn Widget> = Box::new(view.create_body_widget());
            (Box::new(view), body, None, column, source)
        };
        let view = tree.add_child(root, widget, None, 0);
        tree.set_flex(view, FlexParams::flexible(1.0));
        let scroll = tree.add_child(view, Box::new(ScrollArea::new_unwired()), None, 0);
        let body = tree.add_child(scroll, body, None, 0);
        tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().wire_content(body);
        if let Some(footer) = footer { tree.add_child(view, footer, None, 0); }
        (view, body, column, source)
    }

    #[test]
    fn cell_integrated_mount_qualification_is_exact_and_retired() {
        use crate::core::appearance::{CellEditorPresentationRefusal as Refusal, FrameworkAppearanceSnapshot};
        use crate::widgets::text_field::TextField;
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let first = tree.add_child(root, Box::new(Flex::row()), None, 0);
        let second = tree.add_child(root, Box::new(Flex::row()), None, 0);
        let editor = tree.add_child(first, Box::new(TextField::new()), None, 0);
        let unknown = tree.add_child(first, Box::new(crate::widgets::label::Label::new("readout")), None, 0);
        let column = Column::new(120.0).id;
        assert_eq!(tree.qualify_cell_editor(second, editor, 1, column), Err(Refusal::WrongConsumer));
        assert_eq!(tree.qualify_cell_editor(first, unknown, 1, column), Err(Refusal::UnsupportedFamily));
        assert_eq!(tree.node(unknown).unwrap().cell_editor_presentation, CellEditorPresentation::Standalone);
        assert_eq!(tree.qualify_cell_editor(first, editor, 1, column), Ok(true));
        assert_eq!(tree.qualify_cell_editor(first, editor, 1, column), Ok(false));
        assert_eq!(tree.qualify_cell_editor(first, editor, 2, column), Err(Refusal::WrongConsumer));
        let domains = InteractionDomains::new();
        let snapshot = FrameworkAppearanceSnapshot::capture(&domains, WindowId::PRIMARY, &tree).unwrap();
        tree.prepare_cell_edges(root, Some(&snapshot));
        let owner = tree.node(first).unwrap().id;
        let prepared = tree.cell_edges[&owner][&(1, column)].clone();
        tree.prepare_cell_edges(second, Some(&snapshot));
        assert_eq!(tree.cell_edges[&owner][&(1, column)], prepared, "another root cannot erase this publication");
        tree.remove_node(editor);
        tree.prepare_cell_edges(root, Some(&snapshot));
        assert!(!prepared.editor.is_live());
        assert!(!tree.cell_edges.contains_key(&owner), "retired mounts cannot lend an edge to a replacement");
    }

    #[test]
    fn cell_integrated_property_and_tree_sessions_real_metal() {
        use crate::widgets::property_editor::PropertyEditor;
        use crate::widgets::tree_view::TreeView;
        use crate::widgets::text_field::TextField;
        let directory = std::path::PathBuf::from(format!("target/evidence/hal391/sessions-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut receipts = Vec::new();
        for scale in [1.0_f32, 2.0, 1.5] {
            for hierarchical in [false, true] {
                let (mut renderer, compositor, surface) = setup_with_size_and_scale(360, 240, scale).expect("actual Metal");
                assert_eq!(renderer.gpu_resources().adapter.get_info().backend, wgpu::Backend::Metal);
                let mut tree = WidgetTree::new();
                let root = tree.create_root(Box::new(Flex::column()), None, 0);
                let (view, body, column, _) = mount_session_consumer(&mut tree, root, hierarchical);
                let peer = tree.add_child(root, Box::new(ComboBox::simple(vec!["Standalone".into()])), None, 0);
                let mut domains = InteractionDomains::new(); let mut router = EventRouter::new();
                let mut h = RetainedSceneTestHarness::new();
                frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
                let hts = if hierarchical {
                    tree.node(view).unwrap().widget.as_any().downcast_ref::<TreeView>().unwrap().hts_handle()
                } else {
                    tree.node(view).unwrap().widget.as_any().downcast_ref::<PropertyEditor>().unwrap().hts_handle()
                };
                let (allocation_x, allocation_width, row_height) = {
                    let hts = hts.borrow();
                    let slot = hts.column_layout().body_slots().find(|s| s.column_id == column).unwrap();
                    (slot.x, slot.width, hts.row_height())
                };
                let body_rect = tree.get_absolute_paint_rect(body).unwrap();
                let point = Point::new(body_rect.x + allocation_x + allocation_width * 0.5, body_rect.y + row_height * 0.5);
                click(&mut tree, root, &mut renderer, &mut domains, &mut router, point);
                route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::PointerDown {
                    window_id: WindowId::PRIMARY, time: Instant::now(), pos: point, device: PointerDevice::Mouse,
                    button: PointerButton::Primary, click_count: 2,
                });
                route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::PointerUp {
                    window_id: WindowId::PRIMARY, time: Instant::now(), pos: point, device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                });
                frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
                let field = tree.children(body).into_iter().find(|id| tree.node(*id).unwrap().widget.as_any().is::<TextField>())
                    .expect("original consumer DoubleClick mounts the factory TextField");
                crate::core::app::apply_deferred_focus_requests(&mut tree, &mut router, domains.primary_state_mut(), renderer.text_engine_mut());
                let captured = frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
                let rect = tree.get_absolute_paint_rect(field).unwrap();
                assert_eq!(rect.x, body_rect.x + allocation_x);
                assert_eq!(rect.width, allocation_width);
                assert_eq!(rect.height, row_height);
                assert_eq!(tree.node(field).unwrap().cell_editor_presentation, CellEditorPresentation::CellIntegrated);
                let prepared = tree.cell_edges[&tree.node(body).unwrap().id].values()
                    .find(|edge| edge.row == 1 && edge.column == column).unwrap();
                assert_eq!(prepared.state, crate::core::theme::ControlChromeState::Captured);
                assert_eq!(domains.primary_state().focus_manager.focused(), Some(tree.node(field).unwrap().id));
                assert_eq!(domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);
                assert_eq!(rounded_shell_count(&h.local_fragment(tree.node(field).unwrap().id).unwrap(), rect), 0);
                let name = if hierarchical { "treeview" } else { "property-editor" };
                let captured_pixels = pixels(&mut renderer, &compositor, surface, &captured, scale);
                save(&directory, &format!("{name}-captured"), &captured_pixels, scale);
                // The first glyph's actual content area excludes the end caret
                // and cell edge. Logical text/Complete alone cannot prove ink.
                let theme = Theme::light();
                let start_x = rect.x + theme.geometry.text_field.padding_x;
                let mut glyph_ink = 0;
                for y in ((rect.y + 3.0) * scale).ceil() as u32..((rect.bottom() - 3.0) * scale).floor() as u32 {
                    for x in (start_x * scale).ceil() as u32..((start_x + theme.typography.font_size) * scale).floor() as u32 {
                        let pixel = pixel_at(&captured_pixels, (360.0 * scale) as u32, x, y);
                        glyph_ink += usize::from(pixel[..3].iter().all(|channel| *channel < 100));
                    }
                }
                assert!(glyph_ink > 0, "{name}: actual first-glyph pixels must survive the consumer child clip");
                route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::TextInput {
                    window_id: WindowId::PRIMARY, time: Instant::now(), text: "Привет мир".into(), modifiers: Modifiers::default(),
                });
                let text = |tree: &WidgetTree| tree.node(field).unwrap().widget.as_any().downcast_ref::<TextField>().unwrap().text().to_owned();
                assert_eq!(text(&tree), "Привет мир", "immediate real cell document");
                for (shift, expected) in [(false, "Hello мир"), (true, "Привет мир")] {
                    route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::TextInput {
                        window_id: WindowId::PRIMARY, time: Instant::now(), text: "z".into(),
                        modifiers: Modifiers { meta: true, shift, ..Default::default() },
                    });
                    assert_eq!(text(&tree), expected, "original Undo/Redo transaction");
                }
                assert!(!tree.node(peer).unwrap().dirty_paint,
                    "cell input and edge preparation do not invalidate an unrelated peer");
                h.advance_transition_time(0.4);
                let edited = frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
                save(&directory, &format!("{name}-edited"), &pixels(&mut renderer, &compositor, surface, &edited, scale), scale);
                let peer_local = h.local_fragment(tree.node(peer).unwrap().id).unwrap();
                let body_local = h.local_fragment(tree.node(body).unwrap().id).unwrap();
                frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
                assert!(Arc::ptr_eq(&peer_local, &h.local_fragment(tree.node(peer).unwrap().id).unwrap()));
                assert!(Arc::ptr_eq(&body_local, &h.local_fragment(tree.node(body).unwrap().id).unwrap()),
                    "equal current cell edge evidence reuses the consumer fragment");
                tree.node_mut(field).unwrap().widget.as_any_mut().downcast_mut::<TextField>().unwrap().set_password(true);
                tree.mark_layout_dirty(field);
                let private = frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
                visit_scene_nodes(&private.scene_root, &mut |node| {
                    if let SceneNode::Text(text) = node {
                        assert!(!text.text.contains("Привет"), "cell presentation preserves password privacy");
                    }
                });
                let mut fusion = crate::core::input_fusion::InputFusion::new();
                let mut bindings = BindingRegistry::new();
                tree.request_widget_read_only(field, true).unwrap();
                let result = crate::core::app::settle_test_widget_availability(
                    &mut tree, &mut router, &mut fusion, domains.primary_state_mut(), &mut bindings, renderer.text_engine_mut());
                assert!(result.rejected.is_empty());
                route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::TextInput {
                    window_id: WindowId::PRIMARY, time: Instant::now(), text: "rejected".into(), modifiers: Modifiers::default(),
                });
                assert_eq!(text(&tree), "Привет мир");
                tree.request_widget_enabled(field, false).unwrap();
                let result = crate::core::app::settle_test_widget_availability(
                    &mut tree, &mut router, &mut fusion, domains.primary_state_mut(), &mut bindings, renderer.text_engine_mut());
                assert!(result.rejected.is_empty());
                frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
                assert_eq!(tree.cell_edges[&tree.node(body).unwrap().id][&(1, column)].state,
                    crate::core::theme::ControlChromeState::Disabled);
                assert_ne!(domains.primary_state().focus_manager.focused(), Some(tree.node(field).unwrap().id));
                receipts.push(serde_json::json!({"consumer":name,"scale":scale,"view":tree.node(view).unwrap().id.0,
                    "field_rect":[rect.x,rect.y,rect.width,rect.height],"capture":"routed + original deferred owner",
                    "text":"Привет мир","undo_redo":"immediate","peer_reused":true}));
            }
        }
        std::fs::write(directory.join("receipts.json"), serde_json::to_vec_pretty(&receipts).unwrap()).unwrap();
        println!("HAL391 session Metal evidence {}", directory.display());
    }

    #[test]
    fn cell_integrated_listview_choice_real_metal() {
        let directory = std::path::PathBuf::from(format!("target/evidence/hal391/metal-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut receipts = Vec::new();
        for scale in [1.0_f32, 2.0, 1.5] {
            let (mut renderer, compositor, surface) = setup_with_size_and_scale(360, 240, scale).expect("actual native GPU");
            assert_eq!(renderer.gpu_resources().adapter.get_info().backend, wgpu::Backend::Metal);
            let source = ChoiceSource::new();
            let commits = source.commits.clone();
            let mut view = ListView::new(source);
            view.add_column("Name", 100.0);
            let choice = view.add_column("Choice", 120.0);
            view.add_column("Other", 120.0);
            view.set_column_combo_options(choice, ["Alpha", "Beta", "Gamma", "Delta"].into_iter()
                .map(|s| (s.to_string(), s.to_string())).collect());
            let body = view.create_body_widget();
            let footer = view.create_footer_widget();
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()), None, 0);
            let list = tree.add_child(root, Box::new(view), None, 0);
            tree.set_flex(list, FlexParams::flexible(1.0));
            let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
            let body = tree.add_child(scroll, Box::new(body), None, 0);
            tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().wire_content(body);
            tree.add_child(list, Box::new(footer), None, 0);
            let peer = tree.add_child(root, Box::new(ComboBox::simple(vec!["Standalone".into()])), None, 0);
            let mut domains = InteractionDomains::new();
            let mut router = EventRouter::new();
            let mut h = RetainedSceneTestHarness::new();
            // Original two engine cuts: publish roles, then Phase 2 mount and ready Layout.
            frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            let first = frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            let mut editors: Vec<_> = tree.children(body).into_iter().filter(|id| {
                tree.node(*id).unwrap().widget.as_any().is::<ComboBox>()
            }).collect();
            editors.sort_by(|a,b| tree.get_rect(*a).unwrap().y.total_cmp(&tree.get_rect(*b).unwrap().y));
            assert_eq!(editors.len(), 4, "four genuine resident choices");
            assert_eq!(commits.get(), 0);
            let body_rect = tree.get_absolute_paint_rect(body).unwrap();
            let rect = tree.get_absolute_paint_rect(editors[0]).unwrap();
            let next = tree.get_absolute_paint_rect(editors[1]).unwrap();
            assert_eq!(rect.x, body_rect.x + 100.0, "full canonical slot origin");
            assert_eq!(rect.width, 120.0, "original Choice width, no exterior inset");
            assert_eq!(rect.y, body_rect.y, "full row origin");
            assert_eq!(rect.height, next.y - rect.y, "full row height");
            for editor in &editors {
                assert_eq!(tree.node(*editor).unwrap().cell_editor_presentation, CellEditorPresentation::CellIntegrated);
                assert_eq!(rounded_shell_count(&h.local_fragment(tree.node(*editor).unwrap().id).unwrap(),
                    tree.get_absolute_paint_rect(*editor).unwrap()), 0);
            }
            assert_eq!(tree.node(peer).unwrap().cell_editor_presentation, CellEditorPresentation::Standalone);
            let peer_local = h.local_fragment(tree.node(peer).unwrap().id).unwrap();
            let theme = Theme::light();
            let radius = theme.chrome_radius(crate::core::theme::ControlChromeCategory::RecessedEditable,
                theme.geometry.combo_box.radius, false);
            let expected = [radius, (radius - 1.0 / scale).max(0.0), (radius - 2.0 / scale).max(0.0)]
                .into_iter().filter(|radius| *radius > 0.0).count();
            assert_eq!(rounded_shell_count(&peer_local, tree.get_absolute_paint_rect(peer).unwrap()), expected,
                "standalone retains the original component's exact positive-radius shell paths");
            assert_eq!(tree.cell_edges[&tree.node(body).unwrap().id].len(), 4);
            let initial_pixels = pixels(&mut renderer, &compositor, surface, &first, scale);
            let at = |pixels: &[u8], x: f32, y: f32| {
                let index = ((y * scale).floor() as usize * (360.0 * scale) as usize + (x * scale).floor() as usize) * 4;
                pixels[index..index+4].to_vec()
            };
            assert_eq!(at(&initial_pixels, rect.x + 2.0, rect.y + 2.0),
                at(&initial_pixels, rect.x + 2.0, rect.y + rect.height * 0.5), "square semantic fill at corner");
            let mid_y = rect.y + rect.height * 0.5;
            assert_ne!(at(&initial_pixels, rect.x - 1.0 / scale, mid_y), at(&initial_pixels, rect.x, mid_y),
                "the old column separator must not double the cell's left edge");
            assert_ne!(at(&initial_pixels, rect.x, mid_y), at(&initial_pixels, rect.x + 1.0 / scale, mid_y),
                "one physical neutral edge, then the semantic fill");
            save(&directory, "listview-neutral", &initial_pixels, scale);
            click(&mut tree, root, &mut renderer, &mut domains, &mut router,
                Point::new(rect.right() - 8.0, rect.y + rect.height * 0.5));
            h.advance_transition_time(0.1);
            let opened = frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            assert_eq!(domains.primary_state().focus_manager.focused(), Some(tree.node(editors[0]).unwrap().id));
            assert_eq!(domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);
            let popup = ComboBox::test_popup_local_rect(&tree, editors[0]);
            assert!(popup.height > 0.0, "real arrow click opens original overlay");
            assert_eq!(rounded_shell_count(&h.local_fragment(tree.node(editors[0]).unwrap().id).unwrap(), rect), 0,
                "joined-popup host shell also suppressed");
            assert!(Arc::ptr_eq(&peer_local, &h.local_fragment(tree.node(peer).unwrap().id).unwrap()));
            let opened_pixels = pixels(&mut renderer, &compositor, surface, &opened, scale);
            save(&directory, "listview-popup", &opened_pixels, scale);
            let beta = Point::new(rect.x + popup.x + popup.width * 0.5, rect.y + popup.y + 24.0 * 1.5);
            click(&mut tree, root, &mut renderer, &mut domains, &mut router, beta);
            let committed = frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            assert_eq!(tree.node(editors[0]).unwrap().widget.as_any().downcast_ref::<ComboBox>().unwrap().input_text(), "Beta");
            assert_eq!(commits.get(), 1, "one original committed value transaction");
            let mut after: Vec<_> = tree.children(body).into_iter().filter(|id| tree.node(*id).unwrap().widget.as_any().is::<ComboBox>()).collect();
            after.sort_by(|a,b| tree.get_rect(*a).unwrap().y.total_cmp(&tree.get_rect(*b).unwrap().y));
            assert_eq!(after, editors, "resident value sync preserves every concrete mount");
            save(&directory, "listview-commit", &pixels(&mut renderer, &compositor, surface, &committed, scale), scale);
            receipts.push(serde_json::json!({"scale":scale,"choice_rect":[rect.x,rect.y,rect.width,rect.height],
                "popup_local":[popup.x,popup.y,popup.width,popup.height],"resident_editors":4,
                "capture":"actual routed host","standalone_peer_reused":true,"backend":"Metal"}));
        }
        std::fs::write(directory.join("receipts.json"), serde_json::to_vec_pretty(&receipts).unwrap()).unwrap();
        println!("HAL391 native cell evidence {}", directory.display());
    }

    #[test]
    fn cell_integrated_choice_scroll_resize_reorder_retains_real_mounts() {
        use crate::core::event::PointerButtonsState;
        let scale = 2.0;
        let (mut renderer, compositor, surface) = setup_with_size_and_scale(360, 240, scale).expect("actual Metal");
        assert_eq!(renderer.gpu_resources().adapter.get_info().backend, wgpu::Backend::Metal);
        let directory = std::path::PathBuf::from(format!("target/evidence/hal391/geometry-s2-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut source = ChoiceSource::new();
        for _ in 4..40 {
            let row = RowId::new(); source.rows.push(row); source.labels.insert(row, "Alpha".into());
        }
        let first_rows = [source.rows[0], source.rows[1]];
        let commits = source.commits.clone();
        let mut view = ListView::new(source);
        view.add_column("Name", 100.0);
        let column = view.add_column("Choice", 120.0);
        view.add_column("Other", 300.0);
        view.set_column_combo_options(column, ["Alpha", "Beta", "Gamma", "Delta"].into_iter()
            .map(|value| (value.to_owned(), value.to_owned())).collect());
        let link = HorizontalScrollLink::new(); let freeze = Rc::new(Cell::new(false));
        view.set_horizontal_scroll_link(link.clone(), freeze.clone());
        let body_widget = view.create_body_widget(); let footer = view.create_footer_widget();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let list = tree.add_child(root, Box::new(view), None, 0);
        tree.set_flex(list, FlexParams::flexible(1.0));
        let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(link).with_horizontal_freeze(freeze)), None, 0);
        let body = tree.add_child(scroll, Box::new(body_widget), None, 0);
        tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().wire_content(body);
        tree.add_child(list, Box::new(footer), None, 0);
        let peer = tree.add_child(root, Box::new(ComboBox::simple(vec!["Standalone".into()])), None, 0);
        let mut domains = InteractionDomains::new(); let mut router = EventRouter::new();
        let mut h = RetainedSceneTestHarness::new();
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        let members = |tree: &WidgetTree| -> HashMap<u64, crate::utils::id::TreeWidgetId> {
            tree.cell_edges[&tree.node(body).unwrap().id].iter()
                .filter(|((_, col), _)| *col == column).map(|((row, _), edge)| (*row, edge.editor.owner())).collect()
        };
        let before = members(&tree);
        let owner = before[&first_rows[0].raw()];
        let editor = tree.find_by_id(owner).unwrap();
        let mount = tree.concrete_mount(owner).unwrap();
        assert_eq!(tree.get_rect(editor).unwrap().width, 120.0);
        assert_eq!(tree.get_rect(editor).unwrap().height, 24.0);
        tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().scroll_to(40.0, 0.0);
        tree.mark_layout_dirty(scroll);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        let rect = tree.get_absolute_paint_rect(editor).unwrap();
        let (sx, sy) = tree.cumulative_scroll_offset(editor);
        assert_eq!(sx, 40.0); assert_eq!(sy, 0.0);
        assert_eq!(tree.concrete_mount(owner).unwrap(), mount);
        let hover = Point::new(rect.right() - sx - 8.0, rect.y + rect.height * 0.5);
        route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY, time: Instant::now(), pos: hover, delta: (0.0, 0.0),
            device: PointerDevice::Mouse, buttons: PointerButtonsState::default(),
        });
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        assert_eq!(tree.cell_edges[&tree.node(body).unwrap().id][&(first_rows[0].raw(), column)].state,
            crate::core::theme::ControlChromeState::Hovered);
        // Select through the original plain Name cell, not the editor's arrow.
        click(&mut tree, root, &mut renderer, &mut domains, &mut router,
            Point::new(rect.x - sx - 20.0, rect.y + rect.height * 0.5));
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        assert!(tree.node(list).unwrap().widget.as_any().downcast_ref::<ListView<ChoiceSource>>().unwrap()
            .test_structural_probe_entry().selected_rows.contains(&first_rows[0].raw()));
        let mut sizes = Vec::new();
        for width in [160.0, 32.0] {
            let view = tree.node(list).unwrap().widget.as_any().downcast_ref::<ListView<ChoiceSource>>().unwrap();
            let slot = view.test_column_probe_entries().into_iter().find(|slot| slot.column_id == column).unwrap();
            let header = view.test_structural_probe_entry().header_rect;
            let from = Point::new(header.x + slot.separator_x_viewport.unwrap(), header.y + header.height * 0.5);
            let to = Point::new(from.x + width - slot.resolved_width.unwrap(), from.y);
            route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY, time: Instant::now(), pos: from, device: PointerDevice::Mouse,
                button: PointerButton::Primary, click_count: 1,
            });
            route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY, time: Instant::now(), pos: to, delta: (to.x-from.x, 0.0),
                device: PointerDevice::Mouse, buttons: PointerButtonsState { primary: true, ..Default::default() },
            });
            route(&mut tree, root, &mut renderer, &mut domains, &mut router, FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY, time: Instant::now(), pos: to, device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            });
            frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            assert_eq!(tree.get_rect(editor).unwrap().width, width, "real separator route changes full allocation");
            assert_eq!(tree.get_rect(editor).unwrap().height, 24.0);
            assert_eq!(tree.concrete_mount(owner).unwrap(), mount, "resize cannot remount the resident editor");
            let rect = tree.get_absolute_paint_rect(editor).unwrap();
            let (sx, sy) = tree.cumulative_scroll_offset(editor);
            click(&mut tree, root, &mut renderer, &mut domains, &mut router,
                Point::new(rect.right() - sx - 8.0, rect.y - sy + rect.height * 0.5));
            frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            h.advance_transition_time(if width == 160.0 { 1.0 } else { 2.0 });
            let opened = frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            let (row, background, translation) = find_popup_row(&opened.scene_root, Point::new(0.0,0.0)).unwrap();
            let popup = RectF::new(row.position.x + background.x_offset + translation.x,
                row.position.y + background.y_offset + translation.y, background.width, 96.0);
            assert_eq!(popup.x, rect.x - sx); assert_eq!(popup.y, rect.bottom() - sy);
            assert_eq!(rounded_shell_count(&h.local_fragment(owner).unwrap(), rect), 0);
            let image = pixels(&mut renderer, &compositor, surface, &opened, scale);
            save(&directory, &format!("choice-{width}-popup"), &image, scale);
            assert_eq!(tree.get_rect(editor).unwrap().width, width, "arrow click is not separator resize");
            let (option, expected) = if width == 160.0 { (1.0, "Beta") } else { (2.0, "Gamma") };
            click(&mut tree, root, &mut renderer, &mut domains, &mut router,
                Point::new(popup.x + popup.width * 0.5, popup.y + 24.0 * (option + 0.5)));
            frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
            assert_eq!(tree.node(editor).unwrap().widget.as_any().downcast_ref::<ComboBox>().unwrap().input_text(), expected);
            sizes.push(serde_json::json!({"width":width,"rect":[rect.x,rect.y,rect.width,rect.height],
                "scroll":[sx,sy],"popup":[popup.x,popup.y,popup.width,popup.height]}));
        }
        assert_eq!(commits.get(), 2, "one original value transaction for each distinct selected option");
        // Provider-authored identity order replacement, followed by normal Layout/Phase 2.
        {
            let view = tree.node_mut(list).unwrap().widget.as_any_mut().downcast_mut::<ListView<ChoiceSource>>().unwrap();
            let mut adapter = view.adapter_mut(); let source = adapter.source_mut();
            source.rows.swap(0, 1); source.pending.push(DataInvalidationEvent::BaseOrderReplaced);
        }
        tree.mark_layout_dirty(list);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        let reordered = members(&tree);
        for row in first_rows { assert_eq!(reordered[&row.raw()], before[&row.raw()], "RowId survives order epoch"); }
        assert_eq!(tree.concrete_mount(owner).unwrap(), mount);
        // A real one-row virtual window change keeps overlapping realized RowIds.
        tree.node_mut(scroll).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>().unwrap().scroll_to(40.0, 24.0);
        tree.mark_layout_dirty(scroll);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        let shifted = members(&tree);
        let overlap: Vec<_> = shifted.keys().filter(|row| reordered.contains_key(row)).copied().collect();
        assert!(!overlap.is_empty());
        for row in &overlap { assert_eq!(shifted[row], reordered[row]); }
        h.advance_transition_time(3.0);
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        let body_local = h.local_fragment(tree.node(body).unwrap().id).unwrap();
        let peer_local = h.local_fragment(tree.node(peer).unwrap().id).unwrap();
        frame(&mut renderer, surface, &mut tree, root, &mut h, &mut domains, &mut router);
        assert!(Arc::ptr_eq(&body_local, &h.local_fragment(tree.node(body).unwrap().id).unwrap()));
        assert!(Arc::ptr_eq(&peer_local, &h.local_fragment(tree.node(peer).unwrap().id).unwrap()));
        std::fs::write(directory.join("receipt.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "backend":"Metal","scale":scale,"default_slot":[120,24],"sizes":sizes,
            "row_reorder":"provider BaseOrderReplaced, concrete mounts equal",
            "virtualized_overlap":overlap,"retained_body_and_peer_equal":true,"commits":commits.get()
        })).unwrap()).unwrap();
        println!("HAL391 S2 geometry evidence {}",directory.display());
    }

    #[test]
    fn cell_integrated_session_exit_and_tree_clip_keep_one_current_edge() {
        use crate::core::appearance::FrameworkAppearanceSnapshot;
        use crate::widgets::text_field::TextField;
        let scale = 2.0;
        let directory = std::path::PathBuf::from(format!("target/evidence/hal391/exit-s2-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut receipts = Vec::new();
        for hierarchical in [false, true] {
            let (mut renderer, compositor, surface) = setup_with_size_and_scale(360,240,scale).expect("actual Metal");
            assert_eq!(renderer.gpu_resources().adapter.get_info().backend, wgpu::Backend::Metal);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()),None,0);
            let (view,body,column,source) = mount_session_consumer(&mut tree,root,hierarchical);
            let peer = tree.add_child(root, Box::new(ComboBox::simple(vec!["Standalone".into()])),None,0);
            let mut domains = InteractionDomains::new(); let mut router = EventRouter::new();
            let mut h = RetainedSceneTestHarness::new();
            let initial = frame(&mut renderer,surface,&mut tree,root,&mut h,&mut domains,&mut router);
            let initial_pixels = pixels(&mut renderer,&compositor,surface,&initial,scale);
            let body_rect = tree.get_absolute_paint_rect(body).unwrap();
            let hts = if hierarchical {
                tree.node(view).unwrap().widget.as_any().downcast_ref::<crate::widgets::tree_view::TreeView>().unwrap().hts_handle()
            } else {
                tree.node(view).unwrap().widget.as_any().downcast_ref::<crate::widgets::property_editor::PropertyEditor>().unwrap().hts_handle()
            };
            let (slot_x,slot_width,row_height) = {
                let hts = hts.borrow();
                let slot = hts.column_layout().body_slots().find(|slot|slot.column_id==column).unwrap();
                (slot.x,slot.width,hts.row_height())
            };
            let field_cell = Point::new(body_rect.x + slot_x + slot_width*0.5, body_rect.y + row_height*0.5);
            click(&mut tree,root,&mut renderer,&mut domains,&mut router,field_cell);
            route(&mut tree,root,&mut renderer,&mut domains,&mut router,FusedInputEvent::PointerDown {
                window_id:WindowId::PRIMARY,time:Instant::now(),pos:field_cell,device:PointerDevice::Mouse,
                button:PointerButton::Primary,click_count:2,
            });
            route(&mut tree,root,&mut renderer,&mut domains,&mut router,FusedInputEvent::PointerUp {
                window_id:WindowId::PRIMARY,time:Instant::now(),pos:field_cell,device:PointerDevice::Mouse,
                button:PointerButton::Primary,
            });
            frame(&mut renderer,surface,&mut tree,root,&mut h,&mut domains,&mut router);
            crate::core::app::apply_deferred_focus_requests(&mut tree,&mut router,domains.primary_state_mut(),renderer.text_engine_mut());
            let captured = frame(&mut renderer,surface,&mut tree,root,&mut h,&mut domains,&mut router);
            let field = tree.children(body).into_iter().find(|node|tree.node(*node).unwrap().widget.as_any().is::<TextField>()).unwrap();
            let owner = tree.node(field).unwrap().id;
            let mount = tree.concrete_mount(owner).unwrap();
            let rect = tree.get_absolute_paint_rect(field).unwrap();
            let captured_pixels = pixels(&mut renderer,&compositor,surface,&captured,scale);
            if hierarchical {
                let clip = tree.node(body).unwrap().widget.clip_bounds().unwrap();
                assert!(clip.right() >= rect.right() - body_rect.x, "real second-column editor is within child clip");
                assert!(rect.x >= body_rect.x + 200.0);
                let toggles: Vec<_> = tree.children(body).into_iter().filter(|node|tree.node(*node).unwrap().widget
                    .as_any().is::<crate::widgets::disclosure_toggle::DisclosureToggle>()).collect();
                assert!(!toggles.is_empty());
                for toggle in toggles {
                    let toggle = tree.get_absolute_paint_rect(toggle).unwrap();
                    assert!(toggle.x >= body_rect.x && toggle.right() <= body_rect.x + 199.0,
                        "extending the child clip never changes disclosure's tree-column admission");
                    // This is an unselected different row. The new editor must
                    // neither erase its disclosure nor paint into its second cell.
                    for y in (toggle.y*scale) as u32..(toggle.bottom()*scale) as u32 {
                        for x in (toggle.x*scale) as u32..(320.0*scale) as u32 {
                            assert_eq!(pixel_at(&captured_pixels,720,x,y),pixel_at(&initial_pixels,720,x,y));
                        }
                    }
                }
            }
            let name = if hierarchical {"treeview"} else {"property-editor"};
            save(&directory,&format!("{name}-captured"),&captured_pixels,scale);
            route(&mut tree,root,&mut renderer,&mut domains,&mut router,FusedInputEvent::TextInput {
                window_id:WindowId::PRIMARY,time:Instant::now(),text:"Updated мир".into(),modifiers:Modifiers::default(),
            });
            assert_eq!(tree.node(field).unwrap().widget.as_any().downcast_ref::<TextField>().unwrap().text(),"Updated мир");
            route(&mut tree,root,&mut renderer,&mut domains,&mut router,FusedInputEvent::KeyDown {
                window_id:WindowId::PRIMARY,time:Instant::now(),key_code:KeyCode::Enter,
                physical_key:PhysicalKeyCode::Enter,reservation:None,modifiers:Modifiers::default(),repeat:false,
            });
            // Observe the actual post-exit/pre-Layout cut. No dirty bits, owner
            // state, session or mount are manufactured to create this cut.
            assert_eq!(tree.concrete_mount(owner).unwrap(),mount);
            let snapshot = FrameworkAppearanceSnapshot::capture(&domains,WindowId::PRIMARY,&tree).unwrap();
            tree.prepare_cell_edges(root,Some(&snapshot));
            let edge = tree.cell_edges[&tree.node(body).unwrap().id][&(1,column)].clone();
            assert_eq!(edge.editor,mount);
            assert!(snapshot.for_owner(owner).focused());
            assert!(!snapshot.for_owner(owner).keyboard_captured());
            assert_eq!(edge.state,crate::core::theme::ControlChromeState::Focused,
                "the still-mounted editor keeps one real focused cell edge after Capture release");
            assert_eq!(rounded_shell_count(&h.local_fragment(owner).unwrap(),rect),0);
            let exited = frame(&mut renderer,surface,&mut tree,root,&mut h,&mut domains,&mut router);
            assert!(tree.find_by_id(owner).is_none()); assert!(!mount.is_live());
            assert!(!tree.cell_edges.get(&tree.node(body).unwrap().id).is_some_and(|edges|edges.contains_key(&(1,column))));
            assert_eq!(source.borrow().revision,1,"exactly one original commit, no cancellation duplicate");
            assert!(matches!(&source.borrow().value,crate::engines::cell_source::PropertyValue::String(text) if text=="Updated мир"));
            save(&directory,&format!("{name}-exited"),&pixels(&mut renderer,&compositor,surface,&exited,scale),scale);
            frame(&mut renderer,surface,&mut tree,root,&mut h,&mut domains,&mut router);
            let body_local = h.local_fragment(tree.node(body).unwrap().id).unwrap();
            let peer_local = h.local_fragment(tree.node(peer).unwrap().id).unwrap();
            frame(&mut renderer,surface,&mut tree,root,&mut h,&mut domains,&mut router);
            assert!(Arc::ptr_eq(&body_local,&h.local_fragment(tree.node(body).unwrap().id).unwrap()));
            assert!(Arc::ptr_eq(&peer_local,&h.local_fragment(tree.node(peer).unwrap().id).unwrap()));
            receipts.push(serde_json::json!({"consumer":name,"scale":scale,
                "pre_unmount_edge":"actual Focused after original Enter/Capture release",
                "post_unmount":"retired mount, no stale integrated edge","commits":source.borrow().revision,
                "quiet_consumer_and_peer_reused":true}));
        }
        std::fs::write(directory.join("receipts.json"),serde_json::to_vec_pretty(&receipts).unwrap()).unwrap();
        println!("HAL391 S2 exit evidence {}",directory.display());
    }
}

struct ComboListSource {
    rows: Vec<RowId>,
}

impl ComboListSource {
    fn new(row_count: usize) -> Self {
        Self {
            rows: (0..row_count).map(|_| RowId::new()).collect(),
        }
    }
}

impl DataSource for ComboListSource {
    fn row_ids(&self) -> &[RowId] {
        &self.rows
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Snapshot
    }

    fn read_cell(&self, row: RowId, _column: ColumnId) -> ValueState<CellValue> {
        if self.rows.contains(&row) {
            ValueState::Ready(CellValue::String("Alpha".to_string()))
        } else {
            ValueState::Missing
        }
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities {
            visible: true,
            readable: true,
            editable: true,
            committable: true,
            redacted: false,
        }
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        Vec::new()
    }
}

fn rgb_distance(lhs: [u8; 4], rhs: [u8; 4]) -> u16 {
    lhs[..3]
        .iter()
        .zip(rhs[..3].iter())
        .map(|(lhs, rhs)| u16::from(lhs.abs_diff(*rhs)))
        .sum()
}

#[test]
fn list_view_combo_box_popup_has_visible_joined_border_at_retina_scale() {
    use crate::core::event::InteractionDomains;
    const POPUP_WIDTH: u32 = 150;
    const TARGET_WIDTH: u32 = 360;
    const TARGET_HEIGHT: u32 = 240;

    let scale_factor = 2.0;
    let (mut renderer, compositor, surface_id) =
        setup_with_size_and_scale(TARGET_WIDTH, TARGET_HEIGHT, scale_factor)
            .expect("ListView popup qualification requires actual GPU readback");

    let mut list_view = ListView::new(ComboListSource::new(8));
    list_view.add_column_raw(
        "Index",
        Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft),
    );
    list_view.add_column("Name", 200.0);
    list_view.add_column("Description", 400.0);
    list_view.add_column("Status", 150.0);
    let choice = list_view.add_column("Choice", 120.0);
    list_view.add_column("Category", 200.0);
    list_view.set_column_combo_options(
        choice,
        ["Alpha", "Beta", "Gamma", "Delta"]
            .into_iter()
            .map(|value| (value.to_string(), value.to_string()))
            .collect(),
    );
    let horizontal_link = HorizontalScrollLink::new();
    let horizontal_freeze = Rc::new(Cell::new(false));
    list_view.set_horizontal_scroll_link(horizontal_link.clone(), horizontal_freeze.clone());
    let body_widget = list_view.create_body_widget();
    let footer_widget = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::flexible(1.0));
    let scroll = tree.add_child(
        list,
        Box::new(
            ScrollArea::new_unwired()
                .enable_horizontal_scrollbar(true)
                .with_horizontal_scroll_link(horizontal_link)
                .with_horizontal_freeze(horizontal_freeze),
        ),
        None,
        0,
    );
    let body = tree.add_child(scroll, Box::new(body_widget), None, 0);
    tree.node_mut(scroll)
        .and_then(|node| node.widget.as_any_mut().downcast_mut::<ScrollArea>())
        .expect("ListView ScrollArea")
        .wire_content(body);
    tree.add_child(list, Box::new(footer_widget), None, 0);

    let theme = Theme::light();
    let icons = builtin_registry();
    let mut harness = RetainedSceneTestHarness::new();
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let constraints = Constraints::tight(Size::new(TARGET_WIDTH as f32, TARGET_HEIGHT as f32));
    let render_frame = |tree: &mut WidgetTree, renderer: &mut WgpuRenderer,
                        harness: &mut RetainedSceneTestHarness,
                        domains: &mut InteractionDomains, router: &mut EventRouter| {
        // Layout publishes roles; the next ordinary Phase 2 owns installation.
        tree.process_composite_choice_work(router, domains.primary_state_mut(), renderer.text_engine_mut(), 0);
        harness.frame_with_appearance(tree, root, constraints, &theme,
            renderer.text_engine_mut(), &icons, domains, WindowId::PRIMARY)
    };
    let _initial = render_frame(&mut tree, &mut renderer, &mut harness, &mut domains, &mut router);
    tree.node_mut(scroll)
        .and_then(|node| node.widget.as_any_mut().downcast_mut::<ScrollArea>())
        .expect("ListView ScrollArea")
        .scroll_to(746.0, 0.0);
    tree.mark_subtree_paint_dirty(scroll);
    let _scrolled = render_frame(&mut tree, &mut renderer, &mut harness, &mut domains, &mut router);
    let _ready = render_frame(&mut tree, &mut renderer, &mut harness, &mut domains, &mut router);
    let combo_node = tree
        .iter_nodes()
        .filter_map(|(node_id, node)| {
            node.widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .map(|_| (node_id, tree.get_rect(node_id).expect("ComboBox rect")))
        })
        .min_by(|(_, lhs), (_, rhs)| lhs.y.total_cmp(&rhs.y))
        .map(|(node_id, _)| node_id)
        .expect("realized ListView ComboBox");
    let control_rect = tree.get_absolute_paint_rect(combo_node).unwrap();
    let (control_scroll_x, control_scroll_y) = tree.cumulative_scroll_offset(combo_node);
    assert_eq!(control_rect.width, 120.0, "the original full Choice slot");
    assert_eq!(control_rect.height, 24.0, "the original full row");
    let arrow = Point::new(control_rect.right() - control_scroll_x - 8.0,
        control_rect.y - control_scroll_y + control_rect.height * 0.5);
    for event in [
        FusedInputEvent::PointerDown { window_id: WindowId::PRIMARY, time: Instant::now(), pos: arrow,
            device: PointerDevice::Mouse, button: PointerButton::Primary, click_count: 1 },
        FusedInputEvent::PointerUp { window_id: WindowId::PRIMARY, time: Instant::now(), pos: arrow,
            device: PointerDevice::Mouse, button: PointerButton::Primary },
    ] {
        router.route_input_event(&mut tree, event, Some(root), renderer.text_engine_mut(), None,
            domains.primary_state_mut(), &mut BindingRegistry::new());
    }
    assert_eq!(domains.primary_state().focus_manager.focused(), Some(tree.node(combo_node).unwrap().id));
    assert_eq!(domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);
    let _opened = render_frame(&mut tree, &mut renderer, &mut harness, &mut domains, &mut router);
    harness.advance_transition_time(1.0);
    let frame = render_frame(&mut tree, &mut renderer, &mut harness, &mut domains, &mut router);
    let button_rect = RectF::new(TARGET_WIDTH as f32 - 28.0, 0.0, 24.0, 24.0);
    let icon_rect = RectF::new(button_rect.x + 4.0, button_rect.y + 4.0, 16.0, 16.0);
    let icon_color: Color = theme.colors.typography.primary.into();
    let button_color: Color = theme.colors.surfaces.level3.into();
    let mut settings_paths = 0;
    let mut button_present = false;
    visit_scene_nodes(&frame.scene_root, &mut |node| match node {
        SceneNode::Text(text) => assert!(
            !text.text.contains('\u{2699}'),
            "default ListView settings must not depend on font coverage"
        ),
        SceneNode::Rect(rect) if rect.rect == button_rect => {
            button_present |= rect.color == button_color && rect.z_index == LAYER_SCROLLBAR + 1;
        }
        SceneNode::Path(path) if path.z_index == LAYER_SCROLLBAR + 2 => {
            let bounds = path.path.bounds();
            if bounds.x >= button_rect.x && bounds.x < button_rect.x + button_rect.width {
                assert!(bounds.x >= icon_rect.x && bounds.y >= icon_rect.y);
                assert!(bounds.x + bounds.width <= icon_rect.x + icon_rect.width);
                assert!(bounds.y + bounds.height <= icon_rect.y + icon_rect.height);
                assert_eq!(path.color, icon_color);
                assert!(!path.path.to_mesh().vertices.is_empty());
                settings_paths += 1;
            }
        }
        _ => {}
    });
    assert!(
        button_present,
        "the 24x24 settings button must remain present"
    );
    assert_eq!(
        settings_paths,
        icons
            .get(IconId::SETTINGS)
            .expect("built-in settings icon")
            .layers
            .len(),
        "all registered settings layers must reach the retained Path pipeline"
    );
    let (row, row_background, translation) =
        find_popup_row(&frame.scene_root, Point::new(0.0, 0.0)).expect("open ComboBox popup row");
    let popup_width = row_background.width;
    let popup_height = 4.0 * 24.0;
    assert_eq!(popup_width, POPUP_WIDTH as f32);

    let border_rect = RectF::new(
        row.position.x + row_background.x_offset + translation.x,
        row.position.y + row_background.y_offset + translation.y,
        popup_width,
        popup_height,
    );
    assert_eq!(border_rect.x, control_rect.x - control_scroll_x, "current full-cell popup anchor");
    assert_eq!(border_rect.y, control_rect.bottom() - control_scroll_y);
    let mut scene = SceneGraph::new();
    scene.root = frame.scene_root;
    scene.clear_color = theme.colors.surfaces.base0.into();
    let pixels = render_scene_at_scale(&mut renderer, &compositor, surface_id, scene, scale_factor);

    let physical_width = (TARGET_WIDTH as f32 * scale_factor) as u32;
    let button_background = pixel_at(
        &pixels,
        physical_width,
        ((button_rect.x + 2.0) * scale_factor) as u32,
        ((button_rect.y + 2.0) * scale_factor) as u32,
    );
    let mut settings_ink = 0;
    for y in (icon_rect.y * scale_factor) as u32
        ..((icon_rect.y + icon_rect.height) * scale_factor) as u32
    {
        for x in (icon_rect.x * scale_factor) as u32
            ..((icon_rect.x + icon_rect.width) * scale_factor) as u32
        {
            if rgb_distance(pixel_at(&pixels, physical_width, x, y), button_background) >= 100 {
                settings_ink += 1;
            }
        }
    }
    assert!(
        settings_ink >= 30,
        "settings vector paths must produce visible GPU pixels"
    );
    assert_eq!(
        pixel_at(
            &pixels,
            physical_width,
            ((icon_rect.x + 8.0) * scale_factor) as u32,
            ((icon_rect.y + 8.0) * scale_factor) as u32,
        ),
        button_background,
        "the gear's center must remain open"
    );
    let x0 = (border_rect.x * scale_factor).round() as u32;
    let y0 = (border_rect.y * scale_factor).round() as u32;
    let x1 = ((border_rect.x + border_rect.width) * scale_factor).round() as u32;
    let y1 = ((border_rect.y + border_rect.height) * scale_factor).round() as u32;
    let middle_x = (x0 + x1) / 2;
    let middle_y = (y0 + y1) / 2;

    // HAL391 removes the host's joined input shell. The independent popup
    // retains its own actual Captured appearance; do not sample an old shell.
    let chrome = theme.input_chrome(crate::core::theme::ControlChromeState::Captured).chrome;
    assert_eq!((chrome.border_width * scale_factor).round(), 2.0);
    assert_eq!(chrome.border_color.a, 1.0);
    let srgb = |linear: f32| -> u8 {
        let encoded = if linear <= 0.0031308 { linear * 12.92 }
            else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };
        (encoded * 255.0).round() as u8
    };
    let popup_border = [srgb(chrome.border_color.r), srgb(chrome.border_color.g), srgb(chrome.border_color.b), 255];
    let cell_x = control_rect.x - control_scroll_x;
    let cell_y = control_rect.y - control_scroll_y;
    assert_eq!(pixel_at(&pixels, physical_width,
        ((cell_x + control_rect.width * 0.5) * scale_factor) as u32,
        ((cell_y + 0.5) * scale_factor) as u32), popup_border,
        "the cell's single prepared Capture edge uses the qualified theme source");
    assert_eq!(pixel_at(&pixels, physical_width,
        ((cell_x + 2.0) * scale_factor) as u32, ((cell_y + 2.0) * scale_factor) as u32),
        pixel_at(&pixels, physical_width,
            ((cell_x + 2.0) * scale_factor) as u32, ((cell_y + 12.0) * scale_factor) as u32),
        "square full-cell fill without an inset rounded corner");

    let edge_bands = [
        (
            "left",
            pixel_at(&pixels, physical_width, x0, middle_y),
            pixel_at(&pixels, physical_width, x0 + 1, middle_y),
            pixel_at(&pixels, physical_width, x0 + 2, middle_y),
        ),
        (
            "right",
            pixel_at(&pixels, physical_width, x1 - 1, middle_y),
            pixel_at(&pixels, physical_width, x1 - 2, middle_y),
            pixel_at(&pixels, physical_width, x1 - 3, middle_y),
        ),
        (
            "bottom",
            pixel_at(&pixels, physical_width, middle_x, y1 - 1),
            pixel_at(&pixels, physical_width, middle_x, y1 - 2),
            pixel_at(&pixels, physical_width, middle_x, y1 - 3),
        ),
    ];

    assert_eq!(
        pixel_at(&pixels, physical_width, middle_x, y0),
        pixel_at(&pixels, physical_width, middle_x, y0 + 2),
        "the scrolled input/popup contact must remain open"
    );

    for (side, outer, inner, popup) in edge_bands {
        assert_eq!(
            outer, popup_border,
            "{side} independent popup border must match its published Captured chrome"
        );
        assert_eq!(
            outer, inner,
            "{side} popup border must occupy two physical pixels at 2x: outer={outer:?}, inner={inner:?}"
        );
        assert!(
            rgb_distance(outer, popup) >= 100,
            "{side} popup border must retain visible contrast against its opaque row background: border={outer:?}, popup={popup:?}"
        );
    }
    let directory = std::path::PathBuf::from(format!("target/evidence/hal391/original-popup-s2-{}",std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut ppm = format!("P6\n{} {}\n255\n",physical_width,(TARGET_HEIGHT as f32*scale_factor) as u32).into_bytes();
    for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
    std::fs::write(directory.join("popup.ppm"),ppm).unwrap();
    std::fs::write(directory.join("receipt.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "scale":scale_factor,"cell":[cell_x,cell_y,control_rect.width,control_rect.height],
        "scroll":[control_scroll_x,control_scroll_y],"popup":[border_rect.x,border_rect.y,border_rect.width,border_rect.height],
        "settings_ink":settings_ink,"popup_border_rgba":popup_border,"gear_path_complete":true,
        "independent_popup_two_pixel_bands":true,"cell_square_one_capture_edge":true
    })).unwrap()).unwrap();
    println!("HAL391 S2 original popup evidence {}",directory.display());
}

#[test]
fn combo_joined_capture_perimeter_gpu_preserves_contact_and_ledges() {
    use crate::core::event::InteractionDomains;
    use crate::core::layout::EdgeInsets;

    const W: u32 = 360;
    const H: u32 = 280;
    let scale = 2.0;
    let (mut renderer, compositor, surface) = setup_with_size_and_scale(W, H, scale)
        .expect("combo_joined_capture_perimeter requires a real adapter and GPU readback");
    let theme = Theme::light();
    let icons = builtin_registry();
    for (top, left, right, above) in [(20.0, 20.0, 210.0, false), (230.0, 250.0, 20.0, true)] {
        for actual_capture in [false, true] {
            let mut combo = ComboBox::simple(vec![
                "A deliberately wider option".into(),
                "Beta".into(),
                "".into(),
            ]);
            if !actual_capture {
                combo.open_popup();
            }
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Flex::column().with_padding(EdgeInsets {
                    top,
                    left,
                    right,
                    bottom: 0.0,
                })),
                None,
                0,
            );
            let combo_node = tree.add_child(root, Box::new(combo), None, 0);
            let mut harness = RetainedSceneTestHarness::new();
            let mut domains = InteractionDomains::new();
            let constraints = Constraints::tight(Size::new(W as f32, H as f32));
            let initial = harness.frame_with_appearance(
                &mut tree,
                root,
                constraints,
                &theme,
                renderer.text_engine_mut(),
                &icons,
                &domains,
                WindowId::PRIMARY,
            );
            let rect = tree.get_absolute_paint_rect(combo_node).unwrap();
            let frame = if actual_capture {
                let mut router = EventRouter::new();
                let mut bindings = BindingRegistry::new();
                router.route_input_event(
                    &mut tree,
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos: Point::new(rect.x + rect.width - 3.0, rect.y + rect.height * 0.5),
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    },
                    Some(root),
                    renderer.text_engine_mut(),
                    None,
                    domains.primary_state_mut(),
                    &mut bindings,
                );
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
                assert_eq!(
                    domains.primary_state().focus_manager.focused(),
                    Some(tree.node(combo_node).unwrap().id)
                );
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    constraints,
                    &theme,
                    renderer.text_engine_mut(),
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                );
                harness.advance_transition_time(0.05);
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    constraints,
                    &theme,
                    renderer.text_engine_mut(),
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                )
            } else {
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Navigation
                );
                assert_eq!(domains.primary_state().focus_manager.focused(), None);
                initial
            };
            let local_popup = ComboBox::test_popup_local_rect(&tree, combo_node);
            let popup = RectF::new(
                rect.x + local_popup.x,
                rect.y + local_popup.y,
                local_popup.width,
                local_popup.height,
            );
            assert_eq!(popup.y < rect.y, above);
            assert!(popup.width > rect.width + 12.0);
            let mut scene = SceneGraph::new();
            scene.root = frame.scene_root;
            scene.clear_color = theme.colors.surfaces.base0.into();
            let pixels = render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
            let at =
                |x: f32, y: f32| pixel_at(&pixels, W * 2, (x * scale) as u32, (y * scale) as u32);
            let input_outer_y = if above {
                rect.y + rect.height - 0.5
            } else {
                rect.y + 0.5
            };
            let input_border = at(rect.x + rect.width * 0.5, input_outer_y);
            let popup_border = at(popup.x + 0.5, popup.y + popup.height * 0.5);
            assert_eq!(
                input_border, popup_border,
                "main and overlay perimeter must share actual owner appearance, capture={actual_capture}, above={above}"
            );
            let joint = if above { rect.y } else { rect.y + rect.height };
            let seam_x = rect.x.max(popup.x) + 8.0;
            let direction = if above { -1.0 } else { 1.0 };
            let contact = at(seam_x, joint + direction * 0.5);
            let interior = at(seam_x, joint + direction * 2.5);
            assert_eq!(
                contact, interior,
                "the shared contact must have no horizontal border seam"
            );
            assert!(
                rgb_distance(input_border, interior) > 100,
                "the test must distinguish perimeter ink from popup fill"
            );
            let ledge_x = if popup.x + 6.0 < rect.x {
                (popup.x + rect.x) * 0.5
            } else {
                (rect.x + rect.width + popup.x + popup.width) * 0.5
            };
            assert_eq!(
                at(ledge_x, joint + direction * 0.5),
                input_border,
                "the exposed ledge must remain outlined"
            );
        }
    }
}

#[test]
fn combo_box_overlay_pointer_route_isolated_from_list_view() {
    const TARGET_WIDTH: f32 = 420.0;
    const TARGET_HEIGHT: f32 = 280.0;

    let mut list_view = ListView::new(ComboListSource::new(8));
    let choice = list_view.add_column("Choice", 180.0);
    list_view.add_column("Category", 220.0);
    list_view.set_column_combo_options(
        choice,
        ["Alpha", "Beta", "Gamma", "Delta"]
            .into_iter()
            .map(|value| (value.to_string(), value.to_string()))
            .collect(),
    );
    let horizontal_link = HorizontalScrollLink::new();
    let horizontal_freeze = Rc::new(Cell::new(false));
    list_view.set_horizontal_scroll_link(horizontal_link.clone(), horizontal_freeze.clone());
    let body_widget = list_view.create_body_widget();
    let footer_widget = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::flexible(1.0));
    let scroll = tree.add_child(
        list,
        Box::new(
            ScrollArea::new_unwired()
                .enable_horizontal_scrollbar(true)
                .with_horizontal_scroll_link(horizontal_link)
                .with_horizontal_freeze(horizontal_freeze),
        ),
        None,
        0,
    );
    let body = tree.add_child(scroll, Box::new(body_widget), None, 0);
    tree.node_mut(scroll)
        .and_then(|node| node.widget.as_any_mut().downcast_mut::<ScrollArea>())
        .expect("ListView ScrollArea")
        .wire_content(body);
    tree.add_child(list, Box::new(footer_widget), None, 0);

    let theme = Theme::light();
    let mut harness = RetainedSceneTestHarness::new();
    let mut text_engine = FontManagerTextEngine::new();
    let constraints = Constraints::tight(Size::new(TARGET_WIDTH, TARGET_HEIGHT));
    let icons = IconRegistry::new(&[]);
    let _initial = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );

    let combo_node = tree
        .iter_nodes()
        .filter_map(|(node_id, node)| {
            node.widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .map(|_| (node_id, tree.get_absolute_paint_rect(node_id)))
        })
        .filter_map(|(node_id, rect)| rect.map(|rect| (node_id, rect)))
        .min_by(|(_, lhs), (_, rhs)| lhs.y.total_cmp(&rhs.y))
        .map(|(node_id, _)| node_id)
        .expect("realized ListView ComboBox");
    let combo_rect = tree
        .get_absolute_paint_rect(combo_node)
        .expect("ComboBox absolute rect");

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();
    let control_point = Point::new(
        combo_rect.x + combo_rect.width * 0.5,
        combo_rect.y + combo_rect.height * 0.5,
    );
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: control_point,
            delta: (0.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState::default(),
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut bindings,
    );
    assert!(
        tree.node(body)
            .and_then(|node| {
                node.widget
                    .as_any()
                    .downcast_ref::<ListViewBody<ComboListSource>>()
            })
            .and_then(ListViewBody::test_hovered_row)
            .is_some(),
        "ordinary cell hover must still reach ListViewBody"
    );

    tree.node_mut(combo_node)
        .and_then(|node| node.widget.as_any_mut().downcast_mut::<ComboBox>())
        .expect("ListView ComboBox")
        .open_popup();
    tree.mark_paint_dirty(combo_node);
    let _popup = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );

    let popup_rect = ComboBox::test_popup_local_rect(&tree, combo_node);
    let popup_row_height = popup_rect.height / 4.0;
    let beta_point = Point::new(
        combo_rect.x + popup_rect.x + popup_rect.width * 0.5,
        combo_rect.y + popup_rect.y + popup_row_height * 1.5,
    );
    assert_eq!(
        tree.hit_test(beta_point.x, beta_point.y)
            .expect("popup overlay hit")
            .widget_id,
        tree.node(combo_node).expect("ComboBox node").id,
        "popup must own the routed hit"
    );

    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: beta_point,
            delta: (0.0, popup_row_height),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState::default(),
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut bindings,
    );
    assert_eq!(
        tree.node(combo_node)
            .and_then(|node| node.widget.as_any().downcast_ref::<ComboBox>())
            .and_then(ComboBox::test_highlighted_label),
        Some("Beta"),
        "popup owner must receive and highlight its hovered option"
    );
    assert_eq!(
        tree.node(body)
            .and_then(|node| {
                node.widget
                    .as_any()
                    .downcast_ref::<ListViewBody<ComboListSource>>()
            })
            .and_then(ListViewBody::test_hovered_row),
        None,
        "popup hover must clear and stop covered ListView row hover"
    );

    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: beta_point,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut bindings,
    );
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: beta_point,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut bindings,
    );

    assert_eq!(
        tree.node(combo_node)
            .and_then(|node| node.widget.as_any().downcast_ref::<ComboBox>())
            .and_then(ComboBox::committed_value_string),
        Some("Beta".to_string()),
        "popup click must commit the hovered option"
    );
    assert_eq!(
        tree.node(list)
            .and_then(|node| {
                node.widget
                    .as_any()
                    .downcast_ref::<ListView<ComboListSource>>()
            })
            .and_then(ListView::focused_row),
        None,
        "popup click must not project through and select a covered ListView row"
    );
}

fn batch(z_index: u32, batch_index: usize) -> SurfaceDrawCommand {
    SurfaceDrawCommand::Batch {
        order: crate::core::paint::EffectivePaintOrder::scalar(z_index),
        z_index,
        batch_index,
    }
}

fn path(z_index: u32, path_index: usize) -> SurfaceDrawCommand {
    SurfaceDrawCommand::Path {
        order: crate::core::paint::EffectivePaintOrder::scalar(z_index),
        z_index,
        path_index,
    }
}

fn text(z_index: u32, quad_start: u32) -> SurfaceDrawCommand {
    SurfaceDrawCommand::Text {
        order: crate::core::paint::EffectivePaintOrder::scalar(z_index),
        z_index,
        quad_start,
        quad_count: 1,
    }
}

#[test]
fn surface_draw_command_merge_orders_mixed_families() {
    let merged = merge_ordered_surface_draw_commands(
        vec![batch(1, 0), batch(7, 1)].into_iter(),
        vec![path(2, 0), path(8, 1)].into_iter(),
        vec![text(3, 0), text(6, 1)].into_iter(),
    );

    assert_eq!(
        merged,
        vec![
            batch(1, 0),
            path(2, 0),
            text(3, 0),
            text(6, 1),
            batch(7, 1),
            path(8, 1),
        ]
    );
}

#[test]
fn surface_draw_command_merge_preserves_equal_z_family_and_source_order() {
    let merged = merge_ordered_surface_draw_commands(
        vec![batch(10, 0), batch(10, 1)].into_iter(),
        vec![path(10, 0), path(10, 1)].into_iter(),
        vec![text(10, 0), text(10, 1)].into_iter(),
    );

    assert_eq!(
        merged,
        vec![
            batch(10, 0),
            batch(10, 1),
            path(10, 0),
            path(10, 1),
            text(10, 0),
            text(10, 1),
        ]
    );
}

#[test]
fn surface_draw_command_build_skips_empty_paths_without_index_drift() {
    use crate::compositor::SceneKey;
    use crate::renderer::path::{PathDraw, PathMesh};

    fn path_draw(z_index: u32, has_vertices: bool, has_indices: bool) -> PathDraw {
        let mesh = PathMesh {
            vertices: has_vertices
                .then_some(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])
                .unwrap_or_default(),
            indices: has_indices.then_some(vec![0, 1, 2]).unwrap_or_default(),
            bounds: RectF::new(0.0, 0.0, 1.0, 1.0),
        };
        let mut path = PathDraw::fill(mesh, [1.0; 4], SceneKey::BASE);
        path.z_index = z_index;
        path
    }

    let frame_batches = FrameBatches {
        paths: vec![
            path_draw(1, false, true),
            path_draw(2, true, true),
            path_draw(3, true, false),
            path_draw(4, true, true),
        ],
        ..FrameBatches::new()
    };

    assert_eq!(
        build_surface_draw_commands(&frame_batches, vec![text(3, 0)]),
        vec![path(2, 0), text(3, 0), path(4, 1)]
    );
}

#[test]
fn surface_draw_command_merge_preserves_main_overlay_boundary() {
    use crate::core::paint::LAYER_OVERLAY;

    let merged = merge_ordered_surface_draw_commands(
        vec![batch(LAYER_OVERLAY - 1, 0), batch(LAYER_OVERLAY + 1, 1)].into_iter(),
        vec![path(LAYER_OVERLAY, 0)].into_iter(),
        vec![text(LAYER_OVERLAY - 1, 0)].into_iter(),
    );

    let overlay_start = merged.partition_point(|command| command.z_index() < LAYER_OVERLAY);
    assert_eq!(overlay_start, 2);
    assert!(
        merged[..overlay_start]
            .iter()
            .all(|command| command.z_index() < LAYER_OVERLAY)
    );
    assert!(
        merged[overlay_start..]
            .iter()
            .all(|command| command.z_index() >= LAYER_OVERLAY)
    );
}

#[test]
fn surface_draw_command_merge_pulls_each_input_once() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct CountingIterator<I> {
        inner: I,
        pulls: Rc<Cell<usize>>,
    }

    impl<I: Iterator> Iterator for CountingIterator<I> {
        type Item = I::Item;

        fn next(&mut self) -> Option<Self::Item> {
            self.pulls.set(self.pulls.get() + 1);
            self.inner.next()
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.inner.size_hint()
        }
    }

    const COMMANDS_PER_STREAM: usize = 4_096;
    let batches = (0..COMMANDS_PER_STREAM)
        .map(|index| batch(index as u32 * 3, index))
        .collect::<Vec<_>>();
    let paths = (0..COMMANDS_PER_STREAM)
        .map(|index| path(index as u32 * 3 + 1, index))
        .collect::<Vec<_>>();
    let texts = (0..COMMANDS_PER_STREAM)
        .map(|index| text(index as u32 * 3 + 2, index as u32))
        .collect::<Vec<_>>();
    let batch_pulls = Rc::new(Cell::new(0));
    let path_pulls = Rc::new(Cell::new(0));
    let text_pulls = Rc::new(Cell::new(0));

    let merged = merge_ordered_surface_draw_commands(
        CountingIterator {
            inner: batches.into_iter(),
            pulls: Rc::clone(&batch_pulls),
        },
        CountingIterator {
            inner: paths.into_iter(),
            pulls: Rc::clone(&path_pulls),
        },
        CountingIterator {
            inner: texts.into_iter(),
            pulls: Rc::clone(&text_pulls),
        },
    );

    assert_eq!(merged.len(), COMMANDS_PER_STREAM * 3);
    assert!(
        merged
            .windows(2)
            .all(|window| window[0].z_index() <= window[1].z_index())
    );
    assert_eq!(batch_pulls.get(), COMMANDS_PER_STREAM + 1);
    assert_eq!(path_pulls.get(), COMMANDS_PER_STREAM + 1);
    assert_eq!(text_pulls.get(), COMMANDS_PER_STREAM + 1);
}

#[test]
fn strict_z_orders_text_and_opaque_shapes_in_main_and_overlay_scopes() {
    let Some((mut renderer, compositor, surface_id)) = setup() else {
        return;
    };
    let red = Color::from_hex(0xD03030);

    let mut reference = base_scene();
    push_full_rect(&mut reference, red, 20);
    let red_reference = render_scene(&mut renderer, &compositor, surface_id, reference.build());

    let mut text_under = base_scene();
    push_probe_text(&mut text_under, 10);
    push_full_rect(&mut text_under, red, 20);
    assert_eq!(
        render_scene(&mut renderer, &compositor, surface_id, text_under.build()),
        red_reference,
        "higher-Z opaque shape must cover earlier text"
    );

    let mut text_over = base_scene();
    push_full_rect(&mut text_over, red, 10);
    push_probe_text(&mut text_over, 20);
    let text_over_pixels = render_scene(&mut renderer, &compositor, surface_id, text_over.build());
    assert!(
        text_over_pixels
            .chunks_exact(4)
            .zip(red_reference.chunks_exact(4))
            .filter(|(actual, expected)| actual != expected)
            .count()
            > 8,
        "higher-Z text must remain visibly above the opaque shape"
    );

    let mut overlay_reference = base_scene();
    push_full_rect(&mut overlay_reference, red, LAYER_OVERLAY + 20);
    let overlay_red = render_scene(
        &mut renderer,
        &compositor,
        surface_id,
        overlay_reference.build(),
    );

    let mut overlay_text_under = base_scene();
    push_probe_text(&mut overlay_text_under, LAYER_OVERLAY + 10);
    push_full_rect(&mut overlay_text_under, red, LAYER_OVERLAY + 20);
    assert_eq!(
        render_scene(
            &mut renderer,
            &compositor,
            surface_id,
            overlay_text_under.build()
        ),
        overlay_red,
        "overlay scope must not retain the old main-only text exception"
    );

    let mut overlay_text_over = base_scene();
    push_full_rect(&mut overlay_text_over, red, LAYER_OVERLAY + 10);
    push_probe_text(&mut overlay_text_over, LAYER_OVERLAY + 20);
    assert_ne!(
        render_scene(
            &mut renderer,
            &compositor,
            surface_id,
            overlay_text_over.build()
        ),
        overlay_red,
        "higher-Z overlay text must be visible"
    );
}

#[test]
fn event_log_expanded_detail_survives_strict_scene_order() {
    const TARGET_WIDTH: u32 = 640;
    const TARGET_HEIGHT: u32 = 220;
    const DETAIL_LINE: &str = "VISIBLE DETAIL PROBE";

    let mut event_log = EventLog::new(EventLogConfig::default());
    event_log.push(
        EventLogEntry::new(EventLogLevel::Info, 0, "source".into(), "summary".into()).with_detail(
            format!("FIRST DETAIL LINE\n{DETAIL_LINE}\nTHIRD DETAIL LINE"),
        ),
    );
    let body_widget = event_log.create_body_widget();
    let footer_widget = event_log.create_footer_widget();
    let list_view = event_log.take_list_view();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::flexible(1.0));
    let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body_widget), None, 0);
    tree.node_mut(scroll)
        .and_then(|node| node.widget.as_any_mut().downcast_mut::<ScrollArea>())
        .expect("EventLog ScrollArea")
        .wire_content(body);
    tree.add_child(list, Box::new(footer_widget), None, 0);

    let theme = Theme::light();
    let icons = builtin_registry();
    let mut text_engine = FontManagerTextEngine::new();
    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(TARGET_WIDTH as f32, TARGET_HEIGHT as f32));
    let _initial = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );

    let body_rect = tree
        .get_absolute_paint_rect(body)
        .expect("EventLog body rect");
    let position = Point::new(body_rect.x + 80.0, body_rect.y + 12.0);
    let widget_id = tree.node(body).expect("EventLog body").id;
    let tree_ptr = &mut tree as *mut WidgetTree;
    let mut cx = EventContext::new(
        widget_id,
        tree_ptr,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    let double_click = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (position.x, position.y),
        local_pos: (position.x - body_rect.x, position.y - body_rect.y),
        kind: PointerEventKind::DoubleClick,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });
    assert!(
        tree.node_mut(body)
            .expect("EventLog body")
            .widget
            .handle_event(&mut cx, &double_click, EventPhase::Target)
    );

    let frame = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );
    let mut detail_text = None;
    let mut detail_background = None;
    visit_scene_nodes(&frame.scene_root, &mut |node| match node {
        SceneNode::Text(text) if text.text == DETAIL_LINE => {
            detail_text = Some((text.position, text.z_index));
        }
        SceneNode::Rect(rect)
            if rect.color == theme.colors.surfaces.content.into()
                && rect.z_index == LAYER_CONTENT - 1
                && rect.rect.height > 30.0 =>
        {
            detail_background = Some(rect.rect);
        }
        _ => {}
    });
    let (text_position, text_z) = detail_text.expect("mounted detail TextBlock must paint text");
    let background = detail_background.expect("expanded row must paint its detail background");
    assert!(background.y <= text_position.y && text_position.y < background.y + background.height);
    assert!(
        text_z > LAYER_CONTENT - 1,
        "detail text must sort above its background"
    );

    let Some((mut renderer, compositor, surface_id)) = setup_with_size(TARGET_WIDTH, TARGET_HEIGHT)
    else {
        return;
    };
    let mut scene = SceneGraph::new();
    scene.root = frame.scene_root;
    scene.clear_color = theme.colors.surfaces.base0.into();
    let pixels = render_scene(&mut renderer, &compositor, surface_id, scene);
    let background_x = TARGET_WIDTH - 20;
    let mut visible_ink = 0;
    for y in (text_position.y + 2.0) as u32..(text_position.y + 15.0) as u32 {
        let background_pixel = pixel_at(&pixels, TARGET_WIDTH, background_x, y);
        for x in text_position.x as u32..(text_position.x + 180.0) as u32 {
            if rgb_distance(pixel_at(&pixels, TARGET_WIDTH, x, y), background_pixel) > 60 {
                visible_ink += 1;
            }
        }
    }
    assert!(
        visible_ink > 20,
        "expanded detail must produce visible final pixels"
    );
}

#[test]
fn strict_z_preserves_rect_and_image_ordering_controls() {
    let Some((mut renderer, compositor, surface_id)) = setup() else {
        return;
    };
    renderer.register_image_rgba8_srgb(IMAGE_ID, 1, 1, &[0, 255, 0, 255]);
    let red = Color::from_hex(0xD03030);
    let blue = Color::from_hex(0x2050D0);
    let full = RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);

    let mut red_scene = base_scene();
    push_full_rect(&mut red_scene, red, 20);
    let red_reference = render_scene(&mut renderer, &compositor, surface_id, red_scene.build());

    let mut blue_scene = base_scene();
    push_full_rect(&mut blue_scene, blue, 20);
    let blue_reference = render_scene(&mut renderer, &compositor, surface_id, blue_scene.build());

    let mut rects = base_scene();
    push_full_rect(&mut rects, blue, 10);
    push_full_rect(&mut rects, red, 20);
    assert_eq!(
        render_scene(&mut renderer, &compositor, surface_id, rects.build()),
        red_reference,
        "existing rect/rect strict-Z order must remain unchanged"
    );

    let mut reverse_rects = base_scene();
    push_full_rect(&mut reverse_rects, red, 10);
    push_full_rect(&mut reverse_rects, blue, 20);
    assert_eq!(
        render_scene(
            &mut renderer,
            &compositor,
            surface_id,
            reverse_rects.build()
        ),
        blue_reference,
        "rect/rect order must remain two-sided"
    );

    let mut image_scene = base_scene();
    image_scene.push_image(
        full,
        IMAGE_ID,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        20,
        None,
    );
    let image_reference = render_scene(&mut renderer, &compositor, surface_id, image_scene.build());

    let mut image_under_rect = base_scene();
    image_under_rect.push_image(
        full,
        IMAGE_ID,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        10,
        None,
    );
    push_full_rect(&mut image_under_rect, red, 20);
    assert_eq!(
        render_scene(
            &mut renderer,
            &compositor,
            surface_id,
            image_under_rect.build()
        ),
        red_reference,
        "higher-Z rect must cover an image"
    );

    let mut image_over_rect = base_scene();
    push_full_rect(&mut image_over_rect, red, 10);
    image_over_rect.push_image(
        full,
        IMAGE_ID,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        20,
        None,
    );
    assert_eq!(
        render_scene(
            &mut renderer,
            &compositor,
            surface_id,
            image_over_rect.build()
        ),
        image_reference,
        "higher-Z image must cover a rect"
    );
}

#[test]
fn strict_z_orders_executable_paths_without_empty_path_index_drift() {
    let Some((mut renderer, compositor, surface_id)) = setup() else {
        return;
    };
    let red = Color::from_hex(0xD03030);
    let blue = Color::from_hex(0x2050D0);

    let mut red_reference = base_scene();
    push_full_rect(&mut red_reference, red, 20);
    let red_reference = render_scene(
        &mut renderer,
        &compositor,
        surface_id,
        red_reference.build(),
    );

    let mut path_reference = base_scene();
    path_reference.push_path(Path::new(), Color::WHITE, 5, None);
    push_full_path(&mut path_reference, blue, 20);
    let path_reference = render_scene(
        &mut renderer,
        &compositor,
        surface_id,
        path_reference.build(),
    );
    assert_ne!(
        path_reference, red_reference,
        "executable path reference must produce visible pixels"
    );

    let mut path_under_rect = base_scene();
    path_under_rect.push_path(Path::new(), Color::WHITE, 5, None);
    push_full_path(&mut path_under_rect, blue, 10);
    push_full_rect(&mut path_under_rect, red, 20);
    assert_eq!(
        render_scene(
            &mut renderer,
            &compositor,
            surface_id,
            path_under_rect.build()
        ),
        red_reference,
        "higher-Z rect must cover an executable path after an empty path"
    );

    let mut path_over_rect = base_scene();
    push_full_rect(&mut path_over_rect, red, 10);
    path_over_rect.push_path(Path::new(), Color::WHITE, 15, None);
    push_full_path(&mut path_over_rect, blue, 20);
    assert_eq!(
        render_scene(
            &mut renderer,
            &compositor,
            surface_id,
            path_over_rect.build()
        ),
        path_reference,
        "higher-Z executable path must cover a rect without empty-path index drift"
    );
}

mod control_gpu_evidence;
