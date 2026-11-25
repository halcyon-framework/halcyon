//! SPEC-HAL-047 + SPEC-HAL-065 + SPEC-HAL-066: Native text backend trait and shared types.
//!
//! Platform-agnostic interface used by the renderer and platform-specific
//! implementations (DirectWrite, CoreText, FreeType).
//!
//! Per SPEC-HAL-065, backends perform RASTERIZATION ONLY.
//! All text shaping is handled by TextShaper (rustybuzz).
//!
//! Per SPEC-HAL-066, all backends produce normalized GlyphBitmap output
//! with explicit format tagging for grayscale or subpixel rendering.

use crate::core::text_engine::TextMetrics;
use crate::renderer::glyph_atlas::{GlyphKey, GlyphMetrics};

/// Glyph bitmap format for atlas storage.
///
/// Per SPEC-HAL-066, all rasterizers output one of these formats.
/// Grayscale is the current default; SubpixelRGB is reserved for SPEC-HAL-064.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GlyphFormat {
    /// Single-channel grayscale coverage (R8).
    /// `data.len() == width * height`
    #[default]
    Grayscale,
    /// RGB subpixel coverage (RGBA8 or RGB8).
    /// Reserved for SPEC-HAL-064 macOS/Windows subpixel rendering.
    /// `data.len() == width * height * 4` (RGBA) or `width * height * 3` (RGB)
    SubpixelRgb,
}

/// Rasterized glyph bitmap produced by a native backend.
///
/// Per SPEC-HAL-066, this is the unified output format for all platforms.
/// Pixels are tightly packed according to the format field.
#[derive(Debug, Clone)]
pub struct GlyphBitmap {
    pub key: GlyphKey,
    pub metrics: GlyphMetrics,
    pub pixels: Vec<u8>,
    /// Pixel format (grayscale R8 or subpixel RGB).
    pub format: GlyphFormat,
}

/// Platform-agnostic interface for font metrics and glyph rasterization.
///
/// Per SPEC-HAL-065, backends no longer perform text shaping.
/// All shaping is done by TextShaper (rustybuzz) before calling rasterize_glyph.
///
/// - `font_size` is in logical pixels.
/// - `scale` converts logical → physical (OS scale factor).
pub trait NativeFontBackend {
    /// Bootstrap a backend-owned face from manager-admitted immutable bytes.
    fn from_font_data(_data: std::sync::Arc<Vec<u8>>, _font_id: u64) -> Option<Self>
    where
        Self: Sized,
    {
        None
    }

    /// Return font metrics for the given size.
    fn font_metrics(&mut self, font_size: f32, scale: f32) -> TextMetrics;

    /// Rasterize a single glyph into an R8 bitmap.
    ///
    /// The glyph_id in the GlyphKey comes from TextShaper output.
    /// Implementations must use the same font that was used for shaping.
    /// A valid no-ink glyph has zero width/height and empty pixels (SPEC-HAL-360).
    /// `None` reports unavailability, never successful no-ink realization.
    fn rasterize_glyph(&mut self, key: &GlyphKey, scale: f32) -> Option<GlyphBitmap>;
}
