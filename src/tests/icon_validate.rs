use crate::core::icon::{
    IconId, IconRenderMode, IconRole, IconViolation, LayerRole, validate_icon,
};
use crate::core::icon_builder::IconBuilder;
use crate::core::icons::builtin_registry;

#[test]
fn test_all_builtins_comply_with_visual_contract() {
    let reg = builtin_registry();
    for i in 1..=17 {
        let id = IconId(i);
        if let Some(def) = reg.get(id) {
            let result = validate_icon(def);
            assert!(
                result.is_ok(),
                "builtin {:?} (id={}) failed validation: {:?}",
                def.name,
                id.0,
                result.unwrap_err()
            );
        }
    }
}

#[test]
fn test_rejects_out_of_bounds_coordinate() {
    let def = IconBuilder::new(24.0)
        .polygon(&[(25.0, 12.0), (12.0, 12.0), (12.0, 0.0)])
        .build(
            IconId(999),
            "bad-bounds",
            IconRole::Action,
            IconRenderMode::Fill,
        );
    let err = validate_icon(&def).unwrap_err();
    assert!(
        err.iter()
            .any(|v| matches!(v, IconViolation::CoordinateOutOfBounds { .. }))
    );
}

#[test]
fn test_rejects_fractional_coordinate() {
    let def = IconBuilder::new(24.0)
        .polygon(&[(3.7, 12.0), (12.0, 12.0), (12.0, 0.0)])
        .build(
            IconId(999),
            "bad-precision",
            IconRole::Action,
            IconRenderMode::Fill,
        );
    let err = validate_icon(&def).unwrap_err();
    assert!(
        err.iter()
            .any(|v| matches!(v, IconViolation::CoordinatePrecision { .. }))
    );
}

#[test]
fn test_rejects_too_many_layers() {
    let def = IconBuilder::new(24.0)
        .layer(LayerRole::Base)
        .polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])
        .layer(LayerRole::Shadow)
        .polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])
        .layer(LayerRole::Highlight)
        .polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])
        .layer(LayerRole::Base)
        .polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])
        .layer(LayerRole::Shadow)
        .polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])
        .layer(LayerRole::Highlight)
        .polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])
        .build(
            IconId(999),
            "too-many-layers",
            IconRole::Action,
            IconRenderMode::Fill,
        );
    let err = validate_icon(&def).unwrap_err();
    assert!(
        err.iter()
            .any(|v| matches!(v, IconViolation::TooManyLayers { count: 6 }))
    );
}

#[test]
fn test_rejects_too_many_segments() {
    let points: Vec<(f32, f32)> = (0..26)
        .map(|i| ((i % 23) as f32 + 0.5, ((i * 2) % 23) as f32 + 0.5))
        .collect();
    let def = IconBuilder::new(24.0).polygon(&points).build(
        IconId(999),
        "too-many-segments",
        IconRole::Action,
        IconRenderMode::Fill,
    );
    let err = validate_icon(&def).unwrap_err();
    assert!(
        err.iter()
            .any(|v| matches!(v, IconViolation::TooManySegments { .. }))
    );
}

#[test]
fn test_accepts_half_grid_coordinates() {
    let def = IconBuilder::new(24.0)
        .polygon(&[(11.5, 6.5), (12.0, 6.5), (12.0, 7.0)])
        .build(
            IconId(999),
            "half-grid",
            IconRole::Action,
            IconRenderMode::Fill,
        );
    assert!(validate_icon(&def).is_ok());
}

#[test]
fn test_skip_precision_on_computed_geometry() {
    // line() produces computed geometry — skip_precision=true, should pass
    let def_skip = IconBuilder::new(24.0)
        .line(6.0, 6.0, 18.0, 18.0, 1.5)
        .build(
            IconId(999),
            "line-skip",
            IconRole::Action,
            IconRenderMode::Stroke,
        );
    assert!(
        def_skip.layers[0].skip_precision,
        "line() layer must have skip_precision=true"
    );
    assert!(
        validate_icon(&def_skip).is_ok(),
        "line() layer with skip_precision should pass validation"
    );

    // Same geometry manually constructed with skip_precision=false should fail precision
    use crate::core::icon::{FillRule, IconDefinition, IconLayer};
    let mut failing = def_skip;
    failing.layers[0].skip_precision = false;
    let err = validate_icon(&failing).unwrap_err();
    assert!(
        err.iter()
            .any(|v| matches!(v, IconViolation::CoordinatePrecision { .. })),
        "same geometry without skip_precision must fail precision check"
    );
}
