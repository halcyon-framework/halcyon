use crate::core::icon::{IconId, LayerRole};
use crate::core::icons::builtin_registry;
use std::collections::HashSet;

#[test]
fn test_builtin_registry_count() {
    let registry = builtin_registry();
    assert_eq!(registry.len(), 17);
}

#[test]
fn test_builtin_ids_unique() {
    let registry = builtin_registry();
    let all_ids: Vec<IconId> = [
        IconId::CLOSE,
        IconId::CHEVRON_RIGHT,
        IconId::CHEVRON_DOWN,
        IconId::PLUS,
        IconId::MINUS,
        IconId::CHECK,
        IconId::SEARCH,
        IconId::ARROW_UP,
        IconId::ARROW_DOWN,
        IconId::WARNING,
        IconId::FOLDER,
        IconId::FILE,
        IconId::FOLDER_OPEN,
        IconId::SETTINGS,
    ]
    .to_vec();

    let unique: HashSet<_> = all_ids.iter().collect();
    assert_eq!(
        unique.len(),
        all_ids.len(),
        "all listed IconIds must be distinct"
    );

    // Also verify each exists in registry
    for id in &all_ids {
        assert!(
            registry.get(*id).is_some(),
            "IconId {:?} missing from registry",
            id
        );
    }
}

#[test]
fn test_builtin_lookup_all() {
    let registry = builtin_registry();
    let ids = [
        IconId::CLOSE,
        IconId::CHEVRON_RIGHT,
        IconId::CHEVRON_DOWN,
        IconId::PLUS,
        IconId::MINUS,
        IconId::CHECK,
        IconId::SEARCH,
        IconId::ARROW_UP,
        IconId::ARROW_DOWN,
        IconId::WARNING,
        IconId::FOLDER,
        IconId::FILE,
        IconId::FOLDER_OPEN,
        IconId::SETTINGS,
    ];
    for id in &ids {
        let def = registry.get(*id);
        assert!(def.is_some(), "registry.get({:?}) returned None", id);
        assert_eq!(def.unwrap().id, *id);
    }
}

#[test]
fn test_builtin_viewbox_24() {
    let registry = builtin_registry();
    let ids = [
        IconId::CLOSE,
        IconId::CHEVRON_RIGHT,
        IconId::CHEVRON_DOWN,
        IconId::PLUS,
        IconId::MINUS,
        IconId::CHECK,
        IconId::SEARCH,
        IconId::ARROW_UP,
        IconId::ARROW_DOWN,
        IconId::WARNING,
        IconId::FOLDER,
        IconId::FILE,
        IconId::FOLDER_OPEN,
        IconId::SETTINGS,
    ];
    for id in &ids {
        let def = registry.get(*id).unwrap();
        assert_eq!(
            def.viewbox_width, 24.0,
            "{}: viewbox_width != 24.0",
            def.name
        );
        assert_eq!(
            def.viewbox_height, 24.0,
            "{}: viewbox_height != 24.0",
            def.name
        );
    }
}

#[test]
fn test_builtin_path_hash_consistent() {
    let registry = builtin_registry();
    let ids = [
        IconId::CLOSE,
        IconId::CHEVRON_RIGHT,
        IconId::CHEVRON_DOWN,
        IconId::PLUS,
        IconId::MINUS,
        IconId::CHECK,
        IconId::SEARCH,
        IconId::ARROW_UP,
        IconId::ARROW_DOWN,
        IconId::WARNING,
        IconId::FOLDER,
        IconId::FILE,
        IconId::FOLDER_OPEN,
        IconId::SETTINGS,
    ];
    for id in &ids {
        let def = registry.get(*id).unwrap();
        let recomputed: u64 = {
            let mut h: u64 = 0;
            for layer in &def.layers {
                let lh = layer.path.content_hash();
                h = h.wrapping_mul(31).wrapping_add(lh ^ (layer.role as u64));
            }
            h
        };
        assert_eq!(
            def.path_hash, recomputed,
            "{}: path_hash does not match recomputed layer hash",
            def.name
        );
    }
}

#[test]
fn test_builtin_paths_nonempty() {
    let registry = builtin_registry();
    let ids = [
        IconId::CLOSE,
        IconId::CHEVRON_RIGHT,
        IconId::CHEVRON_DOWN,
        IconId::PLUS,
        IconId::MINUS,
        IconId::CHECK,
        IconId::SEARCH,
        IconId::ARROW_UP,
        IconId::ARROW_DOWN,
        IconId::WARNING,
        IconId::FOLDER,
        IconId::FILE,
        IconId::FOLDER_OPEN,
        IconId::SETTINGS,
    ];
    for id in &ids {
        let def = registry.get(*id).unwrap();
        let mesh = def.layers[0].path.to_mesh();
        assert!(
            !mesh.vertices.is_empty(),
            "{}: path tessellates to empty mesh",
            def.name
        );
    }
}
