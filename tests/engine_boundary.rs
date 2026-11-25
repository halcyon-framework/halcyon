// =============================================================================
// Engine Boundary Regression Test
//
// Ensures no file under src/engines/ imports from crate::widgets::.
// Engines must depend only on model-layer types (crate::model::*).
// =============================================================================

use std::fs;
use std::path::Path;

const FORBIDDEN_PATTERNS: &[&str] = &[
    "crate::widgets::",
    "widgets::tabular_view::column",
    "widgets::tabular_view::row",
];

fn collect_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
}

#[test]
fn engine_files_must_not_import_from_widgets() {
    let engines_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/engines");
    assert!(engines_dir.exists(), "src/engines/ directory not found");

    let mut files = Vec::new();
    collect_rs_files(&engines_dir, &mut files);
    assert!(!files.is_empty(), "no .rs files found under src/engines/");

    let mut violations = Vec::new();

    for file in &files {
        let content = fs::read_to_string(file).expect("failed to read engine file");
        for (line_num, line) in content.lines().enumerate() {
            for &pattern in FORBIDDEN_PATTERNS {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{}: {}",
                        file.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                            .unwrap_or(file)
                            .display(),
                        line_num + 1,
                        line.trim(),
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Engine boundary violation: engines must not import from widgets.\n{}",
        violations.join("\n")
    );
}
