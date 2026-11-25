use crate::core::icon::*;
use crate::core::scene::{Color, SceneBuilder, SceneNode};
use crate::renderer::path::Path;

fn make_triangle_def(id: u32) -> IconDefinition {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(24.0, 0.0);
    path.line_to(24.0, 24.0);
    path.close();
    let path_hash = path.content_hash();
    IconDefinition {
        id: IconId(id),
        name: "test-triangle",
        layers: vec![IconLayer {
            role: LayerRole::Base,
            path,
            fill_rule: FillRule::NonZero,
            skip_precision: false,
        }],
        viewbox_width: 24.0,
        viewbox_height: 24.0,
        default_mode: IconRenderMode::Fill,
        role: IconRole::Action,
        path_hash,
    }
}

#[test]
fn test_push_icon_emits_path_node() {
    let def = make_triangle_def(1);
    let mut builder = SceneBuilder::new();
    builder.push_icon(&def, 0.0, 0.0, 16.0, Color::WHITE, 0);
    let scene = builder.build();
    assert_eq!(scene.root.len(), 1);
    assert!(matches!(&scene.root[0], SceneNode::Path(_)));
}

#[test]
fn test_push_icon_scaling() {
    let def = make_triangle_def(1);
    let mut builder = SceneBuilder::new();
    // size=12 on viewbox=24 → scale=0.5
    builder.push_icon(&def, 0.0, 0.0, 12.0, Color::WHITE, 0);
    let scene = builder.build();
    let path_node = match &scene.root[0] {
        SceneNode::Path(p) => p,
        _ => panic!("expected Path node"),
    };
    let bounds = path_node.path.bounds();
    // Original triangle is (0,0)-(24,24), scaled by 0.5 → (0,0)-(12,12)
    assert!(
        (bounds.width - 12.0).abs() < 0.1,
        "width should be ~12, got {}",
        bounds.width
    );
    assert!(
        (bounds.height - 12.0).abs() < 0.1,
        "height should be ~12, got {}",
        bounds.height
    );
}

#[test]
fn test_push_icon_translation() {
    let def = make_triangle_def(1);
    let mut builder = SceneBuilder::new();
    builder.push_icon(&def, 100.0, 50.0, 24.0, Color::WHITE, 0);
    let scene = builder.build();
    let path_node = match &scene.root[0] {
        SceneNode::Path(p) => p,
        _ => panic!("expected Path node"),
    };
    let bounds = path_node.path.bounds();
    // scale=1.0, translated to (100,50)
    assert!(
        (bounds.x - 100.0).abs() < 0.1,
        "x should be ~100, got {}",
        bounds.x
    );
    assert!(
        (bounds.y - 50.0).abs() < 0.1,
        "y should be ~50, got {}",
        bounds.y
    );
}

#[test]
fn test_push_icon_spec_found() {
    let def = make_triangle_def(42);
    let defs: &'static [IconDefinition] = Box::leak(Box::new([def]));
    let registry = IconRegistry::new(defs);
    let spec = IconSpec {
        id: IconId(42),
        size: 16.0,
        color: Color::WHITE,
        stroke_width: None,
        mode: IconRenderMode::Fill,
    };

    let mut builder = SceneBuilder::new();
    let found = builder.push_icon_spec(&spec, &registry, 0.0, 0.0, 0);
    assert!(found);
    let scene = builder.build();
    assert_eq!(scene.root.len(), 1);
    assert!(matches!(&scene.root[0], SceneNode::Path(_)));
}

#[test]
fn test_push_icon_spec_missing() {
    let defs: &'static [IconDefinition] = &[];
    let registry = IconRegistry::new(defs);
    let spec = IconSpec {
        id: IconId(999),
        size: 16.0,
        color: Color::WHITE,
        stroke_width: None,
        mode: IconRenderMode::Fill,
    };

    let mut builder = SceneBuilder::new();
    let found = builder.push_icon_spec(&spec, &registry, 0.0, 0.0, 0);
    assert!(!found);
    let scene = builder.build();
    assert_eq!(scene.root.len(), 0);
}

#[test]
fn test_path_transformed_correctness() {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(24.0, 0.0);
    path.line_to(24.0, 24.0);
    path.close();

    // scale=0.5, translate=(10, 20)
    let t = path.transformed(0.5, 10.0, 20.0);
    let bounds = t.bounds();
    // (0,0)→(10,20), (24,0)→(22,20), (24,24)→(22,32)
    assert!(
        (bounds.x - 10.0).abs() < 0.1,
        "x should be ~10, got {}",
        bounds.x
    );
    assert!(
        (bounds.y - 20.0).abs() < 0.1,
        "y should be ~20, got {}",
        bounds.y
    );
    assert!(
        (bounds.width - 12.0).abs() < 0.1,
        "width should be ~12, got {}",
        bounds.width
    );
    assert!(
        (bounds.height - 12.0).abs() < 0.1,
        "height should be ~12, got {}",
        bounds.height
    );
}
