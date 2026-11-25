//! INV-IMG-005 contract test: compile-phase texture validation.
//!
//! Verifies that compile_multi rejects image batches referencing unregistered
//! texture_ids:
//! - Debug: panics with message containing "INV-IMG-005"
//! - Release: succeeds (batch silently skipped)

use halcyon::compositor::{Compositor, SceneKey};
use halcyon::core::math::RectF;
use halcyon::core::scene::{Color, SceneBuilder};
use halcyon::renderer::DrawPlanBuilder;
use halcyon::renderer::mesh_cache::MeshCache;
use halcyon::renderer::wgpu_renderer::WgpuRenderer;
use std::collections::HashMap;

const W: u32 = 100;
const H: u32 = 100;
const UNREGISTERED_TEX_ID: u64 = 42;

/// Build a minimal scene containing one image referencing UNREGISTERED_TEX_ID.
fn build_scene_with_unregistered_image() -> halcyon::core::scene::SceneGraph {
    let mut builder = SceneBuilder::new();
    builder.push_image(
        RectF::new(0.0, 0.0, 50.0, 50.0),
        UNREGISTERED_TEX_ID,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        0,
        None,
    );
    builder.build()
}

/// Create a headless renderer + compositor + draw plan for the unregistered image scene.
/// Returns None if no GPU adapter is available (CI skip).
fn setup() -> Option<(
    WgpuRenderer,
    halcyon::renderer::draw_plan::DrawPlan<halcyon::compositor::MultiSurfaceContract>,
)> {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())?;

    let viewport = RectF::new(0.0, 0.0, W as f32, H as f32);
    let mut compositor = Compositor::new(viewport);
    let surface_id = compositor.create_surface(0, viewport, SceneKey::BASE);

    renderer
        .register_headless_render_target(surface_id, (W, H), 1.0)
        .expect("register_headless_render_target failed");

    let scene = build_scene_with_unregistered_image();

    let surface_stack = compositor.snapshot_multi();
    let mut surface_scenes = HashMap::new();
    surface_scenes.insert(surface_id, scene);

    let mut draw_plan_builder = DrawPlanBuilder::new();
    let draw_plan = draw_plan_builder
        .build_multi(
            &surface_scenes,
            surface_stack,
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");

    Some((renderer, draw_plan))
}

// =============================================================================
// Debug build: compile_multi must panic with INV-IMG-005
// =============================================================================
#[cfg(debug_assertions)]
#[test]
fn inv_img_005_debug_panics_on_unregistered_texture() {
    let Some((mut renderer, draw_plan)) = setup() else {
        eprintln!("SKIP: no GPU adapter available");
        return;
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = renderer.compile_multi(&draw_plan);
    }));

    let panic_payload =
        result.expect_err("compile_multi must panic in debug when texture_id is unregistered");

    let panic_msg = panic_payload
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
        .unwrap_or("");

    assert!(
        panic_msg.contains("INV-IMG-005"),
        "panic message must contain INV-IMG-005, got: {panic_msg}"
    );
}

// =============================================================================
// Release build: compile_multi must NOT panic (batch silently skipped)
// =============================================================================
#[cfg(not(debug_assertions))]
#[test]
fn inv_img_005_release_skips_without_panic() {
    let Some((mut renderer, draw_plan)) = setup() else {
        eprintln!("SKIP: no GPU adapter available");
        return;
    };

    // Must not panic — unregistered texture batch is silently skipped
    let result = renderer.compile_multi(&draw_plan);
    assert!(
        result.is_ok(),
        "compile_multi must not panic in release for unregistered texture_id"
    );
}
