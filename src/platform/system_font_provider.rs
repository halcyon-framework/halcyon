//! SPEC-HAL-103 Stage 1-5: System font provider with platform-specific discovery.
//!
//! This module provides:
//! - `load_default_system_ui_font()` - Returns native OS UI font
//!   - Windows: Segoe UI via DirectWrite (Stage 4)
//!   - macOS/iOS: SF system font via CoreText (Stage 5)
//!   - Linux: system-ui via fontconfig (Stage 6 - not yet implemented)
//! - `load_legacy_embedded_inter()` - Returns bundled Inter-Regular as explicit legacy fallback
//!
//! RULE: No function named "system" may ever return Inter implicitly.
//!
//! Consumers:
//! - `FontManagerTextEngine` (UI thread shaping)
//! - `WgpuRenderer` (glyph rasterization via FontManager)

use crate::platform::font_manager::FontWeight;
use std::sync::Arc;

/// Bundled legacy font (Inter-Regular) for fallback when OS font discovery fails.
static EMBEDDED_INTER_FONT: &[u8] = include_bytes!("../../assets/fonts/Inter/InterVariable.ttf");

/// Descriptor for a system font, containing all metadata and raw font data.
#[derive(Clone)]
pub struct SystemFontDescriptor {
    /// Font family name (e.g., "Inter", "SF Pro")
    pub family_name: String,
    /// Font weight
    pub weight: FontWeight,
    /// Whether the font is italic
    pub italic: bool,
    /// Raw font file data (TTF/OTF)
    pub data: Arc<Vec<u8>>,
}

/// Load the native OS system UI font.
///
/// Returns `Some(descriptor)` if the platform provides a system UI font,
/// `None` if discovery fails or is not yet implemented.
///
/// # Platform behavior
/// - Windows: Queries DirectWrite for Segoe UI Regular (SPEC-HAL-103 Stage 4)
/// - macOS/iOS: Queries CoreText for SF system font (SPEC-HAL-103 Stage 5)
/// - Linux: Will query fontconfig for system-ui (Stage 6 - not yet implemented)
pub fn load_default_system_ui_font() -> Option<SystemFontDescriptor> {
    #[cfg(target_os = "windows")]
    {
        load_windows_system_ui_font()
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        load_macos_system_ui_font()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    {
        // SPEC-HAL-103 Stage 6: Linux not yet implemented
        None
    }
}

// ============================================================================
// SPEC-HAL-103 Stage 4: Windows SystemUI via DirectWrite
// ============================================================================

/// Load Segoe UI Regular via DirectWrite system font collection.
///
/// Discovery flow:
/// 1. Get system font collection
/// 2. Find "Segoe UI" family
/// 3. Select Regular weight, non-italic face
/// 4. Extract font file bytes via IDWriteFontFileStream
///
/// Returns `None` on any failure (family not found, wrong weight, stream error).
#[cfg(target_os = "windows")]
fn load_windows_system_ui_font() -> Option<SystemFontDescriptor> {
    use dwrote::{FontCollection, FontStretch, FontStyle, FontWeight as DWriteFontWeight};

    // 1. Get system font collection
    let collection = FontCollection::system();

    // 2. Find Segoe UI family (case-insensitive by DirectWrite)
    let family = collection.font_family_by_name("Segoe UI").ok()??;

    // 3. Select Regular, non-italic face
    let font = family
        .first_matching_font(
            DWriteFontWeight::Regular,
            FontStretch::Normal,
            FontStyle::Normal,
        )
        .ok()?;

    // 4. Get FontFace and extract files
    let font_face = font.create_font_face();
    let files = font_face.files().ok()?;

    // 5. Extract font file bytes
    // Accept multiple files – Windows 10/11 may expose Segoe UI as TTC or variable font
    // dwrote abstracts this correctly and returns valid bytes from any backing file
    let mut data: Option<Vec<u8>> = None;
    for file in files {
        if let Ok(bytes) = file.font_file_bytes() {
            if !bytes.is_empty() {
                data = Some(bytes);
                break;
            }
        }
    }
    let data = data?;

    let descriptor = SystemFontDescriptor {
        family_name: "Segoe UI".to_string(),
        weight: FontWeight::Regular,
        italic: false,
        data: Arc::new(data),
    };

    Some(descriptor)
}

// ============================================================================
// SPEC-HAL-103 Stage 5: Apple SystemUI via CoreText
// ============================================================================

/// Load the Apple system UI font via CoreText.
///
/// Discovery flow:
/// 1. Create system UI font via CTFontCreateUIFontForLanguage
/// 2. Get CGFont from CTFont
/// 3. Create CGDataProvider and extract font table data
/// 4. Build complete font file with required tables
///
/// Returns `None` on any failure.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn load_macos_system_ui_font() -> Option<SystemFontDescriptor> {
    use core_foundation::base::TCFType;
    use core_foundation::data::CFData;
    use core_foundation::string::CFString;
    use core_text::font::{CTFont, CTFontRef, kCTFontSystemFontType};

    // FFI for CoreText functions
    unsafe extern "C" {
        fn CTFontCreateUIFontForLanguage(
            ui_type: u32,
            size: f64,
            language: *const std::ffi::c_void,
        ) -> CTFontRef;
        fn CTFontCopyFamilyName(font: CTFontRef) -> core_foundation::string::CFStringRef;
        fn CTFontCopyGraphicsFont(
            font: CTFontRef,
            attributes: *const std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        fn CGFontCopyTableTags(font: *const std::ffi::c_void) -> *const std::ffi::c_void;
        fn CGFontCopyTableForTag(
            font: *const std::ffi::c_void,
            tag: u32,
        ) -> *const std::ffi::c_void;
    }

    // 1. Create system UI font
    let ct_font_ref =
        unsafe { CTFontCreateUIFontForLanguage(kCTFontSystemFontType, 12.0, std::ptr::null()) };
    if ct_font_ref.is_null() {
        return None;
    }
    let _ct_font = unsafe { CTFont::wrap_under_create_rule(ct_font_ref) };

    // 2. Get family name
    let family_name = unsafe {
        let name_ref = CTFontCopyFamilyName(ct_font_ref);
        if name_ref.is_null() {
            ".AppleSystemUIFont".to_string()
        } else {
            CFString::wrap_under_create_rule(name_ref).to_string()
        }
    };

    // 3. Get CGFont for table access
    let cg_font_ptr = unsafe { CTFontCopyGraphicsFont(ct_font_ref, std::ptr::null()) };
    if cg_font_ptr.is_null() {
        return None;
    }

    // 4. Get table tags
    let tags_array_ptr = unsafe { CGFontCopyTableTags(cg_font_ptr) };
    if tags_array_ptr.is_null() {
        return None;
    }
    let tags_array = unsafe {
        core_foundation::array::CFArray::<u32>::wrap_under_create_rule(tags_array_ptr as _)
    };

    // 5. Collect all tables
    let mut tables: Vec<(u32, Vec<u8>)> = Vec::new();
    for i in 0..tags_array.len() {
        let tag = *tags_array.get(i).unwrap();
        let data_ptr = unsafe { CGFontCopyTableForTag(cg_font_ptr, tag) };
        if !data_ptr.is_null() {
            let cf_data = unsafe { CFData::wrap_under_create_rule(data_ptr as _) };
            tables.push((tag, cf_data.bytes().to_vec()));
        }
    }

    if tables.is_empty() {
        return None;
    }

    // 6. Build OpenType font file
    let data = build_otf_from_tables(&tables)?;

    let descriptor = SystemFontDescriptor {
        family_name: family_name.clone(),
        weight: FontWeight::Regular,
        italic: false,
        data: Arc::new(data),
    };

    Some(descriptor)
}

/// Build a valid OpenType font file from table data.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn build_otf_from_tables(tables: &[(u32, Vec<u8>)]) -> Option<Vec<u8>> {
    let num_tables = tables.len() as u16;
    if num_tables == 0 {
        return None;
    }

    // Calculate searchRange, entrySelector, rangeShift
    let mut search_range = 1u16;
    let mut entry_selector = 0u16;
    while search_range * 2 <= num_tables {
        search_range *= 2;
        entry_selector += 1;
    }
    search_range *= 16;
    let range_shift = num_tables * 16 - search_range;

    // Header size: 12 bytes + 16 bytes per table record
    let header_size = 12 + (num_tables as usize) * 16;

    // Calculate table offsets
    let mut current_offset = header_size as u32;
    let mut table_records: Vec<(u32, u32, u32, u32)> = Vec::new(); // (tag, checksum, offset, length)

    for (tag, data) in tables {
        let checksum = calculate_table_checksum(data);
        let length = data.len() as u32;
        table_records.push((*tag, checksum, current_offset, length));
        // Align to 4 bytes
        current_offset += (length + 3) & !3;
    }

    // Build the font file
    let total_size = current_offset as usize;
    let mut font_data = vec![0u8; total_size];

    // Write offset table header
    // sfntVersion: 'OTTO' for CFF, 0x00010000 for TrueType
    // Use 'true' (0x74727565) which works for both
    font_data[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    font_data[4..6].copy_from_slice(&num_tables.to_be_bytes());
    font_data[6..8].copy_from_slice(&search_range.to_be_bytes());
    font_data[8..10].copy_from_slice(&entry_selector.to_be_bytes());
    font_data[10..12].copy_from_slice(&range_shift.to_be_bytes());

    // Write table records
    for (i, (tag, checksum, offset, length)) in table_records.iter().enumerate() {
        let record_offset = 12 + i * 16;
        font_data[record_offset..record_offset + 4].copy_from_slice(&tag.to_be_bytes());
        font_data[record_offset + 4..record_offset + 8].copy_from_slice(&checksum.to_be_bytes());
        font_data[record_offset + 8..record_offset + 12].copy_from_slice(&offset.to_be_bytes());
        font_data[record_offset + 12..record_offset + 16].copy_from_slice(&length.to_be_bytes());
    }

    // Write table data
    for ((_, data), (_, _, offset, _)) in tables.iter().zip(table_records.iter()) {
        let start = *offset as usize;
        font_data[start..start + data.len()].copy_from_slice(data);
    }

    Some(font_data)
}

/// Calculate OpenType table checksum.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn calculate_table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 4 <= data.len() {
        sum = sum.wrapping_add(u32::from_be_bytes([
            data[i],
            data[i + 1],
            data[i + 2],
            data[i + 3],
        ]));
        i += 4;
    }
    // Handle remaining bytes
    if i < data.len() {
        let mut last = [0u8; 4];
        for (j, byte) in data[i..].iter().enumerate() {
            last[j] = *byte;
        }
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

/// Load the legacy embedded Inter font as fallback.
///
/// This is the ONLY way to access the bundled Inter font.
/// Use this when `load_default_system_ui_font()` returns `None`.
///
/// This function exists to:
/// 1. Provide a guaranteed fallback font when OS discovery fails
/// 2. Maintain backwards compatibility during SPEC-HAL-103 migration
/// 3. Support tests and tooling that need a known font
pub fn load_legacy_embedded_inter() -> SystemFontDescriptor {
    SystemFontDescriptor {
        family_name: "Inter".to_string(),
        weight: FontWeight::Regular,
        italic: false,
        data: Arc::new(EMBEDDED_INTER_FONT.to_vec()),
    }
}

/// Real style companions for the embedded fallback, admitted only at bootstrap.
/// Static faces carry their actual outlines/metrics; no synthetic weight or slant.
pub(crate) fn load_legacy_embedded_inter_styles() -> [SystemFontDescriptor; 3] {
    [
        (
            FontWeight::Bold,
            false,
            include_bytes!("../../assets/fonts/Inter/Inter-Bold.ttf").as_slice(),
        ),
        (
            FontWeight::Regular,
            true,
            include_bytes!("../../assets/fonts/Inter/Inter-Italic.ttf").as_slice(),
        ),
        (
            FontWeight::Bold,
            true,
            include_bytes!("../../assets/fonts/Inter/Inter-BoldItalic.ttf").as_slice(),
        ),
    ]
    .map(|(weight, italic, data)| SystemFontDescriptor {
        family_name: "Inter".to_string(),
        weight,
        italic,
        data: Arc::new(data.to_vec()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On Linux, load_default_system_ui_font returns None (not yet implemented)
    #[test]
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
    fn test_load_default_system_ui_font_returns_none_on_linux() {
        // SPEC-HAL-103 Stage 6: Linux not yet implemented
        assert!(
            load_default_system_ui_font().is_none(),
            "Linux should return None until Stage 6"
        );
    }

    /// On macOS, load_default_system_ui_font returns the system UI font
    #[test]
    #[cfg(target_os = "macos")]
    fn test_load_default_system_ui_font_returns_system_font_on_macos() {
        // SPEC-HAL-103 Stage 5: macOS should return system UI font
        let descriptor = load_default_system_ui_font();
        assert!(descriptor.is_some(), "macOS should find system UI font");

        let descriptor = descriptor.unwrap();
        // Family name is typically ".AppleSystemUIFont" or "SF Pro"
        assert!(
            !descriptor.family_name.is_empty(),
            "Family name should not be empty"
        );
        assert_eq!(descriptor.weight, FontWeight::Regular);
        assert!(!descriptor.italic);
        assert!(!descriptor.data.is_empty(), "Font data should not be empty");
    }

    /// On Windows, load_default_system_ui_font returns Segoe UI
    #[test]
    #[cfg(target_os = "windows")]
    fn test_load_default_system_ui_font_returns_segoe_ui_on_windows() {
        // SPEC-HAL-103 Stage 4: Windows should return Segoe UI
        let descriptor = load_default_system_ui_font();
        assert!(descriptor.is_some(), "Windows should find Segoe UI");

        let descriptor = descriptor.unwrap();
        assert_eq!(descriptor.family_name, "Segoe UI");
        assert_eq!(descriptor.weight, FontWeight::Regular);
        assert!(!descriptor.italic);
        assert!(!descriptor.data.is_empty(), "Font data should not be empty");
    }

    #[test]
    fn test_load_legacy_embedded_inter() {
        let descriptor = load_legacy_embedded_inter();
        assert_eq!(descriptor.family_name, "Inter");
        assert_eq!(descriptor.weight, FontWeight::Regular);
        assert!(!descriptor.italic);
        assert!(!descriptor.data.is_empty(), "Font data should not be empty");
    }

    #[test]
    fn test_legacy_inter_data_consistency() {
        let d1 = load_legacy_embedded_inter();
        let d2 = load_legacy_embedded_inter();
        assert_eq!(d1.data.len(), d2.data.len());
        assert_eq!(&*d1.data, &*d2.data, "Font data should be consistent");
    }
}
