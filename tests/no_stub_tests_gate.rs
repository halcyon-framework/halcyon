use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_PLACEHOLDER_MACRO: &str = concat!("todo", "!(");
const FORBIDDEN_UNIMPLEMENTED_MACRO: &str = concat!("unimplemented", "!(");
const FORBIDDEN_PLACEHOLDER_PANIC: &str = concat!("panic!(\"", "todo");

#[derive(Debug)]
struct StubViolation {
    path: PathBuf,
    line: usize,
    reason: &'static str,
}

#[test]
fn integration_tests_have_no_stub_bodies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let violations = scan_test_tree(&root);

    assert!(violations.is_empty(), "{}", format_violations(&violations));
}

fn scan_test_tree(root: &Path) -> Vec<StubViolation> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();

    let mut violations = Vec::new();

    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        violations.extend(scan_source(&path, &source));
    }

    violations
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()));

    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read directory entry: {error}"))
            .path();

        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn scan_source(path: &Path, source: &str) -> Vec<StubViolation> {
    let mut violations = Vec::new();
    let mut test_attribute_pending = false;
    let mut test_function_pending = false;
    let mut in_test_body = false;
    let mut body_start_line = 0;
    let mut body_has_statement = false;
    let mut brace_depth = 0isize;

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed = line.trim();

        if in_test_body {
            if let Some(reason) = forbidden_body_line_reason(line) {
                violations.push(StubViolation {
                    path: path.to_path_buf(),
                    line: line_number,
                    reason,
                });
            }

            if body_line_has_statement(line) {
                body_has_statement = true;
            }

            brace_depth += brace_delta(line);

            if brace_depth == 0 {
                if !body_has_statement {
                    violations.push(StubViolation {
                        path: path.to_path_buf(),
                        line: body_start_line,
                        reason: "test body has no executable statement",
                    });
                }

                in_test_body = false;
                test_attribute_pending = false;
                test_function_pending = false;
            }

            continue;
        }

        if trimmed.starts_with("#[test") {
            test_attribute_pending = true;
            test_function_pending = false;
            continue;
        }

        if !test_attribute_pending {
            continue;
        }

        if trimmed.is_empty() || trimmed.starts_with("#[") || trimmed.starts_with("//") {
            continue;
        }

        if trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.contains(" fn ")
        {
            test_function_pending = true;
        }

        if test_function_pending && first_code_open_brace(line).is_some() {
            in_test_body = true;
            body_start_line = line_number;
            body_has_statement = body_line_has_statement(after_first_open_brace(line));
            brace_depth = brace_delta(line);

            if let Some(reason) = forbidden_body_line_reason(line) {
                violations.push(StubViolation {
                    path: path.to_path_buf(),
                    line: line_number,
                    reason,
                });
            }

            if brace_depth == 0 {
                if !body_has_statement {
                    violations.push(StubViolation {
                        path: path.to_path_buf(),
                        line: body_start_line,
                        reason: "test body has no executable statement",
                    });
                }

                in_test_body = false;
                test_attribute_pending = false;
                test_function_pending = false;
            }
        }
    }

    violations
}

fn forbidden_body_line_reason(line: &str) -> Option<&'static str> {
    let lower = line.to_ascii_lowercase();

    if contains_placeholder_word(line) {
        return Some("placeholder marker inside test body");
    }

    if lower.contains(FORBIDDEN_PLACEHOLDER_MACRO) {
        return Some("placeholder macro inside test body");
    }

    if lower.contains(FORBIDDEN_UNIMPLEMENTED_MACRO) {
        return Some("unimplemented macro inside test body");
    }

    if lower.contains(FORBIDDEN_PLACEHOLDER_PANIC) {
        return Some("placeholder panic inside test body");
    }

    None
}

fn contains_placeholder_word(line: &str) -> bool {
    line.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token.eq_ignore_ascii_case("todo"))
}

fn body_line_has_statement(line: &str) -> bool {
    let code = code_without_literals_and_comments(line);
    let code = code.replace(['{', '}'], "");
    !code.trim().is_empty()
}

fn brace_delta(line: &str) -> isize {
    code_without_literals_and_comments(line)
        .chars()
        .fold(0, |delta, ch| match ch {
            '{' => delta + 1,
            '}' => delta - 1,
            _ => delta,
        })
}

fn first_code_open_brace(line: &str) -> Option<usize> {
    code_without_literals_and_comments(line).find('{')
}

fn after_first_open_brace(line: &str) -> &str {
    line.split_once('{')
        .map(|(_, after_open)| after_open)
        .unwrap_or("")
}

fn code_without_literals_and_comments(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut output = String::with_capacity(line.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            break;
        }

        if bytes[index] == b'r' {
            let mut hash_index = index + 1;
            while bytes.get(hash_index) == Some(&b'#') {
                hash_index += 1;
            }

            if bytes.get(hash_index) == Some(&b'"') {
                output.push(' ');
                index = skip_raw_string(bytes, hash_index + 1, hash_index - index - 1);
                continue;
            }
        }

        if bytes[index] == b'"' {
            output.push(' ');
            index = skip_quoted(bytes, index + 1, b'"');
            continue;
        }

        if bytes[index] == b'\'' {
            output.push(' ');
            index = skip_quoted(bytes, index + 1, b'\'');
            continue;
        }

        output.push(bytes[index] as char);
        index += 1;
    }

    output
}

fn skip_quoted(bytes: &[u8], mut index: usize, terminator: u8) -> usize {
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
        } else if bytes[index] == terminator {
            return index + 1;
        } else {
            index += 1;
        }
    }

    index
}

fn skip_raw_string(bytes: &[u8], mut index: usize, hash_count: usize) -> usize {
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let mut matched = true;
            for offset in 0..hash_count {
                if bytes.get(index + 1 + offset) != Some(&b'#') {
                    matched = false;
                    break;
                }
            }

            if matched {
                return index + 1 + hash_count;
            }
        }

        index += 1;
    }

    index
}

fn format_violations(violations: &[StubViolation]) -> String {
    let mut output = String::from("integration test stub violations:\n");

    for violation in violations {
        output.push_str(&format!(
            "{}:{}: {}\n",
            violation.path.display(),
            violation.line,
            violation.reason
        ));
    }

    output
}
