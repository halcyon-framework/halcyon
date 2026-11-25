// Geometry Authority Guard Test
// SPEC-HAL-096C enforcement: widget geometry must come from theme tokens in layout() only.
//
// This test scans widget source files and verifies that direct theme geometry access
// (cx.theme.borders, cx.theme.padding, cx.theme.radii, cx.theme.spacing)
// only occurs within layout() functions, not in paint() or handle_event().

use std::fs;
use std::path::Path;

/// Files to check (framework widgets only, excluding demos)
const WIDGET_FILES: &[&str] = &[
    "src/widgets/button.rs",
    "src/widgets/panel.rs",
    "src/widgets/groupbox.rs",
    "src/widgets/scroll_area.rs",
    "src/widgets/scrollbar.rs",
    "src/widgets/text_area.rs",
    "src/widgets/text_field.rs",
    "src/widgets/checkbox.rs",
    "src/widgets/radio_button.rs",
    "src/widgets/divider.rs",
    "src/widgets/label.rs",
    "src/widgets/container.rs",
    "src/widgets/flex.rs",
];

/// Geometry token patterns that must only appear in layout()
const GEOMETRY_PATTERNS: &[&str] = &[
    "cx.theme.borders",
    "cx.theme.padding",
    "cx.theme.radii",
    "cx.theme.spacing",
    "cx.theme.geometry",
];

/// Function contexts
#[derive(Debug, Clone, Copy, PartialEq)]
enum FnContext {
    None,
    Layout,
    Paint,
    HandleEvent,
    Other,
}

/// Check if a line starts a function definition
fn detect_fn_start(line: &str) -> Option<FnContext> {
    let trimmed = line.trim();
    if trimmed.starts_with("fn layout") || trimmed.starts_with("pub fn layout") {
        Some(FnContext::Layout)
    } else if trimmed.starts_with("fn paint") || trimmed.starts_with("pub fn paint") {
        Some(FnContext::Paint)
    } else if trimmed.starts_with("fn handle_event") || trimmed.starts_with("pub fn handle_event") {
        Some(FnContext::HandleEvent)
    } else if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
        Some(FnContext::Other)
    } else {
        None
    }
}

/// Check if a line contains an exemption marker
fn has_exemption_marker(line: &str) -> bool {
    line.contains("INV-") || line.contains("SPEC-HAL-")
}

/// Scan a file for geometry authority violations
fn check_file(path: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            // File doesn't exist is not a violation (widget may not exist yet)
            if e.kind() != std::io::ErrorKind::NotFound {
                violations.push(format!("{}: failed to read: {}", path.display(), e));
            }
            return violations;
        }
    };

    let mut current_context = FnContext::None;
    let mut brace_depth = 0;
    let mut fn_start_depth = 0;
    let mut fn_has_exemption = false; // Track if current function has exemption

    for (line_num, line) in content.lines().enumerate() {
        let line_num = line_num + 1; // 1-indexed for display

        // Track brace depth for function boundaries
        for ch in line.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    // If we drop below the function start depth, we've exited
                    if brace_depth < fn_start_depth && current_context != FnContext::None {
                        current_context = FnContext::None;
                        fn_has_exemption = false;
                    }
                }
                _ => {}
            }
        }

        // Detect function boundaries
        if let Some(ctx) = detect_fn_start(line) {
            current_context = ctx;
            fn_start_depth = brace_depth;
            fn_has_exemption = false;
        }

        // Track exemption markers within current function
        if has_exemption_marker(line) {
            fn_has_exemption = true;
        }

        // Check for geometry patterns
        for pattern in GEOMETRY_PATTERNS {
            if line.contains(pattern) {
                match current_context {
                    FnContext::Layout => {
                        // Allowed - this is the canonical sync location
                    }
                    FnContext::Paint | FnContext::HandleEvent => {
                        // Check for exemption markers anywhere in this function
                        // INV-* or SPEC-HAL-* comments indicate documented decisions
                        if !fn_has_exemption && !has_exemption_marker(line) {
                            violations.push(format!(
                                "{}:{}: geometry access `{}` in {:?} (must be in layout())",
                                path.display(),
                                line_num,
                                pattern,
                                current_context
                            ));
                        }
                    }
                    FnContext::None | FnContext::Other => {
                        // Could be in a helper or struct definition - less strict
                        // Only flag if clearly in a Widget impl method context
                    }
                }
            }
        }
    }

    violations
}

#[test]
fn geometry_authority_enforcement() {
    let mut all_violations = Vec::new();

    for file in WIDGET_FILES {
        let path = Path::new(file);
        let violations = check_file(path);
        all_violations.extend(violations);
    }

    if !all_violations.is_empty() {
        panic!(
            "Geometry authority violations found:\n\n{}\n\n\
             Geometry tokens (borders, padding, radii, spacing) must be read in layout() only.\n\
             See SPEC-HAL-096C and widgets/mod.rs for enforcement rules.",
            all_violations.join("\n")
        );
    }
}

#[test]
fn widget_files_exist() {
    // Sanity check: at least some widget files should exist
    let existing: Vec<_> = WIDGET_FILES
        .iter()
        .filter(|f| Path::new(f).exists())
        .collect();

    assert!(
        existing.len() >= 5,
        "Expected at least 5 widget files to exist, found {}",
        existing.len()
    );
}
