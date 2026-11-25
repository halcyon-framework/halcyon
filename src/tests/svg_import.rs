use crate::core::icon::{IconId, IconRenderMode, IconRole, LayerRole};
use crate::core::svg_import::{icon_from_svg, icon_from_svg_layers};

#[test]
fn test_single_layer_import() {
    let def = icon_from_svg(
        IconId(100),
        "test-single",
        "M 0 0 L 10 0 L 10 10 L 0 10 Z",
        24.0,
        IconRole::Action,
        IconRenderMode::Fill,
    )
    .unwrap();
    assert_eq!(def.id, IconId(100));
    assert_eq!(def.name, "test-single");
    assert_eq!(def.viewbox_width, 24.0);
    assert_eq!(def.viewbox_height, 24.0);
    assert_eq!(def.layers.len(), 1);
    assert_eq!(def.layers[0].role, LayerRole::Base);
    assert_eq!(
        def.layers[0].fill_rule,
        crate::core::icon::FillRule::NonZero
    );
    assert!(def.path_hash != 0, "path_hash should be non-zero");
    // Verify hash matches what content_hash produces
    assert_eq!(def.path_hash, def.layers[0].path.content_hash());
}

#[test]
fn test_multi_layer_import() {
    let def = icon_from_svg_layers(
        IconId(101),
        "test-multi",
        &[
            (LayerRole::Shadow, "M 0 0 L 20 0 L 20 5 L 0 5 Z"),
            (LayerRole::Base, "M 0 4 L 20 4 L 20 20 L 0 20 Z"),
        ],
        24.0,
        IconRole::Structural,
        IconRenderMode::Fill,
    )
    .unwrap();
    assert_eq!(def.layers.len(), 2);
    assert_eq!(def.layers[0].role, LayerRole::Shadow);
    assert_eq!(def.layers[1].role, LayerRole::Base);
    assert!(def.path_hash != 0);
}

#[test]
fn test_import_error_propagation() {
    let result = icon_from_svg(
        IconId(102),
        "bad",
        "M 0 0 A 5 5 0 0 1 10 0",
        24.0,
        IconRole::Action,
        IconRenderMode::Fill,
    );
    assert!(result.is_err(), "Arc command should propagate SvgPathError");
}

#[test]
fn test_import_real_lucide() {
    // Lucide "circle" icon as SVG path (circle cx=12 cy=12 r=10)
    let d = "M 22 12 C 22 17.5228 17.5228 22 12 22 C 6.47715 22 2 17.5228 2 12 C 2 6.47715 6.47715 2 12 2 C 17.5228 2 22 6.47715 22 12 Z";
    let def = icon_from_svg(
        IconId(103),
        "circle",
        d,
        24.0,
        IconRole::Status,
        IconRenderMode::Stroke,
    )
    .unwrap();
    // 1 MoveTo + 4 CubicTo + 1 Close = 6 commands
    assert_eq!(def.layers[0].path.commands.len(), 6);
    // All coordinates within icon bounds (0..24)
    for cmd in &def.layers[0].path.commands {
        match cmd {
            crate::renderer::path::PathCommand::MoveTo(x, y)
            | crate::renderer::path::PathCommand::LineTo(x, y) => {
                assert!(*x >= 0.0 && *x <= 24.0, "x out of bounds: {x}");
                assert!(*y >= 0.0 && *y <= 24.0, "y out of bounds: {y}");
            }
            crate::renderer::path::PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                for &v in &[*c1x, *c1y, *c2x, *c2y, *x, *y] {
                    assert!(v >= 0.0 && v <= 24.0, "coordinate out of bounds: {v}");
                }
            }
            _ => {}
        }
    }
}
