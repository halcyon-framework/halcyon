//! SPEC-HAL-047 + SPEC-HAL-065: macOS CoreText implementation of `NativeFontBackend`.
//!
//! TextShaper (rustybuzz) supplies the glyphs for the raster backend.
//! Scoped HAL-048 outline qualification shares its byte-bound font construction.
//!
//! Uses core-text crate for safe CoreText bindings.

use crate::core::text_engine::TextMetrics;
use crate::platform::native_font_backend::{GlyphBitmap, GlyphFormat, NativeFontBackend};
use crate::renderer::glyph_atlas::{GlyphKey, GlyphMetrics};

use core_foundation::base::TCFType;
use core_graphics::context::CGContext;
use core_graphics::data_provider::CGDataProvider;
use core_graphics::font::{CGFont, CGGlyph};
use core_graphics::geometry::{CGAffineTransform, CGPoint, CGRect, CGSize};
use core_graphics::path::SysCGPathRef;
use core_text::font::{CTFont, CTFontRef, kCTFontSystemFontType, new_from_CGFont, new_from_name};
use core_text::font_descriptor::kCTFontOrientationHorizontal;
use foreign_types::ForeignTypeRef;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// CoreText and Core Graphics operations missing from the Rust bindings.
unsafe extern "C" {
    fn CTFontDrawGlyphs(
        font: CTFontRef,
        glyphs: *const CGGlyph,
        positions: *const CGPoint,
        count: usize,
        context: *mut std::ffi::c_void,
    );

    fn CTFontCreateUIFontForLanguage(
        ui_type: u32,
        size: f64,
        language: *const std::ffi::c_void,
    ) -> CTFontRef;

    fn CGPathIsEmpty(path: SysCGPathRef) -> bool;
    fn CGPathGetPathBoundingBox(path: SysCGPathRef) -> CGRect;
}

fn font_from_data(data: &Arc<Vec<u8>>, font_size: f32) -> Option<CTFont> {
    let data_provider = CGDataProvider::from_buffer(Arc::clone(data));
    let cg_font = CGFont::from_data_provider(data_provider).ok()?;
    Some(new_from_CGFont(&cg_font, f64::from(font_size)))
}

pub(crate) struct CoreTextOutlineReader(CTFont);

impl CoreTextOutlineReader {
    pub(crate) fn new(data: &Arc<Vec<u8>>, font_size: f32) -> Option<Self> {
        font_from_data(data, font_size).map(Self)
    }

    pub(crate) fn glyph_bounds(&self, glyph: CGGlyph) -> Option<CGRect> {
        let identity = CGAffineTransform::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let path = self.0.create_path_for_glyph(glyph, &identity).ok()?;
        // SAFETY: The owned path stays live through both Core Graphics queries.
        unsafe {
            if CGPathIsEmpty(path.as_ptr()) {
                return None;
            }
            Some(CGPathGetPathBoundingBox(path.as_ptr()))
        }
    }
}

/// macOS CoreText implementation of `NativeFontBackend`.
pub struct CoreTextBackend {
    font_id: u64,
    /// Optional embedded font data for SPEC-HAL-065 unified shaping.
    /// When present, create_font() uses this instead of the system font.
    font_data: Option<Arc<Vec<u8>>>,
    #[cfg(test)]
    font_metrics_call_count: usize,
}

impl std::fmt::Debug for CoreTextBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreTextBackend")
            .field("font_id", &self.font_id)
            .field(
                "font_data",
                &self
                    .font_data
                    .as_ref()
                    .map(|d| format!("{} bytes", d.len())),
            )
            .finish()
    }
}

impl CoreTextBackend {
    /// Create a CoreTextBackend using the system UI font.
    /// NOTE: This should NOT be used with rustybuzz shaping as glyph IDs won't match.
    pub fn new() -> Self {
        Self {
            font_id: Self::hash_font_name("system-ui"),
            font_data: None,
            #[cfg(test)]
            font_metrics_call_count: 0,
        }
    }

    /// Create a CoreTextBackend from raw font data.
    ///
    /// This is the preferred constructor for SPEC-HAL-065 integration, as it ensures
    /// the backend rasterizes using the same font data that rustybuzz uses for shaping.
    /// This prevents glyph ID mismatches between shaping and rasterization.
    pub fn new_from_data(font_data: Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        // Validate font data by trying to create a CGFont
        let data_provider = CGDataProvider::from_buffer(font_data.clone());
        let _cg_font = CGFont::from_data_provider(data_provider).ok()?;

        Some(Self {
            font_id,
            font_data: Some(font_data),
            #[cfg(test)]
            font_metrics_call_count: 0,
        })
    }

    fn hash_font_name(name: &str) -> u64 {
        let mut h = DefaultHasher::new();
        name.hash(&mut h);
        h.finish()
    }

    fn create_font(&self, font_size: f32) -> CTFont {
        // If we have embedded font data, use it
        if let Some(data) = &self.font_data
            && let Some(font) = font_from_data(data, font_size)
        {
            return font;
        }

        // Fallback to macOS system UI font
        unsafe {
            let font_ref = CTFontCreateUIFontForLanguage(
                kCTFontSystemFontType,
                font_size as f64,
                std::ptr::null(),
            );
            if font_ref.is_null() {
                // Fallback to Helvetica Neue if system font unavailable
                new_from_name("Helvetica Neue", font_size as f64).unwrap_or_else(|_| {
                    new_from_name("Helvetica", font_size as f64).expect("No fallback font")
                })
            } else {
                CTFont::wrap_under_create_rule(font_ref)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn font_metrics_call_count_for_test(&self) -> usize {
        self.font_metrics_call_count
    }
}

impl Default for CoreTextBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeFontBackend for CoreTextBackend {
    fn from_font_data(data: Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        Self::new_from_data(data, font_id)
    }

    fn font_metrics(&mut self, font_size: f32, _scale: f32) -> TextMetrics {
        #[cfg(test)]
        {
            self.font_metrics_call_count += 1;
        }
        // Return LOGICAL pixels: font_size is logical, metrics scale with it.
        // Physical scaling is renderer's job, not layout's.
        let font = self.create_font(font_size);

        let ascent = font.ascent() as f32;
        let descent = font.descent() as f32;
        let leading = font.leading() as f32;
        let line_height = ascent + descent + leading;

        TextMetrics {
            ascent,
            descent,
            line_gap: leading,
            line_height,
        }
    }

    fn rasterize_glyph(&mut self, key: &GlyphKey, scale: f32) -> Option<GlyphBitmap> {
        if key.glyph_id == 0 {
            return None;
        }
        let font_size_logical = key.font_size_scaled as f32 / 100.0;
        // R-010B: 2× oversample for sharper LCD text
        let oversample = 2.0_f32;

        // Canonical physical size (same as font_metrics uses)
        let font_size_phys = font_size_logical * scale;
        // Oversampled size for bitmap rasterization
        let font_size_oversampled = font_size_phys * oversample;
        let font = self.create_font(font_size_oversampled);

        let glyph_id = key.glyph_id as CGGlyph;
        let glyphs: [CGGlyph; 1] = [glyph_id];

        // Get font metrics (constant per size, not per glyph)
        let ascent = font.ascent() as f32;
        let descent = font.descent() as f32;
        let leading = font.leading() as f32;
        let line_height = ascent + descent + leading;

        // Baseline-box dimensions: ascent above baseline, descent below
        let ascent_px = ascent.ceil().max(0.0);
        let descent_px = descent.ceil().max(0.0);

        // Get glyph bounding rect (only used for width/bearing_x now)
        let rect = font.get_bounding_rects_for_glyphs(kCTFontOrientationHorizontal, &glyphs);

        // Get advance (unsafe in core-text 21.0.0)
        let mut advances = [CGSize::new(0.0, 0.0)];
        unsafe {
            font.get_advances_for_glyphs(
                kCTFontOrientationHorizontal,
                glyphs.as_ptr(),
                advances.as_mut_ptr(),
                1,
            );
        }

        let advance_raw = advances[0].width as f32;

        // SPEC-HAL-360: Native empty bounds explicitly classify a no-ink glyph.
        if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
            // Normalize metrics to canonical physical size (divide by oversample)
            // EXCEPTION: bearing_x/y remain in OVERSAMPLE resolution (glyph_quad normalizes)
            let metrics = GlyphMetrics {
                advance: advance_raw / oversample,
                bearing_x: 0.0,
                bearing_y: ascent_px, // OVERSAMPLE resolution - glyph_quad will normalize
                width: 0,
                height: 0,
                ascender: ascent / oversample,
                descender: descent / oversample,
                line_height: line_height / oversample,
            };
            return Some(GlyphBitmap {
                key: *key,
                metrics,
                pixels: Vec::new(),
                format: GlyphFormat::Grayscale,
            });
        }

        // bearing_x from bounding box (horizontal alignment is fine)
        let bearing_x = rect.origin.x as f32;

        // bearing_y = distance from baseline to top of bitmap = ascent_px
        // This is CONSTANT per font size, not per glyph
        let bearing_y = ascent_px;

        // Bitmap dimensions: baseline-box + padding
        let width = (rect.size.width.ceil() as u32).max(1) + 2;
        let height = (ascent_px + descent_px) as u32 + 2;

        // True grayscale AA: 8-bit single-channel bitmap
        // Using DeviceGray colorspace with kCGImageAlphaNone (1 byte per pixel)
        let color_space = core_graphics::color_space::CGColorSpace::create_device_gray();
        let bytes_per_row = width as usize; // 1 byte per pixel (grayscale)
        let mut context = CGContext::create_bitmap_context(
            None,
            width as usize,
            height as usize,
            8,             // bits per component
            bytes_per_row, // bytes per row
            &color_space,
            core_graphics::base::kCGImageAlphaNone, // Grayscale, no alpha
        );

        // Set up context for grayscale AA drawing (no LCD smoothing)
        context.set_allows_antialiasing(true);
        context.set_should_antialias(true);
        context.set_should_smooth_fonts(false); // Disable LCD subpixel AA

        // Grayscale rasterization: BLACK text on WHITE background
        // Fill background with white first
        context.set_gray_fill_color(1.0, 1.0); // white
        let bg_rect = CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(width as f64, height as f64),
        );
        context.fill_rect(bg_rect);

        // Set fill color to BLACK for glyph
        context.set_gray_fill_color(0.0, 1.0); // black

        // Position glyph by baseline, not bounding box
        let draw_x = -bearing_x + 1.0;
        // Baseline inside bitmap: bottom padding (1) + descent_px
        // In CG coords (Y up), this leaves room for descenders below
        let baseline_y = 1.0 + descent_px;

        // Draw glyph using FFI (core-text 21.0.0 draw_glyphs has CGContext version mismatch)
        let position = CGPoint::new(draw_x as f64, baseline_y as f64);
        let ct_font_ref: CTFontRef = font.as_concrete_TypeRef();
        unsafe {
            CTFontDrawGlyphs(
                ct_font_ref,
                glyphs.as_ptr(),
                &position,
                1,
                context.as_ptr() as *mut std::ffi::c_void,
            );
        }

        // Extract grayscale pixel data (1 byte per pixel)
        let data = context.data();
        let mut pixels: Vec<u8> = unsafe {
            std::slice::from_raw_parts(data.as_ptr(), (width * height) as usize).to_vec()
        };

        // Invert coverage: black-on-white → coverage mask
        // White (255) = no glyph, Black (0) = full glyph
        // Coverage = 255 - pixel_value (so glyph area becomes high coverage)
        for pixel in pixels.iter_mut() {
            *pixel = 255 - *pixel;
        }

        // Normalize metrics to canonical physical size (divide by oversample)
        // This ensures metrics match font_metrics() output at font_size * scale
        //
        // EXCEPTION: bearing_x/y remain in OVERSAMPLE resolution.
        // glyph_quad.rs divides bearing_x/y by oversample uniformly for both backends.
        // Normalizing here would cause double-division on macOS.
        let metrics = GlyphMetrics {
            advance: advance_raw / oversample,
            bearing_x, // OVERSAMPLE resolution - glyph_quad will normalize
            bearing_y, // OVERSAMPLE resolution - glyph_quad will normalize
            width,     // bitmap at oversample resolution
            height,    // bitmap at oversample resolution
            ascender: ascent / oversample,
            descender: descent / oversample,
            line_height: line_height / oversample,
        };

        Some(GlyphBitmap {
            key: *key,
            metrics,
            pixels,
            format: GlyphFormat::Grayscale,
        })
    }
}
