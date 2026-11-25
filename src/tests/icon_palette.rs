use crate::core::icon::{IconId, IconRenderMode, IconRole, LayerRole};
use crate::core::icon_palette::{deserialize_palette, load_registry, serialize_palette};
use crate::core::icons::builtin_registry;

const ALL_IDS: [IconId; 12] = [
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
];

#[test]
fn test_round_trip_single() {
    let reg = builtin_registry();
    let close = reg.get(IconId::CLOSE).unwrap();
    let data = serialize_palette(&[close]);
    let result = deserialize_palette(&data).unwrap();
    assert_eq!(result.len(), 1);
    let r = &result[0];
    assert_eq!(r.id, close.id);
    assert_eq!(r.name, close.name);
    assert_eq!(r.viewbox_width, close.viewbox_width);
    assert_eq!(r.viewbox_height, close.viewbox_height);
    assert_eq!(r.default_mode, close.default_mode);
    assert_eq!(r.role, close.role);
    assert_eq!(r.layers.len(), close.layers.len());
    for (rl, ol) in r.layers.iter().zip(close.layers.iter()) {
        assert_eq!(rl.role, ol.role);
        assert_eq!(rl.fill_rule, ol.fill_rule);
    }
    assert_eq!(r.path_hash, close.path_hash);
}

#[test]
fn test_round_trip_multi_layer() {
    let reg = builtin_registry();
    let folder = reg.get(IconId::FOLDER).unwrap();
    assert_eq!(folder.layers.len(), 3, "folder should have 3 layers");
    let data = serialize_palette(&[folder]);
    let result = deserialize_palette(&data).unwrap();
    assert_eq!(result.len(), 1);
    let r = &result[0];
    assert_eq!(r.layers.len(), 3);
    assert_eq!(r.layers[0].role, LayerRole::Shadow);
    assert_eq!(r.layers[1].role, LayerRole::Base);
    assert_eq!(r.layers[2].role, LayerRole::Highlight);
    assert_eq!(r.path_hash, folder.path_hash);
}

#[test]
fn test_round_trip_batch() {
    let reg = builtin_registry();
    let icons: Vec<&_> = ALL_IDS.iter().map(|id| reg.get(*id).unwrap()).collect();
    let data = serialize_palette(&icons);
    let result = deserialize_palette(&data).unwrap();
    assert_eq!(result.len(), 12);
    for (original, deserialized) in icons.iter().zip(result.iter()) {
        assert_eq!(deserialized.id, original.id);
        assert_eq!(
            deserialized.path_hash, original.path_hash,
            "path_hash mismatch for {:?}",
            original.name
        );
    }
}

#[test]
fn test_invalid_magic() {
    let data = b"NOPE\x01\x00\x00\x00";
    let result = deserialize_palette(data);
    assert!(matches!(
        result,
        Err(crate::core::icon_palette::PaletteError::InvalidMagic)
    ));
}

#[test]
fn test_unsupported_version() {
    let mut data = Vec::new();
    data.extend_from_slice(b"HALP");
    data.extend_from_slice(&99u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    let result = deserialize_palette(&data);
    assert!(matches!(
        result,
        Err(crate::core::icon_palette::PaletteError::UnsupportedVersion(
            99
        ))
    ));
}

#[test]
fn test_load_registry() {
    let reg = builtin_registry();
    let icons: Vec<&_> = ALL_IDS.iter().map(|id| reg.get(*id).unwrap()).collect();
    let data = serialize_palette(&icons);
    let loaded = load_registry(&data).unwrap();
    assert_eq!(loaded.len(), 12);
    for id in &ALL_IDS {
        assert!(loaded.get(*id).is_some(), "lookup failed for {:?}", id);
    }
}
