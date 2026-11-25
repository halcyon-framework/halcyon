#![cfg(target_os = "macos")]

use std::collections::HashMap;

use crate::compositor::{Compositor, MultiSurfaceContract, SceneKey, SurfaceId};
use crate::core::math::{Point, RectF};
use crate::core::paint::Color;
use crate::core::scene::{SceneBuilder, SceneGraph};
use crate::renderer::draw_plan::DrawPlan;
use crate::renderer::draw_plan_builder::DrawPlanBuilder;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::wgpu_renderer::WgpuRenderer;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 90;

#[test]
fn ui_typography_metrics_match_bound_shaping() {
    use crate::core::paragraph::TextStyle;
    use crate::core::text_engine::TextEngine;
    use crate::platform::font_manager::FontWeight;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("headless adapter required for typography metrics");
    let assets = super::retained_value_updates::ui_typography_font_assets();
    renderer.admit_font_assets(&assets).unwrap();
    let catalog = renderer.font_face_catalog();
    let default_id = renderer
        .text_engine_mut()
        .shape("default", 16.0)
        .binding
        .unwrap()
        .face()
        .font_id();
    let mut ui = FontManagerTextEngine::with_face_catalog(catalog, default_id).unwrap();
    for scale in [1.0, 1.25, 2.0] {
        renderer.text_engine_mut().set_scale(scale);
        ui.set_scale(scale);
        for weight in [FontWeight::Regular, FontWeight::Bold] {
            let style = TextStyle::default()
                .with_family(assets[0].family_name.clone())
                .with_size(24.0)
                .with_weight(weight)
                .with_cluster_gap(2.25)
                .unwrap();
            for text in ["AV office", "Привет e\u{301}", "ffi  AV"] {
                let a = ui.shape_styled(text, Some(&style), 13.0).unwrap();
                let b = renderer
                    .text_engine_mut()
                    .shape_styled(text, Some(&style), 13.0)
                    .unwrap();
                assert_eq!(a.binding, b.binding);
                assert_eq!(a.clusters.len(), b.clusters.len());
                assert!((a.width - b.width).abs() < 0.002);
                for (a, b) in a.glyphs.iter().zip(&b.glyphs) {
                    assert_eq!(a.id, b.id);
                    assert!((a.x - b.x).abs() < 0.002);
                }
                let (compiled, keys) = renderer
                    .text_engine_mut()
                    .shape_bound_with_keys(text, b.font_size, b.binding.as_ref().unwrap())
                    .unwrap();
                assert_eq!(compiled.width, b.width);
                assert!(
                    keys.iter()
                        .all(|key| key.font_id == b.binding.as_ref().unwrap().face().font_id())
                );
                for key in keys {
                    assert!(
                        renderer
                            .text_engine_mut()
                            .rasterize_bound_glyph(&key, scale)
                            .is_some(),
                        "matching backend bytes must realize each supported glyph"
                    );
                }
            }
        }
    }
}

fn text_scene() -> SceneGraph {
    let mut builder = SceneBuilder::new();
    builder.push_rect(
        RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        Color::BLACK.into(),
        0,
        None,
    );
    builder.push_text_probe(
        Point::new(8.0, 20.0),
        "unchanged renderer text".to_string(),
        11.0,
        Color::WHITE.into(),
        1,
        None,
    );
    builder.build()
}

fn draw_plan(renderer: &mut WgpuRenderer) -> DrawPlan<MultiSurfaceContract> {
    let viewport = RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    let mut compositor = Compositor::new(viewport);
    let surface_id: SurfaceId = compositor.create_surface(0, viewport, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface_id, (WIDTH, HEIGHT), 1.0)
        .expect("register headless text target");

    let mut scenes = HashMap::new();
    scenes.insert(surface_id, text_scene());
    DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission")
}

#[test]
fn repeated_renderer_compile_reuses_native_font_metrics_across_frames() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let plan = draw_plan(&mut renderer);

    assert_eq!(
        renderer
            .text_engine_mut()
            .backend()
            .font_metrics_call_count_for_test(),
        0
    );
    assert!(
        renderer
            .compile_multi(&plan)
            .expect("first compile")
            .is_ready()
    );
    assert_eq!(
        renderer
            .text_engine_mut()
            .backend()
            .font_metrics_call_count_for_test(),
        1,
        "one exact metrics entry must serve shaping and renderer quad construction"
    );

    assert!(
        renderer
            .compile_multi(&plan)
            .expect("second compile")
            .is_ready()
    );
    assert_eq!(
        renderer
            .text_engine_mut()
            .backend()
            .font_metrics_call_count_for_test(),
        1,
        "an unchanged renderer frame must not reconstruct a native font"
    );
}
