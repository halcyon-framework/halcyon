//! SPEC-HAL-065: Unified Text Shaping via rustybuzz
//!
//! Platform-agnostic text shaping engine that provides:
//! - Complete Unicode-correct shaping
//! - RTL/LTR direction handling
//! - Ligatures and contextual forms
//! - Cluster mapping
//! - Complex scripts (Arabic, Hebrew, Indic, Thai, etc.)
//! - Emoji sequences (ZWJ)
//!
//! This module is the ONLY shaping engine in Halcyon. Native backends
//! (DirectWrite, CoreText, FreeType) perform rasterization only.

use rustybuzz::{Direction, Face, GlyphBuffer, UnicodeBuffer};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ClusterGap(f32);

impl ClusterGap {
    pub fn new(value: f32) -> Result<Self, TextInputError> {
        if !value.is_finite() || value < 0.0 {
            return Err(TextInputError::InvalidClusterGap);
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    pub fn value(self) -> f32 {
        self.0
    }
}

impl Eq for ClusterGap {}

impl std::hash::Hash for ClusterGap {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.0.to_bits(), state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputError {
    InvalidClusterGap,
    InvalidFontSize,
    InvalidScale,
    PositionOverflow,
    FontUnavailable,
    UnsupportedTypography,
}

impl std::fmt::Display for TextInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid or unavailable text input: {self:?}")
    }
}

impl std::error::Error for TextInputError {}

/// Ephemeral source mapping in visual order; positions exclude a terminal gap.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceCluster {
    pub source: std::ops::Range<usize>,
    pub x_start: f32,
    pub x_end: f32,
    pub direction: TextDirection,
}

#[derive(Debug)]
pub struct SpacedShape {
    pub glyphs: Vec<ShapedCluster>,
    pub clusters: Vec<SourceCluster>,
    pub width: f32,
}

/// Text direction for shaping.
///
/// Per SPEC-HAL-065, we only support horizontal text (no TTB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDirection {
    #[default]
    LeftToRight,
    RightToLeft,
}

impl TextDirection {
    /// Detect direction from the first strong directional character in text.
    ///
    /// Arabic/Hebrew → RTL, Latin/Cyrillic/Greek → LTR
    pub fn detect(text: &str) -> Self {
        for ch in text.chars() {
            match unicode_bidi_class(ch) {
                BidiClass::L => return TextDirection::LeftToRight,
                BidiClass::R | BidiClass::AL => return TextDirection::RightToLeft,
                _ => continue,
            }
        }
        TextDirection::LeftToRight
    }
}

/// Simplified Bidi class for direction detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BidiClass {
    L,     // Left-to-Right (Latin, Cyrillic, Greek, etc.)
    R,     // Right-to-Left (Hebrew)
    AL,    // Arabic Letter
    Other, // Everything else (numbers, punctuation, etc.)
}

/// Get the Unicode Bidi class of a character.
fn unicode_bidi_class(ch: char) -> BidiClass {
    // Simplified classification based on Unicode script blocks
    let cp = ch as u32;
    match cp {
        // Arabic block (0600-06FF) and Arabic Extended blocks
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            BidiClass::AL
        }
        // Hebrew block (0590-05FF) and Hebrew presentation forms
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => BidiClass::R,
        // Basic Latin, Latin Extended, Greek, Cyrillic (and more LTR scripts)
        0x0041..=0x005A | 0x0061..=0x007A | // Basic Latin letters
        0x00C0..=0x024F | // Latin Extended-A/B
        0x0370..=0x03FF | // Greek
        0x0400..=0x04FF | // Cyrillic
        0x0500..=0x052F | // Cyrillic Supplement
        0x1E00..=0x1EFF | // Latin Extended Additional
        0x2C60..=0x2C7F | // Latin Extended-C
        0xA720..=0xA7FF => BidiClass::L, // Latin Extended-D
        _ => BidiClass::Other,
    }
}

/// A shaped cluster output from the shaper.
///
/// Represents a single glyph with its position in physical pixel coordinates.
/// Multiple codepoints may map to a single cluster (ligatures) or a single
/// codepoint may produce multiple glyphs (decomposition).
///
/// All values are in physical pixels as f32 to preserve subpixel precision.
/// Integer truncation caused horizontal quantization artifacts (staircase pattern).
#[derive(Debug, Clone, Copy)]
pub struct ShapedCluster {
    /// Glyph ID from the font (for rasterization)
    pub glyph_id: u32,
    /// X offset from pen position in physical pixels (raw from HarfBuzz)
    pub x_offset_phys: f32,
    /// Y offset from baseline in physical pixels (raw from HarfBuzz)
    pub y_offset_phys: f32,
    /// Horizontal advance to next glyph in physical pixels (raw from HarfBuzz)
    pub advance_phys: f32,
    /// Original cluster index (byte offset in source text)
    pub cluster: u32,
    /// X offset in physical pixels (f32 to preserve subpixel precision)
    pub x_offset_px: f32,
    /// Y offset in physical pixels (f32 to preserve subpixel precision)
    pub y_offset_px: f32,
    /// Advance in physical pixels (f32 to preserve subpixel precision)
    pub advance_px: f32,
}

/// Unified text shaper using rustybuzz.
///
/// Constructed from font data (the same bytes used by rasterizers).
/// Thread-safe via `Send + Sync` (Face is lifetime-bound to owned data).
pub struct TextShaper {
    /// The rustybuzz Face for shaping.
    /// Lifetime is 'static because we own the font data via Arc.
    face: Face<'static>,
    /// Font units per em (for converting design units to pixels)
    units_per_em: u16,
    /// Owned font data (keeps Face valid)
    _font_data: Arc<Vec<u8>>,
}

impl std::fmt::Debug for TextShaper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextShaper")
            .field("units_per_em", &self.units_per_em)
            .finish()
    }
}

impl TextShaper {
    /// Create a new TextShaper from font data.
    ///
    /// # Arguments
    /// - `font_data`: Raw font file bytes (TTF/OTF)
    /// - `units_per_em`: Font's design units per em (from font metrics)
    ///
    /// # Returns
    /// `Some(TextShaper)` if the font data is valid, `None` otherwise.
    pub fn new(font_data: Arc<Vec<u8>>, units_per_em: u16) -> Option<Self> {
        // SAFETY: We keep _font_data alive for the lifetime of TextShaper,
        // so the Face reference remains valid.
        let data_ref: &'static [u8] = unsafe { std::mem::transmute(font_data.as_slice()) };

        let face = Face::from_slice(data_ref, 0)?;

        Some(Self {
            face,
            units_per_em,
            _font_data: font_data,
        })
    }

    /// Create a TextShaper from font data, inferring units_per_em from the font.
    ///
    /// Uses ttf-parser to read units_per_em from the font's head table.
    pub fn from_data(font_data: Arc<Vec<u8>>) -> Option<Self> {
        let face_info = ttf_parser::Face::parse(&font_data, 0).ok()?;
        let units_per_em = face_info.units_per_em();
        Self::new(font_data, units_per_em)
    }

    /// Get the font's units per em.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    pub(crate) fn font_data(&self) -> Arc<Vec<u8>> {
        self._font_data.clone()
    }

    /// Shape text into positioned glyph clusters.
    ///
    /// # Arguments
    /// - `text`: UTF-8 text to shape
    /// - `font_size`: Desired font size in logical pixels
    /// - `device_scale`: Device scale factor (e.g., 2.0 for Retina)
    /// - `direction`: Text direction (LTR or RTL)
    ///
    /// # Returns
    /// Vector of `ShapedCluster` with positions in physical pixels.
    ///
    /// # Coordinate System
    /// Positions are relative to the text run origin on the baseline:
    /// - `x_offset_phys`: horizontal offset from current pen position
    /// - `y_offset_phys`: vertical offset from baseline (positive = up)
    /// - `advance_phys`: horizontal distance to move pen after this glyph
    pub fn shape(
        &self,
        text: &str,
        font_size: f32,
        device_scale: f32,
        direction: TextDirection,
    ) -> Vec<ShapedCluster> {
        if text.is_empty() {
            return Vec::new();
        }

        // Build Unicode buffer with direction
        let mut buffer = UnicodeBuffer::new();
        buffer.set_direction(match direction {
            TextDirection::LeftToRight => Direction::LeftToRight,
            TextDirection::RightToLeft => Direction::RightToLeft,
        });
        buffer.push_str(text);

        // Run shaping
        let glyph_buffer: GlyphBuffer = rustybuzz::shape(&self.face, &[], buffer);

        // Conversion factor: design units → physical pixels
        // scale = (font_size * device_scale) / units_per_em
        let scale = (font_size * device_scale) / self.units_per_em as f32;

        // Convert glyph info to ShapedClusters
        let glyph_infos = glyph_buffer.glyph_infos();
        let glyph_positions = glyph_buffer.glyph_positions();

        let mut clusters = Vec::with_capacity(glyph_infos.len());

        for (info, pos) in glyph_infos.iter().zip(glyph_positions.iter()) {
            // Float values from HarfBuzz - keep as f32 to preserve subpixel precision
            let x_offset_phys = pos.x_offset as f32 * scale;
            let y_offset_phys = pos.y_offset as f32 * scale;
            let advance_phys = pos.x_advance as f32 * scale;

            // Keep as f32 - no integer truncation to avoid horizontal quantization
            let x_offset_px = x_offset_phys;
            let y_offset_px = y_offset_phys;
            let advance_px = advance_phys;

            clusters.push(ShapedCluster {
                glyph_id: info.glyph_id,
                x_offset_phys,
                y_offset_phys,
                advance_phys,
                cluster: info.cluster,
                x_offset_px,
                y_offset_px,
                advance_px,
            });
        }

        clusters
    }

    /// Shape text with automatic direction detection.
    ///
    /// Convenience method that detects direction from the text content.
    pub fn shape_auto(&self, text: &str, font_size: f32, device_scale: f32) -> Vec<ShapedCluster> {
        let direction = TextDirection::detect(text);
        self.shape(text, font_size, device_scale, direction)
    }

    /// Shape the whole string, then space consecutive visual source groups.
    pub fn shape_with_gap(
        &self,
        text: &str,
        font_size: f32,
        device_scale: f32,
        gap: ClusterGap,
    ) -> Result<SpacedShape, TextInputError> {
        self.shape_with_gap_direction(
            text,
            font_size,
            device_scale,
            gap,
            TextDirection::detect(text),
        )
    }

    /// The paragraph owner preserves its original span direction across face runs.
    pub(crate) fn shape_with_gap_direction(
        &self,
        text: &str,
        font_size: f32,
        device_scale: f32,
        gap: ClusterGap,
        direction: TextDirection,
    ) -> Result<SpacedShape, TextInputError> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(TextInputError::InvalidFontSize);
        }
        if !device_scale.is_finite() || device_scale <= 0.0 {
            return Err(TextInputError::InvalidScale);
        }
        if !(font_size * device_scale).is_finite()
            || !(gap.value() * device_scale).is_finite()
            || font_size * 100.0 > u32::MAX as f32
            || device_scale * 1000.0 > u32::MAX as f32
        {
            return Err(TextInputError::PositionOverflow);
        }
        let mut glyphs = self.shape(text, font_size, device_scale, direction);
        let starts: std::collections::BTreeSet<usize> =
            glyphs.iter().map(|glyph| glyph.cluster as usize).collect();
        let mut clusters = Vec::new();
        let mut pen = 0.0_f32;
        let mut first = 0;
        while first < glyphs.len() {
            let source_start = glyphs[first].cluster as usize;
            let mut end = first + 1;
            while end < glyphs.len() && glyphs[end].cluster == glyphs[first].cluster {
                end += 1;
            }
            let source_end = starts
                .range((source_start + 1)..)
                .next()
                .copied()
                .unwrap_or(text.len());
            let x_start = pen / device_scale;
            for glyph in &glyphs[first..end] {
                if ![
                    glyph.advance_phys,
                    glyph.x_offset_phys,
                    glyph.y_offset_phys,
                    pen + glyph.x_offset_phys,
                ]
                .iter()
                .all(|v| v.is_finite())
                {
                    return Err(TextInputError::PositionOverflow);
                }
                pen += glyph.advance_phys;
            }
            clusters.push(SourceCluster {
                source: source_start..source_end,
                x_start,
                x_end: pen / device_scale,
                direction,
            });
            if end < glyphs.len() {
                let physical_gap = gap.value() * device_scale;
                glyphs[end - 1].advance_phys += physical_gap;
                glyphs[end - 1].advance_px += physical_gap;
                pen += physical_gap;
            }
            if !pen.is_finite() || !(pen / device_scale).is_finite() {
                return Err(TextInputError::PositionOverflow);
            }
            first = end;
        }
        Ok(SpacedShape {
            glyphs,
            clusters,
            width: pen / device_scale,
        })
    }

    /// Calculate the total advance width of shaped text in physical pixels.
    pub fn measure_width(&self, text: &str, font_size: f32, device_scale: f32) -> f32 {
        let clusters = self.shape_auto(text, font_size, device_scale);
        clusters.iter().map(|c| c.advance_phys).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_typography_cluster_gap_preserves_shaping() {
        let shaper = TextShaper::from_data(Arc::new(
            include_bytes!("../../assets/fonts/Inter/InterVariable.ttf").to_vec(),
        ))
        .unwrap();
        for text in [
            "",
            "A",
            "AV",
            "office",
            "e\u{301}x",
            "Привет мир",
            "a  b",
            "שלום",
        ] {
            for scale in [1.0, 1.25, 2.0] {
                let plain = shaper.shape_auto(text, 24.0, scale);
                let zero = shaper
                    .shape_with_gap(text, 24.0, scale, ClusterGap::default())
                    .unwrap();
                let spaced = shaper
                    .shape_with_gap(text, 24.0, scale, ClusterGap::new(2.5).unwrap())
                    .unwrap();
                assert_eq!(plain.len(), zero.glyphs.len());
                for ((a, b), c) in plain.iter().zip(&zero.glyphs).zip(&spaced.glyphs) {
                    assert_eq!((a.glyph_id, a.cluster), (b.glyph_id, b.cluster));
                    assert_eq!((a.glyph_id, a.cluster), (c.glyph_id, c.cluster));
                    assert_eq!(
                        (a.x_offset_phys, a.y_offset_phys),
                        (c.x_offset_phys, c.y_offset_phys)
                    );
                    assert_eq!(a.advance_phys, b.advance_phys);
                }
                let gaps = spaced.clusters.len().saturating_sub(1) as f32;
                assert!((spaced.width - zero.width - gaps * 2.5).abs() < 0.002);
                if let Some(last) = spaced.clusters.last() {
                    assert!((last.x_end - spaced.width).abs() < 0.002);
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            let ligature_shaper = TextShaper::from_data(Arc::new(
                std::fs::read("/System/Library/Fonts/Supplemental/STIXGeneral.otf").unwrap(),
            ))
            .unwrap();
            let plain = ligature_shaper
                .shape_with_gap("ffi", 24.0, 1.0, ClusterGap::default())
                .unwrap();
            let ligature = ligature_shaper
                .shape_with_gap("ffi", 24.0, 1.0, ClusterGap::new(5.0).unwrap())
                .unwrap();
            assert!(
                ligature.clusters.len() < 3,
                "the whole string must retain the font's ligature"
            );
            assert_eq!(plain.clusters.len(), ligature.clusters.len());
            assert!(
                (ligature.width
                    - plain.width
                    - ligature.clusters.len().saturating_sub(1) as f32 * 5.0)
                    .abs()
                    < 0.002
            );
        }
        assert!(ClusterGap::new(-1.0).is_err());
        assert!(ClusterGap::new(f32::NAN).is_err());
        assert!(ClusterGap::new(f32::INFINITY).is_err());
        assert_eq!(
            shaper
                .shape_with_gap("abc", 24.0, 2.0, ClusterGap::new(f32::MAX).unwrap())
                .unwrap_err(),
            TextInputError::PositionOverflow
        );
    }

    #[test]
    fn test_direction_detection() {
        // Latin text
        assert_eq!(TextDirection::detect("Hello"), TextDirection::LeftToRight);

        // Arabic text
        assert_eq!(TextDirection::detect("مرحبا"), TextDirection::RightToLeft);

        // Hebrew text
        assert_eq!(TextDirection::detect("שלום"), TextDirection::RightToLeft);

        // Mixed (first strong character wins)
        assert_eq!(
            TextDirection::detect("Hello مرحبا"),
            TextDirection::LeftToRight
        );
        assert_eq!(
            TextDirection::detect("مرحبا Hello"),
            TextDirection::RightToLeft
        );

        // Numbers only (no strong character, defaults to LTR)
        assert_eq!(TextDirection::detect("12345"), TextDirection::LeftToRight);

        // Empty string
        assert_eq!(TextDirection::detect(""), TextDirection::LeftToRight);
    }

    #[test]
    fn test_bidi_class() {
        // Latin letters
        assert_eq!(unicode_bidi_class('A'), BidiClass::L);
        assert_eq!(unicode_bidi_class('z'), BidiClass::L);

        // Arabic letters
        assert_eq!(unicode_bidi_class('ا'), BidiClass::AL);
        assert_eq!(unicode_bidi_class('م'), BidiClass::AL);

        // Hebrew letters
        assert_eq!(unicode_bidi_class('א'), BidiClass::R);
        assert_eq!(unicode_bidi_class('ש'), BidiClass::R);

        // Numbers and punctuation
        assert_eq!(unicode_bidi_class('1'), BidiClass::Other);
        assert_eq!(unicode_bidi_class('.'), BidiClass::Other);
        assert_eq!(unicode_bidi_class(' '), BidiClass::Other);
    }
}
