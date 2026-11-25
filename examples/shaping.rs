//! SPEC-HAL-050 native-script shaping validation
//!
//! Checks the shipped samples with the local Noto font fixtures.
//!
//! Usage:
//!   cargo run --example shaping -- [font]
//!
//! Font options:
//!   arabic     - NotoSansArabic-Regular.ttf (default)
//!   hebrew     - NotoSansHebrew-Regular.ttf
//!   devanagari - NotoSansDevanagari-Regular.ttf
//!   latin      - NotoSans-Regular.ttf
//!
//! Each sample must produce at least one glyph and no glyph ID zero.

use halcyon::core::text_shaper::{ShapedCluster, TextDirection, TextShaper};
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let font_choice = args.get(1).map(|s| s.as_str()).unwrap_or("arabic");

    let (font_path, font_name, test_texts) = match font_choice {
        "arabic" => (
            "assets/fonts/Noto/NotoSansArabic-Regular.ttf",
            "NotoSansArabic-Regular",
            vec![
                ("مرحبا", "Hello"),
                ("بالعالم", "World"),
                ("السلام عليكم", "Peace be upon you"),
                ("العربية", "Arabic"),
            ],
        ),
        "hebrew" => (
            "assets/fonts/Noto/NotoSansHebrew-Regular.ttf",
            "NotoSansHebrew-Regular",
            vec![
                ("שלום", "Hello"),
                ("עולם", "World"),
                ("עברית", "Hebrew"),
                ("מה שלומך", "How are you"),
            ],
        ),
        "devanagari" => (
            "assets/fonts/Noto/NotoSansDevanagari-Regular.ttf",
            "NotoSansDevanagari-Regular",
            vec![
                ("नमस्ते", "Hello"),
                ("हिंदी", "Hindi"),
                ("देवनागरी", "Devanagari"),
                ("भारत", "India"),
            ],
        ),
        "latin" => (
            "assets/fonts/Noto/NotoSans-Regular.ttf",
            "NotoSans-Regular",
            vec![
                ("Hello", "Hello"),
                ("World", "World"),
                ("office", "with fi ligature"),
                ("waffle", "with ff+fl ligatures"),
            ],
        ),
        _ => {
            eprintln!(
                "Unknown font: {}. Use: arabic, hebrew, devanagari, latin",
                font_choice
            );
            return ExitCode::FAILURE;
        }
    };

    // Load font from file
    let font_data = match std::fs::read(font_path) {
        Ok(data) => Arc::new(data),
        Err(error) => {
            eprintln!("Failed to load font {}: {}", font_path, error);
            return ExitCode::FAILURE;
        }
    };
    let Some(shaper) = TextShaper::from_data(font_data) else {
        eprintln!("Failed to create TextShaper from {}", font_path);
        return ExitCode::FAILURE;
    };

    println!("═══════════════════════════════════════════════════════════════");
    println!("  SPEC-HAL-050 native-script shaping validation");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Font: {}", font_name);
    println!("Path: {}", font_path);
    println!("Units per em: {}", shaper.units_per_em());
    println!();

    let mut all_valid = true;

    for (text, description) in test_texts {
        println!("───────────────────────────────────────────────────────────────");
        println!("Test: {} ({})", description, text);
        println!("───────────────────────────────────────────────────────────────");

        let direction = TextDirection::detect(text);
        let clusters = shaper.shape(text, 16.0, 1.0, direction);

        let valid = print_shaping_result(text, direction, &clusters);
        if !valid {
            all_valid = false;
        }
        println!();
    }

    println!("═══════════════════════════════════════════════════════════════");
    let status = if all_valid {
        println!("  ✓ All shaping tests PASSED - nonzero glyph IDs");
        ExitCode::SUCCESS
    } else {
        println!("  ✗ Some tests FAILED - glyph_id=0 indicates missing glyphs");
        ExitCode::FAILURE
    };
    println!("═══════════════════════════════════════════════════════════════");
    status
}

fn print_shaping_result(text: &str, direction: TextDirection, clusters: &[ShapedCluster]) -> bool {
    println!("Input text: \"{}\"", text);
    println!("Codepoints: {}", text.chars().count());
    println!("Direction:  {:?}", direction);
    println!("Glyphs:     {}", clusters.len());

    if clusters.is_empty() {
        println!("ERROR: No glyphs produced!");
        return false;
    }

    let total_advance: f32 = clusters.iter().map(|c| c.advance_phys).sum();
    println!("Total advance: {:.2}px @ 16px font size", total_advance);

    // Check for glyph_id = 0 (missing glyph / .notdef)
    let missing_count = clusters.iter().filter(|c| c.glyph_id == 0).count();

    println!();
    println!("Glyph details:");
    println!(
        "  {:>4}  {:>6}  {:>8}  {:>8}  {:>8}  {}",
        "idx", "glyph", "x_off", "y_off", "advance", "cluster"
    );
    println!(
        "  {:─>4}  {:─>6}  {:─>8}  {:─>8}  {:─>8}  {:─>7}",
        "", "", "", "", "", ""
    );

    for (i, c) in clusters.iter().enumerate() {
        let glyph_marker = if c.glyph_id == 0 { "⚠" } else { " " };
        println!(
            "  {:>4}  {:>5}{} {:>8.2}  {:>8.2}  {:>8.2}  {:>7}",
            i,
            c.glyph_id,
            glyph_marker,
            c.x_offset_phys,
            c.y_offset_phys,
            c.advance_phys,
            c.cluster
        );
    }

    if missing_count > 0 {
        println!();
        println!(
            "ERROR: {} glyphs with glyph_id=0 (missing/unsupported)",
            missing_count
        );
        return false;
    }

    true
}
