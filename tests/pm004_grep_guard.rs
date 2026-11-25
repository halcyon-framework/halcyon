/// SPEC-PM-004 Grep Guard: Ensures `add_rounded_rect` and `add_rounded_border`
/// keep one scroll owner for their active rendering pipeline.
///
/// SPEC-R-022 routes rounded geometry through `PrimitiveData::SdfRect`, not the
/// path pipeline. The SDF branch emits scroll-adjusted params exactly once.

const BATCHER_SRC: &str = include_str!("../src/renderer/batcher.rs");

/// Extract the body text between a function start marker and the next `pub fn` or end of file.
fn extract_fn_body(source: &str, fn_name: &str) -> String {
    let marker = format!("fn {fn_name}");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("Could not find `{marker}` in batcher.rs"));
    let rest = &source[start..];
    let body_end = rest[1..]
        .find("\n    pub fn")
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    rest[..body_end].to_string()
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

#[test]
fn test_pm004_grep_guard_rounded_rect_sdf_scroll_owner() {
    let body = extract_fn_body(BATCHER_SRC, "add_rounded_rect");

    assert!(body.contains("PrimitiveData::SdfRect"));
    assert_eq!(
        count_occurrences(&body, "apply_scroll_to_rect("),
        1,
        "PM-004 grep guard: add_rounded_rect SDF branch must apply scroll exactly once"
    );
    assert!(
        !body.contains("add_path("),
        "PM-004 grep guard: add_rounded_rect must not route SDF rounded geometry through add_path"
    );
    assert!(
        !body.contains("Path::rounded_rect"),
        "PM-004 grep guard: add_rounded_rect must not CPU-tessellate rounded geometry"
    );
    assert!(
        !body.contains("current_scroll()"),
        "PM-004 grep guard: add_rounded_rect must not call current_scroll()"
    );
}

#[test]
fn test_pm004_grep_guard_rounded_border_sdf_scroll_owner() {
    let body = extract_fn_body(BATCHER_SRC, "add_rounded_border");

    assert!(body.contains("PrimitiveData::SdfRect"));
    assert_eq!(
        count_occurrences(&body, "apply_scroll_to_rect("),
        1,
        "PM-004 grep guard: add_rounded_border SDF branch must apply scroll exactly once"
    );
    assert!(
        !body.contains("add_path("),
        "PM-004 grep guard: add_rounded_border must not route SDF rounded geometry through add_path"
    );
    assert!(
        !body.contains("Path::rounded_rect"),
        "PM-004 grep guard: add_rounded_border must not CPU-tessellate rounded geometry"
    );
    assert!(
        !body.contains("current_scroll()"),
        "PM-004 grep guard: add_rounded_border must not call current_scroll()"
    );
}
