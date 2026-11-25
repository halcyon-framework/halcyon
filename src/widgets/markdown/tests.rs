use super::*;
use std::sync::Arc;

fn parse(source: &str) -> Arc<MarkdownDocument> {
    MarkdownDocument::parse(source.into(), MarkdownLimits::default()).unwrap()
}
#[test]
fn markdown_source_decoding_projection_and_code_are_distinct() {
    let source = "# H &amp; \\*\r\n\r\n**bold** *italic* ~~strike~~ `a\tb`  \r\nnext\r\nsoft\r\n\r\n```rs\r\n\tlet x = 1;\r\n```\r\n\r\n<div>inert &amp;</div>\r\n";
    let doc = parse(source);
    assert_eq!(doc.source().as_bytes(), source.as_bytes());
    assert_eq!(
        doc.reading_text(),
        "H & *\n\nbold italic strike a\tb\nnext soft\n\n\tlet x = 1;\n\n\n<div>inert &amp;</div>\r\n"
    );
    assert_eq!(doc.code_block_count(), 1);
    assert_eq!(doc.code_literal(0), Some("\tlet x = 1;\n"));
    assert_eq!(doc.code_literal(1), None);
    assert!(!format!("{doc:?}").contains("let x"));
    assert!(!format!("{doc:?}").contains("inert"));
}
#[test]
fn markdown_lists_tables_images_links_and_unclosed_fence() {
    let doc = parse(
        "3. alpha\n4. beta\n   - [x] nested\n   - [ ] wait\n\n> quote\n>\n> second\n\n| A | B |\n| :- | -: |\n| x | y |\n\n![alt *text*](file:never-read) [label](javascript:never-run)\n\n---\n\n```raw\nbody\tend",
    );
    assert_eq!(
        doc.reading_text(),
        "3. alpha\n4. beta\n\n  - [x] nested\n  - [ ] wait\n\nquote\n\nsecond\n\nA\tB\nx\ty\n\n[image not loaded: alt text] label\n\nbody\tend"
    );
    assert_eq!(doc.code_literal(0), Some("body\tend"));
    assert_eq!(parse("").reading_text(), "");
    assert_eq!(parse("---\n").reading_text(), "");
    assert_eq!(
        parse("before\n\n---\n\nafter").reading_text(),
        "before\n\nafter"
    );
    assert_eq!(
        parse("    x\r\n    \ty\r\n").code_literal(0),
        Some("x\n\ty\n")
    );
}
#[test]
fn markdown_limits_cancellation_and_revision_identity() {
    let default = MarkdownLimits::default();
    assert_eq!(
        MarkdownDocument::parse(
            "x".into(),
            MarkdownLimits {
                max_depth: 0,
                ..default
            }
        )
        .unwrap_err(),
        MarkdownPrepareError::InvalidLimits
    );
    for (source, limits, kind) in [
        (
            "four",
            MarkdownLimits {
                max_source_bytes: 3,
                ..default
            },
            MarkdownLimitKind::SourceBytes,
        ),
        (
            "![alt](inert)",
            MarkdownLimits {
                max_reading_bytes: 2,
                ..default
            },
            MarkdownLimitKind::ReadingBytes,
        ),
        (
            "*x*",
            MarkdownLimits {
                max_nodes: 2,
                ..default
            },
            MarkdownLimitKind::Nodes,
        ),
        (
            "> > nested",
            MarkdownLimits {
                max_depth: 1,
                ..default
            },
            MarkdownLimitKind::Depth,
        ),
        (
            "|a|b|\n|-|-|\n|c|d|",
            MarkdownLimits {
                max_table_columns: 1,
                ..default
            },
            MarkdownLimitKind::TableColumns,
        ),
    ] {
        assert!(
            matches!(MarkdownDocument::parse(source.into(),limits),Err(MarkdownPrepareError::LimitExceeded {kind:k,..}) if k==kind)
        );
    }
    for checkpoint in [0, 1, 3, 7] {
        let mut calls = 0;
        let result =
            MarkdownDocument::parse_with_cancel("**one** and *two*".into(), default, || {
                calls += 1;
                calls > checkpoint
            });
        assert!(matches!(result, Err(MarkdownPrepareError::Canceled)));
    }
    let first = parse("same");
    let second = parse("same");
    assert!(!Arc::ptr_eq(&first, &second));
    let style = MarkdownStyle::new("Arial", "Courier New").unwrap();
    let view = MarkdownView::new(first.clone(), style.clone());
    assert!(Arc::ptr_eq(view.document(), &first));
    let replacement = MarkdownView::new(second.clone(), style);
    assert!(Arc::ptr_eq(replacement.document(), &second));
}
#[test]
fn markdown_checked_styles_and_thread_safe_document() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<MarkdownDocument>();
    for name in ["", " ", "bad\nfamily"] {
        assert_eq!(
            MarkdownStyle::new(name, "code").unwrap_err(),
            MarkdownStyleError::InvalidFamily
        );
    }
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            MarkdownStyle::new("Arial", "Courier New")
                .unwrap()
                .with_size(size)
                .unwrap_err(),
            MarkdownStyleError::InvalidFontSize
        );
    }
    let doc = parse("\0 &amp; \\* `  one\r\ntwo  `");
    assert_eq!(doc.source(), "\0 &amp; \\* `  one\r\ntwo  `");
    let selected = pulldown_cmark::Parser::new_ext(
        doc.source(),
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    )
    .filter_map(|event| match event {
        pulldown_cmark::Event::Text(s) | pulldown_cmark::Event::Code(s) => Some(s.into_string()),
        _ => None,
    })
    .collect::<String>();
    assert_eq!(
        doc.reading_text(),
        selected,
        "selected parser decoding, with original NUL retained in source"
    );
    assert!(doc.reading_text().contains("& *"));
}
