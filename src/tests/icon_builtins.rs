use crate::core::icon::{IconDefinition, IconId, IconRegistry};
use crate::core::icons::builtin_registry;
use std::collections::HashSet;

// Expected public inventory, in registry order.
const BUILTIN_IDS: [IconId; 20] = [
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
    IconId::CALENDAR,
    IconId::SELECT_ALL,
    IconId::CLEAR_ALL,
    IconId::SETTINGS,
    IconId::GLOBE,
    IconId::MAIL,
    IconId::BRIEFCASE,
];

fn builtin_definitions(registry: &IconRegistry) -> impl Iterator<Item = &IconDefinition> {
    BUILTIN_IDS.into_iter().map(|id| {
        registry
            .get(id)
            .unwrap_or_else(|| panic!("built-in icon {id:?} missing from registry"))
    })
}

#[test]
fn test_builtin_registry_count() {
    let registry = builtin_registry();
    assert_eq!(registry.len(), BUILTIN_IDS.len());
}

#[test]
fn test_builtin_ids_unique() {
    let registry = builtin_registry();
    let unique: HashSet<_> = builtin_definitions(&registry).map(|def| def.id).collect();
    assert_eq!(unique.len(), BUILTIN_IDS.len());
}

#[test]
fn test_builtin_lookup_all() {
    let registry = builtin_registry();
    for (expected, def) in BUILTIN_IDS.into_iter().zip(builtin_definitions(&registry)) {
        assert_eq!(def.id, expected);
    }
}

#[test]
fn test_builtin_viewbox_24() {
    let registry = builtin_registry();
    for def in builtin_definitions(&registry) {
        assert_eq!(
            (def.viewbox_width, def.viewbox_height),
            (24.0, 24.0),
            "{}: viewbox must be 24 × 24",
            def.name
        );
    }
}

#[test]
fn test_builtin_path_hash_consistent() {
    let registry = builtin_registry();
    for def in builtin_definitions(&registry) {
        let recomputed = def.layers.iter().fold(0_u64, |hash, layer| {
            hash.wrapping_mul(31)
                .wrapping_add(layer.path.content_hash() ^ (layer.role as u64))
        });
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
    for def in builtin_definitions(&registry) {
        assert!(
            !def.layers.is_empty(),
            "{}: expected a path layer",
            def.name
        );
        for (index, layer) in def.layers.iter().enumerate() {
            let mesh = layer
                .path
                .try_to_mesh_with_fill_rule(layer.fill_rule)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} layer {index} under {:?}: {error}",
                        def.name, layer.fill_rule
                    )
                });
            assert!(
                !mesh.vertices.is_empty() && !mesh.indices.is_empty(),
                "{} layer {index} under {:?}: expected nonempty geometry",
                def.name,
                layer.fill_rule
            );
        }
    }
}
