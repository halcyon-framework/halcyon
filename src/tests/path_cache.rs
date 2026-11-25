use crate::renderer::mesh_cache::PixelSnapMode;
use crate::renderer::path::{Path, tessellate_and_snap};

#[test]
fn test_content_hash_deterministic() {
    let mut p1 = Path::new();
    p1.move_to(0.0, 0.0);
    p1.line_to(10.0, 0.0);
    p1.line_to(10.0, 10.0);
    p1.close();

    let mut p2 = Path::new();
    p2.move_to(0.0, 0.0);
    p2.line_to(10.0, 0.0);
    p2.line_to(10.0, 10.0);
    p2.close();

    assert_eq!(p1.content_hash(), p2.content_hash());
}

#[test]
fn test_content_hash_different() {
    let mut p1 = Path::new();
    p1.move_to(0.0, 0.0);
    p1.line_to(10.0, 0.0);
    p1.close();

    let mut p2 = Path::new();
    p2.move_to(0.0, 0.0);
    p2.line_to(20.0, 0.0);
    p2.close();

    assert_ne!(p1.content_hash(), p2.content_hash());
}

#[test]
fn test_snap_center_mode() {
    // Create a triangle at non-pixel-aligned coordinates
    let mut path = Path::new();
    path.move_to(0.3, 0.7);
    path.line_to(10.3, 0.7);
    path.line_to(10.3, 10.7);
    path.close();

    let dpr = 2.0;
    let mesh = tessellate_and_snap(&path, PixelSnapMode::Center, dpr);

    // At DPR 2.0, physical coords: (0.6, 1.4), (20.6, 1.4), (20.6, 21.4)
    // round: (1.0, 1.0), (21.0, 1.0), (21.0, 21.0)
    // Back to logical: (0.5, 0.5), (10.5, 0.5), (10.5, 10.5)
    // All vertices should land on 0.5-logical-px grid (physical pixel centers)
    for v in &mesh.vertices {
        let phys_x = v[0] * dpr;
        let phys_y = v[1] * dpr;
        assert!(
            (phys_x - phys_x.round()).abs() < 0.001,
            "x={} not snapped to physical pixel center (phys={})",
            v[0],
            phys_x
        );
        assert!(
            (phys_y - phys_y.round()).abs() < 0.001,
            "y={} not snapped to physical pixel center (phys={})",
            v[1],
            phys_y
        );
    }
}

#[test]
fn test_snap_edge_mode() {
    let mut path = Path::new();
    path.move_to(0.3, 0.7);
    path.line_to(10.3, 0.7);
    path.line_to(10.3, 10.7);
    path.close();

    let dpr = 2.0;
    let mesh = tessellate_and_snap(&path, PixelSnapMode::Edge, dpr);

    // Edge snap: floor(phys) + 0.5
    // Physical: (0.6, 1.4) → (0.5, 1.5), back to logical: (0.25, 0.75)
    // All physical coords should be at half-pixel (x.5)
    for v in &mesh.vertices {
        let phys_x = v[0] * dpr;
        let phys_y = v[1] * dpr;
        let frac_x = phys_x - phys_x.floor();
        let frac_y = phys_y - phys_y.floor();
        assert!(
            (frac_x - 0.5).abs() < 0.001,
            "x={} not snapped to physical pixel edge (phys={}, frac={})",
            v[0],
            phys_x,
            frac_x
        );
        assert!(
            (frac_y - 0.5).abs() < 0.001,
            "y={} not snapped to physical pixel edge (phys={}, frac={})",
            v[1],
            phys_y,
            frac_y
        );
    }
}

#[test]
fn test_snap_none_mode() {
    let mut path = Path::new();
    path.move_to(0.3, 0.7);
    path.line_to(10.3, 0.7);
    path.line_to(10.3, 10.7);
    path.close();

    let original = path.to_mesh();
    let snapped = tessellate_and_snap(&path, PixelSnapMode::None, 2.0);

    assert_eq!(original.vertices.len(), snapped.vertices.len());
    for (orig, snap) in original.vertices.iter().zip(snapped.vertices.iter()) {
        assert_eq!(orig[0], snap[0]);
        assert_eq!(orig[1], snap[1]);
    }
}
