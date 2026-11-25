use crate::compositor::{Compositor, SceneKey};
use crate::core::math::RectF;
use crate::core::scene::{Color, SceneBuilder};
use crate::renderer::DrawPlanBuilder;
use crate::renderer::mesh_cache::{MeshCache, MeshCacheKey, PixelSnapMode};
use crate::renderer::path::{FillRule, Path};
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

#[test]
fn compound_fill_cache_isolates_rule_and_exact_dpr_and_reuses_warm_geometry() {
    let viewport = RectF::new(0.0, 0.0, 64.0, 64.0);
    let mut compositor = Compositor::new(viewport);
    let surface = compositor.create_surface(0, viewport, SceneKey::BASE);
    let mut cache = MeshCache::new();
    let mut path = Path::rounded_rect(0.49, 0.49, 20.0, 20.0, 0.0);
    path.commands
        .extend(Path::rounded_rect(5.49, 5.49, 10.0, 10.0, 0.0).commands);
    let build = |cache: &mut MeshCache, rule, scale, color, frame| {
        let mut scene = SceneBuilder::new();
        scene.push_path_with_fill_rule(path.clone(), rule, color, 1, None);
        DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene.build())]),
                compositor.snapshot_multi(),
                cache,
                |_| scale,
                frame,
            )
            .unwrap()
    };
    let scale_a = 1.0_f32;
    let scale_b = 1.03_f32;
    assert_eq!(
        MeshCacheKey::quantize_dpr(scale_a),
        MeshCacheKey::quantize_dpr(scale_b)
    );
    let first = build(&mut cache, FillRule::NonZero, scale_a, Color::WHITE, 1);
    let even_odd = build(&mut cache, FillRule::EvenOdd, scale_a, Color::WHITE, 2);
    let other_scale = build(&mut cache, FillRule::NonZero, scale_b, Color::WHITE, 3);
    assert_eq!(cache.len(), 3);

    let first_draw = &first.surface_data[&surface].path_draws[0];
    let even_odd_draw = &even_odd.surface_data[&surface].path_draws[0];
    let other_scale_draw = &other_scale.surface_data[&surface].path_draws[0];
    let area = |mesh: &crate::renderer::path::PathMesh| {
        mesh.indices
            .chunks_exact(3)
            .map(|triangle| {
                let [a, b, c] =
                    [triangle[0], triangle[1], triangle[2]].map(|i| mesh.vertices[i as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * 0.5
            })
            .sum::<f32>()
    };
    assert_eq!(area(&first_draw.mesh), 400.0);
    assert_eq!(area(&even_odd_draw.mesh), 300.0);
    assert_ne!(first_draw.mesh.bounds, other_scale_draw.mesh.bounds);

    let key = MeshCacheKey {
        path_hash: path.content_hash(),
        render_width_q: MeshCacheKey::quantize_size(20.0),
        render_height_q: MeshCacheKey::quantize_size(20.0),
        stroke_width_q: 0,
        is_fill: true,
        fill_rule: FillRule::NonZero,
        dpr_q: MeshCacheKey::quantize_dpr(scale_a),
        dpr_bits: scale_a.to_bits(),
        snap_mode: PixelSnapMode::Center,
    };
    let cached_vertices = cache.get(&key, 3).unwrap().vertices.as_ptr();
    let warm = build(&mut cache, FillRule::NonZero, scale_a, Color::BLACK, 4);
    let warm_draw = &warm.surface_data[&surface].path_draws[0];
    assert_eq!(cache.len(), 3);
    assert_eq!(
        cache.get(&key, 4).unwrap().vertices.as_ptr(),
        cached_vertices
    );
    assert_eq!(warm_draw.mesh.vertices, first_draw.mesh.vertices);
    assert_eq!(warm_draw.mesh.indices, first_draw.mesh.indices);
    assert_ne!(warm_draw.color, first_draw.color);
}

#[test]
fn compound_fill_batcher_caches_empty_success_and_skips_failed_source() {
    let viewport = RectF::new(0.0, 0.0, 64.0, 64.0);
    let mut compositor = Compositor::new(viewport);
    let surface = compositor.create_surface(0, viewport, SceneKey::BASE);
    let mut builder = SceneBuilder::new();
    let mut invalid = Path::new();
    invalid.line_to(3.0, 3.0);
    builder.push_path(invalid, Color::WHITE, 1, None);
    let mut empty = Path::rounded_rect(1.0, 1.0, 8.0, 8.0, 0.0);
    empty.commands.extend(empty.commands.clone());
    builder.push_path_with_fill_rule(empty, FillRule::EvenOdd, Color::WHITE, 2, None);
    builder.push_path(
        Path::rounded_rect(8.0, 8.0, 18.0, 18.0, 0.0),
        Color::WHITE,
        3,
        None,
    );

    let mut cache = MeshCache::new();
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &HashMap::from([(surface, builder.build())]),
            compositor.snapshot_multi(),
            &mut cache,
            |_| 1.0,
            1,
        )
        .unwrap();
    assert_eq!(cache.len(), 2);
    let [visible] = plan.surface_data[&surface].path_draws.as_slice() else {
        panic!("only the admitted non-empty fill can emit a draw");
    };
    assert_eq!(visible.z_index, 3);
}
