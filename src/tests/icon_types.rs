use crate::core::icon::*;
use crate::core::scene::Color;
use crate::renderer::path::Path;

#[test]
fn test_icon_id_copy_eq() {
    let a = IconId(1);
    let b = a; // Copy
    assert_eq!(a, b);

    let c = IconId(2);
    assert_ne!(a, c);
}

#[test]
fn test_icon_size_values() {
    assert_eq!(IconSize::XS.logical_px(), 10.0);
    assert_eq!(IconSize::S.logical_px(), 12.0);
    assert_eq!(IconSize::M.logical_px(), 14.0);
    assert_eq!(IconSize::L.logical_px(), 16.0);
    assert_eq!(IconSize::XL.logical_px(), 20.0);
}

fn make_dummy_definition(id: u32) -> IconDefinition {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(16.0, 0.0);
    path.line_to(16.0, 16.0);
    path.close();
    let path_hash = path.content_hash();
    IconDefinition {
        id: IconId(id),
        name: "test-icon",
        layers: vec![IconLayer {
            role: LayerRole::Base,
            path,
            fill_rule: FillRule::NonZero,
            skip_precision: false,
        }],
        viewbox_width: 16.0,
        viewbox_height: 16.0,
        default_mode: IconRenderMode::Fill,
        role: IconRole::Action,
        path_hash,
    }
}

#[test]
fn test_icon_registry_lookup() {
    let def = make_dummy_definition(42);
    let defs: &'static [IconDefinition] = Box::leak(Box::new([def]));
    let registry = IconRegistry::new(defs);

    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());

    let found = registry.get(IconId(42));
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, IconId(42));
    assert_eq!(found.unwrap().name, "test-icon");

    assert!(registry.get(IconId(999)).is_none());
}

#[test]
fn test_icon_registry_empty() {
    let defs: &'static [IconDefinition] = &[];
    let registry = IconRegistry::new(defs);

    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert!(registry.get(IconId(0)).is_none());
    assert!(registry.get(IconId(1)).is_none());
}

#[test]
fn test_icon_definition_path_hash() {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(10.0, 5.0);
    path.line_to(5.0, 10.0);
    path.close();
    let expected_hash = path.content_hash();

    let def = IconDefinition {
        id: IconId(1),
        name: "hash-test",
        layers: vec![IconLayer {
            role: LayerRole::Base,
            path,
            fill_rule: FillRule::EvenOdd,
            skip_precision: false,
        }],
        viewbox_width: 10.0,
        viewbox_height: 10.0,
        default_mode: IconRenderMode::Stroke,
        role: IconRole::Status,
        path_hash: expected_hash,
    };

    assert_eq!(def.path_hash, def.layers[0].path.content_hash());
}
