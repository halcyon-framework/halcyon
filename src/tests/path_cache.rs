use crate::renderer::mesh_cache::PixelSnapMode;
use crate::renderer::path::{FillRule, Path, PathFillError, PathMesh, tessellate_and_snap};

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

type Point = [f64; 2];

fn orient(a: Point, b: Point, p: Point) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

fn safely_away_from_edge(a: Point, b: Point, p: Point, margin: f64) -> bool {
    let direction = [b[0] - a[0], b[1] - a[1]];
    let length_squared = direction[0].powi(2) + direction[1].powi(2);
    if length_squared == 0.0 {
        return true;
    }
    let t = (((p[0] - a[0]) * direction[0] + (p[1] - a[1]) * direction[1]) / length_squared)
        .clamp(0.0, 1.0);
    (p[0] - (a[0] + t * direction[0])).hypot(p[1] - (a[1] + t * direction[1])) > margin
}

/// Independent ray crossings over the authored contours, including implicit closing edges.
fn winding_at(contours: &[Vec<Point>], point: Point, margin: f64) -> Option<i32> {
    let mut winding = 0;
    for contour in contours {
        for (&start, &end) in contour.iter().zip(contour.iter().cycle().skip(1)) {
            if !safely_away_from_edge(start, end, point, margin) {
                return None;
            }
            if start[1] <= point[1] && point[1] < end[1] && orient(start, end, point) > 0.0 {
                winding += 1;
            } else if end[1] <= point[1] && point[1] < start[1] && orient(start, end, point) < 0.0 {
                winding -= 1;
            }
        }
    }
    Some(winding)
}

fn separated_by_edge(a: [Point; 3], b: [Point; 3]) -> bool {
    (0..3).any(|i| {
        b.iter()
            .all(|&point| orient(a[i], a[(i + 1) % 3], point) <= 1e-8)
    })
}

fn check_coverage(
    label: &str,
    mesh: &PathMesh,
    contours: &[Vec<Point>],
    rule: FillRule,
    margin: f64,
) {
    assert_eq!(mesh.indices.len() % 3, 0, "{label}");
    assert!(
        mesh.vertices.iter().flatten().all(|v| v.is_finite()),
        "{label}"
    );
    let triangles: Vec<[Point; 3]> = mesh
        .indices
        .chunks_exact(3)
        .map(|indices| {
            [indices[0], indices[1], indices[2]].map(|i| mesh.vertices[i as usize].map(f64::from))
        })
        .collect();
    for (index, &[a, b, c]) in triangles.iter().enumerate() {
        assert!(
            orient(a, b, c) > 0.0,
            "{label} non-CCW or degenerate triangle"
        );
        for &other in &triangles[..index] {
            assert!(
                separated_by_edge([a, b, c], other) || separated_by_edge(other, [a, b, c]),
                "{label} triangles overlap: {:?} {other:?}",
                [a, b, c],
            );
        }
    }
    let mut expected_filled = 0;
    let mut expected_clear = 0;
    for y in -1..24 {
        for x in -1..29 {
            let point = [f64::from(x) + 0.31, f64::from(y) + 0.57];
            let Some(winding) = winding_at(contours, point, margin) else {
                continue;
            };
            let expected = match rule {
                FillRule::NonZero => winding != 0,
                FillRule::EvenOdd => winding % 2 != 0,
            };
            let actual = triangles.iter().any(|&[a, b, c]| {
                [
                    orient(a, b, point),
                    orient(b, c, point),
                    orient(c, a, point),
                ]
                .into_iter()
                .all(|side| side >= -1e-8)
            });
            assert_eq!(actual, expected, "{label} {rule:?} at {point:?}");
            expected_filled += usize::from(expected);
            expected_clear += usize::from(!expected);
        }
    }
    assert!(expected_clear > 0);
    assert_eq!(triangles.is_empty(), expected_filled == 0, "{label}");
    if triangles.is_empty() {
        assert!(mesh.vertices.is_empty());
        assert_eq!(mesh.bounds, crate::core::math::RectF::zero());
    } else {
        for &[x, y] in &mesh.vertices {
            assert!(x >= mesh.bounds.x && x <= mesh.bounds.x + mesh.bounds.width + 1e-5);
            assert!(y >= mesh.bounds.y && y <= mesh.bounds.y + mesh.bounds.height + 1e-5);
        }
    }
}

fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Vec<Point> {
    vec![[left, top], [right, top], [right, bottom], [left, bottom]]
}

fn polygon_path(contours: &[Vec<Point>]) -> Path {
    let mut path = Path::new();
    for (index, contour) in contours.iter().enumerate() {
        let [x, y] = contour[0];
        path.move_to(x as f32, y as f32);
        for &[x, y] in &contour[1..] {
            path.line_to(x as f32, y as f32);
        }
        if index % 2 == 0 {
            path.close();
        }
    }
    path
}

#[test]
fn compound_fill_matches_independent_winding_and_partitions_coverage() {
    let outer = rect(1.0, 1.0, 21.0, 21.0);
    let inner = rect(5.0, 5.0, 17.0, 17.0);
    let hole: Vec<_> = inner.iter().copied().rev().collect();
    let reversed_outer: Vec<_> = outer.iter().copied().rev().collect();
    let cases = [
        (
            "disconnected",
            vec![rect(1.0, 1.0, 8.0, 8.0), rect(12.0, 3.0, 21.0, 11.0)],
        ),
        (
            "overlap",
            vec![rect(1.0, 1.0, 9.0, 9.0), rect(5.0, 5.0, 13.0, 13.0)],
        ),
        ("same-winding nesting", vec![outer.clone(), inner]),
        (
            "nested island",
            vec![outer.clone(), hole, rect(8.0, 8.0, 14.0, 14.0)],
        ),
        (
            "coincident same winding",
            vec![outer.clone(), outer.clone()],
        ),
        ("coincident opposite winding", vec![outer, reversed_outer]),
        (
            "bow-tie",
            vec![vec![[1.0, 1.0], [12.0, 12.0], [1.0, 12.0], [12.0, 1.0]]],
        ),
        (
            "touching and shared collinear edges",
            vec![
                rect(1.0, 1.0, 9.0, 9.0),
                rect(9.0, 3.0, 16.0, 14.0),
                rect(16.0, 14.0, 20.0, 18.0),
            ],
        ),
        (
            "collinear overlapping edges",
            vec![rect(1.0, 1.0, 9.0, 9.0), rect(5.0, 1.0, 13.0, 9.0)],
        ),
        (
            "zero area and repeated points",
            vec![vec![[1.0, 3.0], [1.0, 3.0], [5.0, 3.0], [11.0, 3.0]]],
        ),
    ];

    for (label, mut contours) in cases {
        for _ in 0..2 {
            let path = polygon_path(&contours);
            for rule in [FillRule::NonZero, FillRule::EvenOdd] {
                let mesh = path.try_to_mesh_with_fill_rule(rule).expect(label);
                check_coverage(label, &mesh, &contours, rule, 0.04);
            }
            for contour in &mut contours {
                contour.reverse();
            }
        }
    }
}

#[test]
fn compound_fill_subdivides_quadratic_and_cubic_contours() {
    let mut path = Path::new();
    path.move_to(2.0, 13.0);
    path.quad_to(8.0, -7.0, 14.0, 13.0);
    path.move_to(12.0, 12.0);
    path.cubic_to(12.0, -2.0, 27.0, -2.0, 27.0, 12.0);
    path.close();

    let parameters = (0..=128).map(|step| f64::from(step) / 128.0);
    let quad = parameters
        .clone()
        .map(|t| [2.0 + 12.0 * t, 13.0 - 40.0 * t * (1.0 - t)]);
    let cubic = parameters.map(|t| {
        let u = 1.0 - t;
        [
            12.0 + 15.0 * (3.0 * u * t.powi(2) + t.powi(3)),
            12.0 - 42.0 * t * u,
        ]
    });
    let contours = [quad.collect(), cubic.collect()];
    for rule in [FillRule::NonZero, FillRule::EvenOdd] {
        let mesh = path.try_to_mesh_with_fill_rule(rule).unwrap();
        check_coverage("curves", &mesh, &contours, rule, 0.4);
    }
}

#[test]
fn compound_fill_snaps_source_edges_before_computing_intersections() {
    let source = vec![vec![[0.1, 0.1], [5.2, 3.2], [0.1, 3.2], [5.2, 0.15]]];
    let projected = vec![vec![[0.0, 0.0], [5.0, 3.0], [0.0, 3.0], [5.0, 0.0]]];
    let mesh = tessellate_and_snap(&polygon_path(&source), PixelSnapMode::Center, 1.0);
    assert!(mesh.vertices.contains(&[2.5, 1.5]));
    check_coverage(
        "projected bow-tie",
        &mesh,
        &projected,
        FillRule::NonZero,
        0.04,
    );
}

#[test]
fn compound_fill_distinguishes_empty_coverage_from_invalid_source() {
    let empty = Path::new()
        .try_to_mesh_with_fill_rule(FillRule::NonZero)
        .unwrap();
    assert!(empty.indices.is_empty() && empty.vertices.is_empty());

    use crate::renderer::path::PathCommand::*;
    for commands in [
        vec![LineTo(1.0, 1.0)],
        vec![Close],
        vec![MoveTo(0.0, f32::NAN)],
        vec![MoveTo(0.0, 0.0), LineTo(f32::INFINITY, 0.0)],
        vec![MoveTo(0.0, 0.0), QuadTo(f32::INFINITY, 1.0, 2.0, 3.0)],
        vec![
            MoveTo(0.0, 0.0),
            CubicTo(0.0, f32::MAX, 0.0, 1.0, f32::MAX, 0.0),
        ],
    ] {
        let path = Path { commands };
        assert_eq!(
            path.try_to_mesh_with_fill_rule(FillRule::EvenOdd)
                .unwrap_err(),
            PathFillError::InvalidSource,
        );
        assert!(path.to_mesh().indices.is_empty());
    }
}
