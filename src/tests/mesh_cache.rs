use crate::core::math::RectF;
use crate::renderer::mesh_cache::*;
use crate::renderer::path::{FillRule, PathMesh};

fn dummy_mesh() -> PathMesh {
    PathMesh {
        vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        indices: vec![0, 1, 2],
        bounds: RectF::new(0.0, 0.0, 1.0, 1.0),
    }
}

fn make_key(path_hash: u64, dpr_q: u32) -> MeshCacheKey {
    MeshCacheKey {
        path_hash,
        render_width_q: MeshCacheKey::quantize_size(16.0),
        render_height_q: MeshCacheKey::quantize_size(16.0),
        stroke_width_q: 0,
        is_fill: true,
        fill_rule: FillRule::NonZero,
        dpr_q,
        dpr_bits: (dpr_q as f32 / 8.0).to_bits(),
        snap_mode: PixelSnapMode::Center,
    }
}

#[test]
fn test_cache_hit() {
    let mut cache = MeshCache::new();
    let key = make_key(42, 8);
    let mesh = dummy_mesh();
    cache.insert(key.clone(), mesh.clone(), 0);

    let hit = cache.get(&key, 1);
    assert!(hit.is_some());
    let hit = hit.unwrap();
    assert_eq!(hit.vertices.len(), 3);
    assert_eq!(hit.indices, vec![0, 1, 2]);
}

#[test]
fn test_cache_miss() {
    let mut cache = MeshCache::new();
    let key = make_key(99, 8);
    assert!(cache.get(&key, 0).is_none());
}

#[test]
fn test_dpr_isolation() {
    let mut cache = MeshCache::new();
    let key_1x = make_key(42, MeshCacheKey::quantize_dpr(1.0));
    let key_2x = make_key(42, MeshCacheKey::quantize_dpr(2.0));

    let mesh1 = dummy_mesh();
    let mut mesh2 = dummy_mesh();
    mesh2.vertices = vec![[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]];

    cache.insert(key_1x.clone(), mesh1, 0);
    cache.insert(key_2x.clone(), mesh2, 0);

    assert_eq!(cache.len(), 2);
    assert!(cache.get(&key_1x, 1).is_some());
    assert!(cache.get(&key_2x, 1).is_some());
    assert_eq!(cache.get(&key_1x, 1).unwrap().vertices[1], [1.0, 0.0]);
    assert_eq!(cache.get(&key_2x, 1).unwrap().vertices[1], [2.0, 0.0]);
}

#[test]
fn test_eviction() {
    let mut cache = MeshCache::new();
    let key = make_key(42, 8);
    cache.insert(key.clone(), dummy_mesh(), 0);

    cache.evict_stale(200, 120, 16);
    assert!(cache.is_empty());
}

#[test]
fn test_eviction_bounded() {
    let mut cache = MeshCache::new();
    for i in 0..5u64 {
        cache.insert(make_key(i, 8), dummy_mesh(), 0);
    }
    assert_eq!(cache.len(), 5);

    cache.evict_stale(200, 120, 2);
    assert_eq!(
        cache.len(),
        3,
        "only 2 of 5 stale entries should be evicted"
    );
}

#[test]
fn test_clear() {
    let mut cache = MeshCache::new();
    for i in 0..10u64 {
        cache.insert(make_key(i, 8), dummy_mesh(), 0);
    }
    assert_eq!(cache.len(), 10);

    cache.clear();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn test_color_independence() {
    // INV-VMP-004: MeshCacheKey has no color field.
    // Structural assertion: two keys differing only in path_hash (not color)
    // are different entries. Color is not part of the key at all.
    let key = make_key(42, 8);
    assert_eq!(key.path_hash, 42);
    // No color field exists on MeshCacheKey — this is a compile-time structural guarantee.
    // We verify the key contains exactly the fields defined in the SPEC.
    let _key2 = MeshCacheKey {
        path_hash: key.path_hash,
        render_width_q: key.render_width_q,
        render_height_q: key.render_height_q,
        stroke_width_q: key.stroke_width_q,
        is_fill: key.is_fill,
        fill_rule: key.fill_rule,
        dpr_q: key.dpr_q,
        dpr_bits: key.dpr_bits,
        snap_mode: key.snap_mode,
    };
}

#[test]
fn test_quantize_stability() {
    // 12.0 and 12.2 are within 0.5px bucket: (12.0*2).round() = 24, (12.2*2).round() = 24
    assert_eq!(
        MeshCacheKey::quantize_size(12.0),
        MeshCacheKey::quantize_size(12.2)
    );
    // 12.0 and 12.8 are different buckets: (12.8*2).round() = 26
    assert_ne!(
        MeshCacheKey::quantize_size(12.0),
        MeshCacheKey::quantize_size(12.8)
    );
}

#[test]
fn test_last_used_frame_updated() {
    let mut cache = MeshCache::new();
    let key = make_key(42, 8);
    cache.insert(key.clone(), dummy_mesh(), 0);

    // Access at frame 50 — updates last_used_frame
    assert!(cache.get(&key, 50).is_some());

    // Evict at frame 160 with max_age=120: 160 - 50 = 110 < 120 → survives
    cache.evict_stale(160, 120, 16);
    assert_eq!(
        cache.len(),
        1,
        "entry accessed at frame 50 should survive eviction at frame 160 with max_age=120"
    );
}
