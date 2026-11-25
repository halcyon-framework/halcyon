use crate::core::icon::{IconDefinition, IconId};
use crate::core::icons::builtin_registry;

/// Measure the fill ratio of an icon at a given target size.
///
/// Scales each layer's path to `target_size`, tessellates, and computes
/// total triangle area / (target_size^2).
fn measure_fill_ratio(definition: &IconDefinition, target_size: f32) -> f32 {
    let scale = target_size / definition.viewbox_width;
    let mut total_area = 0.0f32;

    for layer in &definition.layers {
        let scaled = layer.path.transformed(scale, 0.0, 0.0);
        let mesh = scaled.to_mesh();

        let verts = &mesh.vertices;
        let indices = &mesh.indices;

        for tri in indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let (x1, y1) = (verts[i0][0], verts[i0][1]);
            let (x2, y2) = (verts[i1][0], verts[i1][1]);
            let (x3, y3) = (verts[i2][0], verts[i2][1]);
            let area = 0.5 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2)).abs();
            total_area += area;
        }
    }

    total_area / (target_size * target_size)
}

const ALL_ICONS: [(IconId, &str); 13] = [
    (IconId::CLOSE, "close"),
    (IconId::CHEVRON_RIGHT, "chevron-right"),
    (IconId::CHEVRON_DOWN, "chevron-down"),
    (IconId::PLUS, "plus"),
    (IconId::MINUS, "minus"),
    (IconId::CHECK, "check"),
    (IconId::SEARCH, "search"),
    (IconId::ARROW_UP, "arrow-up"),
    (IconId::ARROW_DOWN, "arrow-down"),
    (IconId::WARNING, "warning"),
    (IconId::FOLDER, "folder"),
    (IconId::FILE, "file"),
    (IconId::FOLDER_OPEN, "folder-open"),
];

const SIZES: [f32; 5] = [10.0, 12.0, 14.0, 16.0, 20.0];

/// Target fill ratio ranges per size token.
const TARGET_RANGES: [(f32, f32); 5] = [
    (0.30, 0.40), // 10px
    (0.30, 0.40), // 12px
    (0.25, 0.35), // 14px
    (0.25, 0.35), // 16px
    (0.20, 0.30), // 20px
];

#[test]
fn test_fill_ratio_audit() {
    let registry = builtin_registry();

    eprintln!();
    eprintln!("=== SPEC-265A §6.3: Icon Fill Ratio Audit ===");
    eprintln!(
        "{:<16}| {:>5} | {:>5} | {:>5} | {:>5} | {:>5}",
        "Icon", "10px", "12px", "14px", "16px", "20px"
    );
    eprintln!("----------------+-------+-------+-------+-------+------");

    let mut warnings = Vec::new();

    for &(id, name) in &ALL_ICONS {
        let def = registry.get(id).expect(name);
        let mut ratios = [0.0f32; 5];

        for (si, &size) in SIZES.iter().enumerate() {
            ratios[si] = measure_fill_ratio(def, size);
        }

        eprintln!(
            "{:<16}| {:.2}  | {:.2}  | {:.2}  | {:.2}  | {:.2}",
            name, ratios[0], ratios[1], ratios[2], ratios[3], ratios[4]
        );

        for (si, &size) in SIZES.iter().enumerate() {
            let (lo, hi) = TARGET_RANGES[si];
            if ratios[si] < lo || ratios[si] > hi {
                warnings.push(format!(
                    "WARNING: {} at {}px = {:.3} (target {:.2}–{:.2})",
                    name, size as u32, ratios[si], lo, hi
                ));
            }
        }
    }

    eprintln!();
    if warnings.is_empty() {
        eprintln!("All icons within target ranges.");
    } else {
        for w in &warnings {
            eprintln!("{}", w);
        }
    }
    eprintln!();
}

#[test]
fn test_fill_ratio_positive() {
    let registry = builtin_registry();

    for &(id, name) in &ALL_ICONS {
        let def = registry.get(id).expect(name);
        for &size in &SIZES {
            let ratio = measure_fill_ratio(def, size);
            assert!(
                ratio > 0.0,
                "{} at {}px has zero fill ratio — degenerate geometry",
                name,
                size as u32
            );
        }
    }
}
