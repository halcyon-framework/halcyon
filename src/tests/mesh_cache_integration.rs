use crate::compositor::{Compositor, SceneKey};
use crate::core::math::RectF;
use crate::core::scene::{Color, SceneBuilder};
use crate::renderer::DrawPlanBuilder;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::path::Path;
use std::collections::HashMap;

/// SPEC-HAL-264 Milestone 2: Verify mesh cache hit on repeated flatten.
///
/// Creates a SceneNode::Path with no pre-computed mesh, builds a DrawPlan twice
/// with a shared MeshCache, and asserts cache len == 1 after the second build
/// (proving the cache hit occurred).
#[test]
fn test_mesh_cache_hit_on_repeated_build() {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(100.0, 0.0);
    path.line_to(100.0, 100.0);
    path.line_to(0.0, 100.0);
    path.close();

    let mut cache = MeshCache::new();

    // Set up compositor with one surface
    let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
    let surface_id =
        compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

    // First build: cache miss, tessellates and inserts
    {
        let mut builder = SceneBuilder::new();
        builder.push_path(path.clone(), Color::WHITE, 0, None);
        let scene = builder.build();

        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        let snapshot = compositor.snapshot_multi();

        let mut dpb = DrawPlanBuilder::new();
        let _ = dpb
            .build_multi(&surface_scenes, snapshot, &mut cache, |_| 2.0, 1)
            .expect("scene must satisfy DrawPlan Build admission");
    }
    assert_eq!(
        cache.len(),
        1,
        "cache should have 1 entry after first build"
    );

    // Second build: cache hit, no new entry
    {
        let mut builder = SceneBuilder::new();
        builder.push_path(path.clone(), Color::WHITE, 0, None);
        let scene = builder.build();

        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        let snapshot = compositor.snapshot_multi();

        let mut dpb = DrawPlanBuilder::new();
        let _ = dpb
            .build_multi(&surface_scenes, snapshot, &mut cache, |_| 2.0, 2)
            .expect("scene must satisfy DrawPlan Build admission");
    }
    assert_eq!(
        cache.len(),
        1,
        "cache should still have 1 entry after second build (cache hit)"
    );
}
