use std::collections::HashMap;

use crate::compositor::{Compositor, MultiSurfaceContract, SceneKey, SurfaceId};
use crate::core::math::RectF;
use crate::core::paint::Color;
use crate::core::scene::{SceneBuilder, SceneGraph};
use crate::renderer::draw_plan::DrawPlan;
use crate::renderer::draw_plan_builder::DrawPlanBuilder;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::path::Path;
use crate::renderer::wgpu_renderer::WgpuRenderer;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 90;

fn path_scene() -> SceneGraph {
    let mut path = Path::new();
    path.move_to(8.0, 8.0);
    path.line_to(152.0, 8.0);
    path.line_to(80.0, 82.0);
    path.close();

    let mut builder = SceneBuilder::new();
    builder.push_path(path, Color::WHITE.into(), 0, None);
    builder.build()
}

fn draw_plan(renderer: &mut WgpuRenderer) -> DrawPlan<MultiSurfaceContract> {
    let viewport = RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    let mut compositor = Compositor::new(viewport);
    let surface_id: SurfaceId = compositor.create_surface(0, viewport, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface_id, (WIDTH, HEIGHT), 1.0)
        .expect("register headless path target");

    let mut scenes = HashMap::new();
    scenes.insert(surface_id, path_scene());
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
fn repeated_path_compile_reuses_renderer_staging_buffers() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let plan = draw_plan(&mut renderer);

    assert!(
        renderer
            .compile_multi(&plan)
            .expect("first path compile")
            .is_ready()
    );
    renderer.wait_for_staging_recycles_for_test();
    let warm_bytes = renderer.staging_pool_total_bytes_for_test();
    assert!(warm_bytes > 0, "path compile must use renderer staging");

    for _ in 0..16 {
        assert!(
            renderer
                .compile_multi(&plan)
                .expect("repeated path compile")
                .is_ready()
        );
        renderer.wait_for_staging_recycles_for_test();
        assert_eq!(
            renderer.staging_pool_total_bytes_for_test(),
            warm_bytes,
            "an unchanged path frame must not allocate more staging capacity"
        );
    }
}

#[test]
fn repeated_path_compile_with_frame_advance_bounds_in_flight_staging() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let plan = draw_plan(&mut renderer);

    for _ in 0..128 {
        assert!(
            renderer
                .compile_multi(&plan)
                .expect("repeated path compile")
                .is_ready()
        );
        renderer.advance_frame();
    }

    renderer.wait_for_staging_recycles_for_test();
    let warm_bytes = renderer.staging_pool_total_bytes_for_test();

    for _ in 0..128 {
        assert!(
            renderer
                .compile_multi(&plan)
                .expect("repeated path compile")
                .is_ready()
        );
        renderer.advance_frame();
    }

    renderer.wait_for_staging_recycles_for_test();
    assert_eq!(renderer.pending_staging_work_for_test(), (0, 0));
    assert_eq!(
        renderer.staging_pool_total_bytes_for_test(),
        warm_bytes,
        "the production frame-advance lifecycle must stabilize unchanged staging capacity"
    );
}
