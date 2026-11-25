//! Test scaffolding only — not examples, not demos, not production widgets.
//!
//! This module contains shared test infrastructure (probe widgets, mock authorities, etc.)
//! used by multiple test modules. These are implementation details of the test suite.

pub(crate) mod authority_probe_demo;

use crate::core::paint::Color;
use crate::core::scene::{SceneNode, TextBackgroundSpan};
use crate::core::theme::Theme;

pub(crate) fn text_selection_palette_cases() -> [(Theme, Color); 6] {
    let mut muted = Theme::light();
    muted.colors.accents.muted = Color::from_hex(0xA03974).with_alpha(0.13);
    muted.focus_ring.capture_color = Color::from_hex(0x13A67F);

    let mut capture = Theme::dark();
    capture.focus_ring.capture_color = Color::from_hex(0x13A67F).with_alpha(0.2);

    let mut alpha = Theme::light();
    alpha.typography.selection_alpha = 0.62;

    let mut muted_alpha_only = Theme::light();
    muted_alpha_only.colors.accents.muted.a = 0.13;

    [
        (Theme::light(), Color::from_hex(0x1D7DFC).with_alpha(0.35)),
        (Theme::dark(), Color::from_hex(0x4B9AFF).with_alpha(0.35)),
        (muted, Color::from_hex(0xA03974).with_alpha(0.35)),
        (capture, Color::from_hex(0x13A67F).with_alpha(0.35)),
        (alpha, Color::from_hex(0x1D7DFC).with_alpha(0.62)),
        (muted_alpha_only, Color::from_hex(0x647781).with_alpha(0.35)),
    ]
}

pub(crate) fn assert_painted_text_backgrounds(nodes: &[SceneNode], text: &str, expected: &[Color]) {
    fn collect<'a>(nodes: &'a [SceneNode], text: &str, backgrounds: &mut Vec<&'a TextBackgroundSpan>) {
        for node in nodes {
            match node {
                SceneNode::Text(run) if run.text == text => backgrounds.extend(&run.backgrounds),
                SceneNode::RetainedFragment(node) => collect(&node.children, text, backgrounds),
                SceneNode::Clip(node) => collect(&node.children, text, backgrounds),
                SceneNode::Layer(node) => collect(&node.children, text, backgrounds),
                SceneNode::Scroll(node) => collect(&node.children, text, backgrounds),
                _ => {}
            }
        }
    }

    let mut backgrounds = Vec::new();
    collect(nodes, text, &mut backgrounds);
    let actual: Vec<_> = backgrounds.iter().map(|span| span.color).collect();
    let expected: Vec<_> = expected.iter().copied().map(crate::core::scene::Color::from).collect();
    assert_eq!(
        actual, expected,
        "text background colors for {text:?}",
    );
    assert!(
        backgrounds.iter().all(|span| span.width > 0.0 && span.height > 0.0),
        "each background must cover part of the text run for {text:?}",
    );
}

#[cfg(target_os = "macos")]
pub(crate) fn markdown_fonts() -> Vec<crate::platform::font_manager::FontAsset> {
    use crate::platform::font_manager::{FontAsset, FontManager, FontWeight};

    let mut manager = FontManager::new();
    let mut assets = Vec::new();
    for (file, family, weight, italic) in [
        ("Arial.ttf", "Arial", FontWeight::Regular, false),
        ("Arial Bold.ttf", "Arial", FontWeight::Bold, false),
        ("Arial Italic.ttf", "Arial", FontWeight::Regular, true),
        ("Arial Bold Italic.ttf", "Arial", FontWeight::Bold, true),
        ("Courier New.ttf", "Courier New", FontWeight::Regular, false),
        (
            "Courier New Bold.ttf",
            "Courier New",
            FontWeight::Bold,
            false,
        ),
        (
            "Courier New Italic.ttf",
            "Courier New",
            FontWeight::Regular,
            true,
        ),
        (
            "Courier New Bold Italic.ttf",
            "Courier New",
            FontWeight::Bold,
            true,
        ),
        (
            "Arial Unicode.ttf",
            "Arial Unicode MS",
            FontWeight::Regular,
            false,
        ),
    ] {
        let id = manager
            .load_font_file(&format!("/System/Library/Fonts/Supplemental/{file}"))
            .unwrap();
        let face = manager.get_face(id).unwrap();
        let parsed = ttf_parser::Face::parse(&face.data, 0).unwrap();
        let actual_family = parsed
            .names()
            .into_iter()
            .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
            .find_map(|n| n.to_string())
            .expect("actual Unicode family metadata");
        assert_eq!(
            (
                actual_family.as_str(),
                FontWeight::from_value(parsed.weight().to_number()),
                parsed.is_italic()
            ),
            (family, weight, italic)
        );
        assets.push(FontAsset {
            family_name: actual_family,
            weight: face.weight,
            italic: face.italic,
            data: face.data.clone(),
        });
    }
    assets
}
