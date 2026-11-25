use crate::core::svg_path::parse_svg_path;
use crate::renderer::path::PathCommand;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn test_parse_move_line_close() {
    let path = parse_svg_path("M 0 0 L 10 10 Z").unwrap();
    assert_eq!(path.commands.len(), 3);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 0.0 && *y == 0.0));
    assert!(matches!(&path.commands[1], PathCommand::LineTo(x, y) if *x == 10.0 && *y == 10.0));
    assert!(matches!(&path.commands[2], PathCommand::Close));
}

#[test]
fn test_parse_relative() {
    let abs_path = parse_svg_path("M 0 0 L 10 10 Z").unwrap();
    let rel_path = parse_svg_path("m 0 0 l 10 10 z").unwrap();
    assert_eq!(abs_path.commands.len(), rel_path.commands.len());
    for (a, r) in abs_path.commands.iter().zip(rel_path.commands.iter()) {
        match (a, r) {
            (PathCommand::MoveTo(ax, ay), PathCommand::MoveTo(rx, ry)) => {
                assert!(approx(*ax, *rx));
                assert!(approx(*ay, *ry));
            }
            (PathCommand::LineTo(ax, ay), PathCommand::LineTo(rx, ry)) => {
                assert!(approx(*ax, *rx));
                assert!(approx(*ay, *ry));
            }
            (PathCommand::Close, PathCommand::Close) => {}
            _ => panic!("command mismatch: {a:?} vs {r:?}"),
        }
    }
}

#[test]
fn test_parse_horizontal_vertical() {
    let path = parse_svg_path("M 0 0 H 10 V 10 H 0 Z").unwrap();
    assert_eq!(path.commands.len(), 5);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 0.0 && *y == 0.0));
    assert!(matches!(&path.commands[1], PathCommand::LineTo(x, y) if *x == 10.0 && *y == 0.0));
    assert!(matches!(&path.commands[2], PathCommand::LineTo(x, y) if *x == 10.0 && *y == 10.0));
    assert!(matches!(&path.commands[3], PathCommand::LineTo(x, y) if *x == 0.0 && *y == 10.0));
    assert!(matches!(&path.commands[4], PathCommand::Close));
}

#[test]
fn test_parse_cubic() {
    let path = parse_svg_path("M 0 0 C 1 2 3 4 5 6").unwrap();
    assert_eq!(path.commands.len(), 2);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 0.0 && *y == 0.0));
    assert!(
        matches!(&path.commands[1], PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) if *c1x == 1.0 && *c1y == 2.0 && *c2x == 3.0 && *c2y == 4.0 && *x == 5.0 && *y == 6.0)
    );
}

#[test]
fn test_parse_smooth_cubic() {
    let path = parse_svg_path("M 0 0 C 1 0 2 1 3 1 S 5 0 6 0").unwrap();
    assert_eq!(path.commands.len(), 3);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 0.0 && *y == 0.0));
    assert!(
        matches!(&path.commands[1], PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) if *c1x == 1.0 && *c1y == 0.0 && *c2x == 2.0 && *c2y == 1.0 && *x == 3.0 && *y == 1.0)
    );
    // S reflects cp2 of previous C: reflected = 2*(3,1) - (2,1) = (4,1)
    assert!(
        matches!(&path.commands[2], PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) if *c1x == 4.0 && *c1y == 1.0 && *c2x == 5.0 && *c2y == 0.0 && *x == 6.0 && *y == 0.0)
    );
}

#[test]
fn test_parse_quadratic_elevation() {
    let path = parse_svg_path("M 0 0 Q 5 10 10 0").unwrap();
    assert_eq!(path.commands.len(), 2);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 0.0 && *y == 0.0));
    // Quadratic control point (5, 10) elevated to cubic:
    // cp1 = current + 2/3 * (qp - current) = (0,0) + 2/3*(5,10) = (10/3, 20/3)
    // cp2 = end + 2/3 * (qp - end) = (10,0) + 2/3*(-5,10) = (20/3, 20/3)
    if let PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) = &path.commands[1] {
        assert!(approx(*c1x, 10.0 / 3.0), "c1x: {c1x}");
        assert!(approx(*c1y, 20.0 / 3.0), "c1y: {c1y}");
        assert!(approx(*c2x, 20.0 / 3.0), "c2x: {c2x}");
        assert!(approx(*c2y, 20.0 / 3.0), "c2y: {c2y}");
        assert!(approx(*x, 10.0), "x: {x}");
        assert!(approx(*y, 0.0), "y: {y}");
    } else {
        panic!("expected CubicTo, got {:?}", path.commands[1]);
    }
}

#[test]
fn test_parse_implicit_lineto() {
    let path = parse_svg_path("M 0 0 10 10 20 0").unwrap();
    assert_eq!(path.commands.len(), 3);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 0.0 && *y == 0.0));
    assert!(matches!(&path.commands[1], PathCommand::LineTo(x, y) if *x == 10.0 && *y == 10.0));
    assert!(matches!(&path.commands[2], PathCommand::LineTo(x, y) if *x == 20.0 && *y == 0.0));
}

#[test]
fn test_parse_compact_numbers() {
    let path = parse_svg_path("M10-5L.3.7L1.5.3Z").unwrap();
    assert_eq!(path.commands.len(), 4);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 10.0 && *y == -5.0));
    assert!(
        matches!(&path.commands[1], PathCommand::LineTo(x, y) if approx(*x, 0.3) && approx(*y, 0.7))
    );
    assert!(
        matches!(&path.commands[2], PathCommand::LineTo(x, y) if approx(*x, 1.5) && approx(*y, 0.3))
    );
    assert!(matches!(&path.commands[3], PathCommand::Close));
}

#[test]
fn test_parse_comma_separation() {
    let path = parse_svg_path("M0,0L10,10Z").unwrap();
    assert_eq!(path.commands.len(), 3);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(x, y) if *x == 0.0 && *y == 0.0));
    assert!(matches!(&path.commands[1], PathCommand::LineTo(x, y) if *x == 10.0 && *y == 10.0));
    assert!(matches!(&path.commands[2], PathCommand::Close));
}

#[test]
fn test_parse_empty_string() {
    let path = parse_svg_path("").unwrap();
    assert_eq!(path.commands.len(), 0);
}

#[test]
fn test_parse_real_icon() {
    // Lucide "star" icon path
    let d = "M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z";
    let path = parse_svg_path(d).unwrap();
    // 1 MoveTo + 10 LineTo + 1 Close = 12 commands
    assert_eq!(path.commands.len(), 12);
    assert!(matches!(&path.commands[0], PathCommand::MoveTo(..)));
    assert!(matches!(&path.commands[11], PathCommand::Close));
    // All coordinates should be within reasonable icon bounds (0..24)
    for cmd in &path.commands {
        match cmd {
            PathCommand::MoveTo(x, y) | PathCommand::LineTo(x, y) => {
                assert!(*x >= 0.0 && *x <= 24.0, "x out of bounds: {x}");
                assert!(*y >= 0.0 && *y <= 24.0, "y out of bounds: {y}");
            }
            _ => {}
        }
    }
}
