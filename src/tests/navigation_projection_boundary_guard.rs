//! SPEC-HAL-246 / SPEC-HAL-248: Navigation Projection Boundary Guard
//!
//! This file performs real source scanning of Rust files as text. This is intentional.
//!
//! These tests enforce negative architectural laws: constraints of the form "this must never
//! exist anywhere in the codebase." They read `.rs` files from disk, iterate lines, skip
//! comments, and match against forbidden patterns. If a match is found, the test fails with
//! file path, line number, and reason. The guard is designed to fail loudly at compile/test
//! time, not to be elegant.
//!
//! AST-based approaches (tree-sitter, syn, or any parser) are intentionally rejected:
//! - AST parsers answer "what does this code mean?" These guards answer "does this forbidden
//!   shape exist anywhere?" The question is lexical and absolute, not semantic.
//! - AST normalization hides violations. Imports via re-exports, macro expansion, type
//!   aliasing, and glob imports all survive AST resolution while violating the architectural
//!   boundary. Text scanning catches them.
//! - Tree-sitter introduces tooling dependency weight, parser version drift risk, and silent
//!   failure modes when Rust syntax evolves. These guards have zero dependencies beyond `std::fs`.
//! - The moment an AST exists, developers start reasoning about intent instead of law.
//!   "The AST says this import is unused" or "the macro expands to something safe" are
//!   exactly the rationalizations these guards exist to prevent.
//! - Tree-sitter would make this file look more sophisticated. It would also make the
//!   boundary weaker. It is rejected on architectural grounds, not technical limitation.
//!
//! These guards must break before cleverness can justify violations. If this test fails,
//! the architecture is already broken, regardless of the developer's intent.
//!
//! If someone feels tempted to "upgrade" this to AST parsing, they are attempting to relax
//! a hard architectural law into a soft convention. That is a violation of SPEC-HAL-246 and
//! SPEC-HAL-248, not an improvement.
//!
//! ## Guards enforced
//!
//! 1. No widget other than ScreenHost imports ProjectedSurfaceView.
//! 2. No renderer file imports ProjectedSurfaceView or either projection type.
//! 3. No widget imports either projection type or NavigationSurfaceOrchestrator.
//! 4. ScreenHost has zero navigation imports (INV-248-07).
//! 5. Runtime glue (pull_navigation_frame_projection) is the sole call site for:
//!    - orchestrator.project()
//!    - compositor.apply_frame_projection()
//!    - set_projected_view()
//! 6. No cached projection state exists anywhere in src/.
//!
//! If any of these constraints are violated, the test fails with file, line, and reason.

use std::fs;
use std::path::Path;

// ============================================================================
// Helpers
// ============================================================================

/// Scan all .rs files under a directory, returning (relative_path, content) pairs.
/// Returned relative paths use "/" separators for guard classification.
fn scan_rs_files(dir: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    collect_rs_files(Path::new(dir), &mut results);
    results
}

fn collect_rs_files(dir: &Path, results: &mut Vec<(String, String)>) {
    if !dir.is_dir() {
        return;
    }
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("Failed to read dir {:?}: {}", dir, e));
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, results);
        } else if path.extension().map_or(false, |ext| ext == "rs") {
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", path, e));
            results.push((
                path.display()
                    .to_string()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
                content,
            ));
        }
    }
}

#[test]
fn source_scan_paths_match_existing_guard_scopes() {
    let files = scan_rs_files("src");
    for path in [
        "src/tests/navigation_projection_boundary_guard.rs",
        "src/compositor/mod.rs",
        "src/core/frame_projection_binding_registry.rs",
        "src/core/app.rs",
        "src/core/app/ui_loop.rs",
        "src/core/app/app_root.rs",
        "src/core/app/run.rs",
    ] {
        let (_, content) = files
            .iter()
            .find(|(scanned_path, _)| scanned_path == path)
            .unwrap_or_else(|| panic!("Missing scanned source path: {path}"));
        let expected = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("Failed to read {path}: {error}"));
        assert_eq!(
            content, &expected,
            "Scanned source content differs for {path}"
        );
    }
}

/// Check non-comment lines for a pattern. Returns violations as (file, line_num, line_text).
fn find_in_non_comment_lines(
    files: &[(String, String)],
    pattern: &str,
) -> Vec<(String, usize, String)> {
    let mut hits = Vec::new();
    for (path, content) in files {
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
                continue;
            }
            if line.contains(pattern) {
                hits.push((path.clone(), line_num + 1, line.to_string()));
            }
        }
    }
    hits
}

// ============================================================================
// Guard 1: No widget other than ScreenHost imports ProjectedSurfaceView
// (INV-248-04, FP-248-04)
// ============================================================================

#[test]
fn no_widget_except_screen_host_imports_projected_surface_view() {
    let files = scan_rs_files("src/widgets");
    let mut violations = Vec::new();

    for (path, content) in &files {
        if path.ends_with("screen_host.rs") {
            continue;
        }
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
                continue;
            }
            if line.contains("ProjectedSurfaceView") {
                violations.push(format!("{}:{}: {}", path, line_num + 1, line.trim()));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\nINV-248-04 VIOLATION: Widget other than ScreenHost references ProjectedSurfaceView\n\n{}\n\n\
             Only ScreenHost may import or reference ProjectedSurfaceView.\n\
             See: SPEC-HAL-248 §5 INV-248-04, §6 FP-248-04\n",
            violations.join("\n")
        );
    }
}

// ============================================================================
// Guard 2: No renderer file imports ProjectedSurfaceView or NavigationProjection
// (FP-248-04, FP-248-06)
// ============================================================================

#[test]
fn renderer_has_no_projection_imports() {
    let files = scan_rs_files("src/renderer");
    let mut violations = Vec::new();

    let forbidden = &[
        (
            "ProjectedSurfaceView",
            "Renderer must not access ProjectedSurfaceView (FP-248-04)",
        ),
        (
            "NavigationProjection",
            "Renderer must not access NavigationProjection (FP-248-06)",
        ),
        (
            "NavigationFrameProjection",
            "Renderer must not access NavigationFrameProjection (FP-248-06)",
        ),
        (
            "NavigationSurfaceOrchestrator",
            "Renderer must not access NavigationSurfaceOrchestrator",
        ),
        ("NavigationHost", "Renderer must not access NavigationHost"),
        (
            "navigation_orchestrator",
            "Renderer must not import navigation_orchestrator module",
        ),
        (
            "navigation_host",
            "Renderer must not import navigation_host module",
        ),
    ];

    for (path, content) in &files {
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
                continue;
            }
            for (pattern, reason) in forbidden {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{}: Found '{}'\n  Reason: {}",
                        path,
                        line_num + 1,
                        pattern,
                        reason
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\nSPEC-HAL-248 VIOLATION: Renderer references projection or navigation types\n\n{}\n\n\
             The renderer is a blind executor. It must not access projection or navigation types.\n\
             See: SPEC-HAL-248 §6 FP-248-04, FP-248-06\n",
            violations.join("\n\n")
        );
    }
}

// ============================================================================
// Guard 3: No widget imports projection authority types
// (FP-248-04, FP-248-06)
// ============================================================================

#[test]
fn no_widget_imports_projection_authority_types() {
    let files = scan_rs_files("src/widgets");
    let mut violations = Vec::new();

    let forbidden = &[
        (
            "NavigationProjection",
            "Widget must not access NavigationProjection (FP-248-06)",
        ),
        (
            "NavigationFrameProjection",
            "Widget must not access NavigationFrameProjection (FP-248-06)",
        ),
        (
            "NavigationSurfaceOrchestrator",
            "Widget must not access NavigationSurfaceOrchestrator",
        ),
        (
            "navigation_orchestrator",
            "Widget must not import navigation_orchestrator module",
        ),
    ];

    for (path, content) in &files {
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
                continue;
            }
            for (pattern, reason) in forbidden {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{}: Found '{}'\n  Reason: {}",
                        path,
                        line_num + 1,
                        pattern,
                        reason
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\nSPEC-HAL-248 VIOLATION: Widget references projection authority types\n\n{}\n\n\
             Widgets must not import or reference either projection type or NavigationSurfaceOrchestrator.\n\
             See: SPEC-HAL-248 §6 FP-248-04, FP-248-06\n",
            violations.join("\n\n")
        );
    }
}

// ============================================================================
// Guard 4: ScreenHost has zero navigation imports
// (INV-248-07)
// ============================================================================

#[test]
fn screen_host_has_zero_navigation_imports() {
    let path = "src/widgets/screen_host.rs";
    let content =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));

    let mut violations = Vec::new();

    let forbidden_imports = &[
        "navigation_orchestrator",
        "navigation_host",
        "NavigationProjection",
        "NavigationSurfaceOrchestrator",
        "NavigationHost",
        "NavigationIntent",
        "NavigationNodeId",
    ];

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
            continue;
        }
        for pattern in forbidden_imports {
            if line.contains(pattern) {
                violations.push(format!(
                    "{}:{}: Found '{}' in non-comment code",
                    path,
                    line_num + 1,
                    pattern
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\nINV-248-07 VIOLATION: ScreenHost contains navigation imports\n\n{}\n\n\
             ScreenHost must import zero navigation types.\n\
             Its only projection-related import is crate::compositor::ProjectedSurfaceView.\n\
             See: SPEC-HAL-248 §5 INV-248-07\n",
            violations.join("\n")
        );
    }
}

// ============================================================================
// Guard 5: Sole call-site enforcement for projection pipeline
// (INV-248-01, INV-248-04, FP-248-08)
//
// Guard 5 uses receiver-anchored string matching. This means the guard does
// not search for `.project()` generically — it searches for specific receiver
// expressions like `orchestrator.project(` or `compositor.apply_frame_projection(`.
//
// Why receiver-anchored:
// - A generic `.project()` scan would false-positive on any Rust type that
//   happens to have a method named `project`. Iterator adapters, camera
//   transforms, matrix operations — none of these are architectural violations,
//   but a naïve substring match would flag them.
// - False positives are more dangerous than false negatives here. A false
//   positive trains developers to ignore the guard. A false negative is caught
//   in code review. The guard prefers precision over generality.
// - The receiver names are drawn from the actual runtime glue code in app.rs.
//   If the runtime code renames its variables, the receiver list is updated,
//   not relaxed.
//
// The synthetic validation test below (guard5_does_not_false_positive_on_
// unrelated_project) exists to prove the matcher is not naïve. It feeds
// unrelated `.project()` calls through the matcher and asserts zero hits.
// ============================================================================

/// Receiver patterns that identify orchestrator.project() calls.
/// Any line matching one of these (outside comments) is a call to the
/// NavigationSurfaceOrchestrator projection accessor.
const ORCHESTRATOR_PROJECT_RECEIVERS: &[&str] = &[
    "orchestrator.project(",
    "orch.project(",
    "nav_orchestrator.project(",
    "self.orchestrator.project(",
    "self.orch.project(",
    "self.nav_orchestrator.project(",
    "root.project(",
];

/// Receiver patterns that identify compositor.apply_frame_projection() calls.
const COMPOSITOR_APPLY_PROJECTION_RECEIVERS: &[&str] = &[
    "compositor.apply_frame_projection(",
    "self.compositor.apply_frame_projection(",
    "runtime.compositor.apply_frame_projection(",
    ".apply_frame_projection(",
];

/// Receiver patterns that identify ScreenHost::set_projected_view() calls.
const SCREEN_HOST_SET_PROJECTED_VIEW_RECEIVERS: &[&str] = &[
    "sh.set_projected_view(",
    "screen_host.set_projected_view(",
    "self.screen_host.set_projected_view(",
    ".set_projected_view(",
];

/// Check if a line matches any of the given receiver patterns.
fn line_matches_any_receiver(line: &str, receivers: &[&str]) -> bool {
    receivers.iter().any(|pat| line.contains(pat))
}

/// Find all non-comment lines in files matching any receiver pattern.
/// Returns (file, line_num, line_text) triples.
fn find_receiver_calls(
    files: &[(String, String)],
    receivers: &[&str],
) -> Vec<(String, usize, String)> {
    let mut hits = Vec::new();
    for (path, content) in files {
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
                continue;
            }
            if line_matches_any_receiver(line, receivers) {
                hits.push((path.clone(), line_num + 1, line.to_string()));
            }
        }
    }
    hits
}

#[test]
fn project_called_from_single_runtime_location() {
    let files = scan_rs_files("src");
    let hits = find_receiver_calls(&files, ORCHESTRATOR_PROJECT_RECEIVERS);

    // Exclude: navigation_orchestrator.rs (defines project()), src/tests/ (guard tests)
    let caller_hits: Vec<_> = hits
        .iter()
        .filter(|(path, _, _)| {
            !path.contains("navigation_orchestrator.rs") && !path.contains("src/tests/")
        })
        .collect();

    let runtime_hits: Vec<_> = caller_hits
        .iter()
        .filter(|(path, _, _)| path.contains("frame_projection_binding_registry.rs"))
        .collect();
    let non_runtime_hits: Vec<_> = caller_hits
        .iter()
        .filter(|(path, _, _)| !path.contains("frame_projection_binding_registry.rs"))
        .collect();

    assert!(
        !runtime_hits.is_empty(),
        "SPEC-HAL-248: Expected project() calls in the frame-projection runtime glue"
    );

    if !non_runtime_hits.is_empty() {
        let msgs: Vec<String> = non_runtime_hits
            .iter()
            .map(|(p, l, t)| format!("{}:{}: {}", p, l, t.trim()))
            .collect();
        panic!(
            "\n\nFP-248-08 VIOLATION: orchestrator.project() called outside pull_navigation_frame_projection()\n\n{}\n\n\
             orchestrator.project() must only be called from the bounded runtime glue.\n\
             See: SPEC-HAL-248 §3, §6 FP-248-08\n",
            msgs.join("\n")
        );
    }
}

#[test]
fn apply_frame_projection_called_from_single_runtime_location() {
    let files = scan_rs_files("src");
    let hits = find_receiver_calls(&files, COMPOSITOR_APPLY_PROJECTION_RECEIVERS);

    // Exclude: src/compositor/ (defines apply_frame_projection), src/tests/
    let caller_hits: Vec<_> = hits
        .iter()
        .filter(|(path, _, _)| !path.contains("src/compositor/") && !path.contains("src/tests/"))
        .collect();

    let runtime_hits: Vec<_> = caller_hits
        .iter()
        .filter(|(path, _, _)| path.contains("frame_projection_binding_registry.rs"))
        .collect();
    let non_runtime_hits: Vec<_> = caller_hits
        .iter()
        .filter(|(path, _, _)| !path.contains("frame_projection_binding_registry.rs"))
        .collect();

    assert!(
        !runtime_hits.is_empty(),
        "SPEC-HAL-248: Expected compositor.apply_frame_projection() in runtime glue"
    );

    if !non_runtime_hits.is_empty() {
        let msgs: Vec<String> = non_runtime_hits
            .iter()
            .map(|(p, l, t)| format!("{}:{}: {}", p, l, t.trim()))
            .collect();
        panic!(
            "\n\nFP-248-08 VIOLATION: compositor.apply_frame_projection() called outside runtime glue\n\n{}\n\n\
             Compositor::apply_frame_projection() must only be called from pull_navigation_frame_projection().\n\
             See: SPEC-HAL-248 §3, §6 FP-248-08\n",
            msgs.join("\n")
        );
    }
}

#[test]
fn set_projected_view_called_from_single_runtime_location() {
    let files = scan_rs_files("src");
    let hits = find_receiver_calls(&files, SCREEN_HOST_SET_PROJECTED_VIEW_RECEIVERS);

    // Exclude: screen_host.rs (defines set_projected_view), src/tests/
    let caller_hits: Vec<_> = hits
        .iter()
        .filter(|(path, _, _)| !path.contains("screen_host.rs") && !path.contains("src/tests/"))
        .collect();

    let runtime_hits: Vec<_> = caller_hits
        .iter()
        .filter(|(path, _, _)| path.contains("frame_projection_binding_registry.rs"))
        .collect();
    let non_runtime_hits: Vec<_> = caller_hits
        .iter()
        .filter(|(path, _, _)| !path.contains("frame_projection_binding_registry.rs"))
        .collect();

    assert!(
        !runtime_hits.is_empty(),
        "SPEC-HAL-248: Expected ScreenHost publication in runtime glue"
    );

    if !non_runtime_hits.is_empty() {
        let msgs: Vec<String> = non_runtime_hits
            .iter()
            .map(|(p, l, t)| format!("{}:{}: {}", p, l, t.trim()))
            .collect();
        panic!(
            "\n\nFP-248-08 VIOLATION: set_projected_view() called outside runtime glue\n\n{}\n\n\
             ScreenHost::set_projected_view() must only be called from pull_navigation_frame_projection().\n\
             See: SPEC-HAL-248 §3, §6 FP-248-08\n",
            msgs.join("\n")
        );
    }
}

#[test]
fn runtime_frame_loops_pull_navigation_frame_projection() {
    let app = [
        "src/core/app.rs",
        "src/core/app/ui_loop.rs",
        "src/core/app/app_root.rs",
        "src/core/app/run.rs",
    ]
    .into_iter()
    .map(|path| {
        std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("Failed to read {path}: {error}"))
    })
    .collect::<Vec<_>>()
    .join("\n");
    let constructor_sites: Vec<_> = app
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            !trimmed.starts_with("//")
                && trimmed
                    .contains("NavigationSurfaceOrchestrator::new_with_existing_root_surface(")
        })
        .map(|(idx, line)| format!("{}: {}", idx + 1, line.trim()))
        .collect();
    let pull_sites: Vec<_> = app
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            !trimmed.starts_with("//") && trimmed.contains("pull_navigation_frame_projection(")
        })
        .map(|(idx, line)| format!("{}: {}", idx + 1, line.trim()))
        .collect();

    assert_eq!(
        constructor_sites.len(),
        2,
        "SPEC-HAL-248: App::run and run_ui_loop must each construct a NavigationSurfaceOrchestrator using the existing base surface.\n{}",
        constructor_sites.join("\n")
    );
    assert_eq!(
        pull_sites.len(),
        2,
        "SPEC-HAL-248: App::run and run_ui_loop must each call pull_navigation_frame_projection().\n{}",
        pull_sites.join("\n")
    );
}

// ============================================================================
// Guard 5 validation: Prove receiver-anchored matching does NOT false-positive
// on unrelated .project() / .apply_frame_projection() calls.
//
// This test contains strings that look like Rust source code. They are not
// mocked, simulated, or abstracted. They are literal text lines fed through
// the same matching function the guards use, proving that unrelated APIs with
// similar method names do not trigger the guard. The positive assertions at
// the end prove that actual orchestrator/compositor/ScreenHost patterns DO
// match.
//
// Removing this test weakens trust in Guard 5. Without it, there is no proof
// that the receiver-anchored matcher discriminates between navigation pipeline
// calls and unrelated method calls that happen to share a name.
// ============================================================================

#[test]
fn guard5_does_not_false_positive_on_unrelated_project() {
    // Synthetic source lines that contain .project() but are NOT orchestrator calls.
    // These must NOT match the receiver-anchored patterns.
    let unrelated_lines = &[
        "let result = iterator.project(|x| x.name);",
        "self.camera.project(point);",
        "matrix.project(vec3);",
        "let p = some_struct.project();",
        "config.project(\"name\");",
        "builder.apply_projection_output(data);",
        "combo.apply_projection_output(output);",
        "widget.set_projected_value(v);",
    ];

    for line in unrelated_lines {
        assert!(
            !line_matches_any_receiver(line, ORCHESTRATOR_PROJECT_RECEIVERS),
            "False positive on orchestrator receiver for: {}",
            line
        );
        assert!(
            !line_matches_any_receiver(line, COMPOSITOR_APPLY_PROJECTION_RECEIVERS),
            "False positive on compositor receiver for: {}",
            line
        );
        assert!(
            !line_matches_any_receiver(line, SCREEN_HOST_SET_PROJECTED_VIEW_RECEIVERS),
            "False positive on ScreenHost receiver for: {}",
            line
        );
    }

    // These SHOULD match (positive validation of the actual patterns)
    assert!(line_matches_any_receiver(
        "    let proj = orchestrator.project();",
        ORCHESTRATOR_PROJECT_RECEIVERS
    ));
    assert!(line_matches_any_receiver(
        "    let view = compositor.apply_frame_projection(",
        COMPOSITOR_APPLY_PROJECTION_RECEIVERS
    ));
    assert!(line_matches_any_receiver(
        "            sh.set_projected_view(view);",
        SCREEN_HOST_SET_PROJECTED_VIEW_RECEIVERS
    ));
}

// ============================================================================
// Guard 6: No cached projection state anywhere in src/
// (FP-248-03, FP-248-07)
// ============================================================================

#[test]
fn no_cached_projection_state() {
    let files = scan_rs_files("src");
    let mut violations = Vec::new();

    // Patterns that would indicate cached/retained projection state
    let forbidden = &[
        ("previous_projection", "Cached projection state (FP-248-03)"),
        ("last_projection", "Cached projection state (FP-248-03)"),
        ("cached_projection", "Cached projection state (FP-248-03)"),
        ("prev_projected_view", "Cached projection state (FP-248-03)"),
        ("last_projected_view", "Cached projection state (FP-248-03)"),
        (
            "cached_projected_view",
            "Cached projection state (FP-248-03)",
        ),
        ("projection_cache", "Cached projection state (FP-248-03)"),
        ("projection_history", "Projection history (FP-248-07)"),
    ];

    for (path, content) in &files {
        if path.contains("src/tests/") {
            continue;
        }
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
                continue;
            }
            for (pattern, reason) in forbidden {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{}: Found '{}'\n  Reason: {}",
                        path,
                        line_num + 1,
                        pattern,
                        reason
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\nFP-248-03 VIOLATION: Cached projection state detected\n\n{}\n\n\
             Projection must not be cached across frames. Each frame pulls fresh.\n\
             See: SPEC-HAL-248 §6 FP-248-03, FP-248-07\n",
            violations.join("\n\n")
        );
    }
}
