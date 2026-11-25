//! SPEC-HAL-192: ListView Closed-Composite Lock — Source Scan Enforcement
//!
//! This test scans ListView source code for forbidden SPEC-HAL-190 API usage.
//! If any forbidden pattern is found, the test fails.
//!
//! Forbidden patterns:
//! - `cx.authority<` — ListView owns its substrate; no ancestor lookup needed
//! - `cx.emit_intent` — ListView calls self.substrate.handle_intent() directly
//! - `impl AuthorityOwner for ListView` — ListView is not an authority provider
//! - `AuthorityOwner` token in non-comment code — catch any sneaky patterns
//!
//! This is test-time enforcement. Compile-fail tests (trybuild) provide
//! additional enforcement for impl attempts.

use std::fs;
use std::path::Path;

/// Forbidden API patterns per SPEC-HAL-192.
const FORBIDDEN_PATTERNS: &[(&str, &str)] = &[
    (
        "cx.authority<",
        "ListView must not use cx.authority<T>() — owns substrate directly",
    ),
    (
        "cx.emit_intent",
        "ListView must not use cx.emit_intent() — calls substrate.handle_intent()",
    ),
    (
        "impl AuthorityOwner for ListView",
        "ListView must not implement AuthorityOwner — closed composite",
    ),
];

/// Additional token to detect (must not appear outside comments).
const FORBIDDEN_TOKEN: &str = "AuthorityOwner";

/// The single file to scan.
const LISTVIEW_SOURCE_FILE: &str = "src/widgets/list_view/list_view.rs";

#[test]
fn listview_has_no_forbidden_SPEC_hal_190_apis() {
    let path = Path::new(LISTVIEW_SOURCE_FILE);
    assert!(
        path.exists(),
        "SPEC-HAL-192: ListView source file not found: {}",
        LISTVIEW_SOURCE_FILE
    );

    let content = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "SPEC-HAL-192: Failed to read {}: {}",
            LISTVIEW_SOURCE_FILE, e
        );
    });

    let mut violations = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comment lines (SPEC-HAL-192 header mentions forbidden patterns)
        if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
            continue;
        }

        // Check forbidden patterns
        for (pattern, reason) in FORBIDDEN_PATTERNS {
            if line.contains(pattern) {
                violations.push(format!(
                    "{}:{}: Found '{}'\n  Reason: {}",
                    LISTVIEW_SOURCE_FILE,
                    line_num + 1,
                    pattern,
                    reason
                ));
            }
        }

        // Check forbidden token (AuthorityOwner) in non-comment code
        if line.contains(FORBIDDEN_TOKEN) {
            violations.push(format!(
                "{}:{}: Found '{}' token in code\n  Reason: ListView must not reference AuthorityOwner — closed composite",
                LISTVIEW_SOURCE_FILE,
                line_num + 1,
                FORBIDDEN_TOKEN
            ));
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\nSPEC-HAL-192 VIOLATION: ListView contains forbidden SPEC-HAL-190 APIs\n\n{}\n\n\
             ListView is a closed composite. It must use direct substrate access only.\n\
             See: docs/SPECs/SPEC-HAL-192-ListView-Closed-Composite-Lock.md\n",
            violations.join("\n\n")
        );
    }
}
