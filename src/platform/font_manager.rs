//! SPEC-HAL-051: Unified Font Loading & Asset Management
//! SPEC-HAL-049: Font Collections, Fallback Rules & Font Resolution
//!
//! Provides the single, universal font pipeline for Halcyon:
//! - Loading fonts from file, memory, embedded assets
//! - Registering fonts into the engine at runtime
//! - Creating LoadedFontFace objects with complete cross-platform metadata
//! - Ensuring byte-for-byte consistent input to rustybuzz (shaping) and backends (rasterization)
//! - Deterministic font resolution and fallback (049)

use crate::core::text_engine::FontFaceBinding;
use crate::core::text_shaper::TextShaper;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

#[derive(Debug, Clone)]
pub(crate) struct CatalogFace {
    pub binding: FontFaceBinding,
    pub shaper: Arc<TextShaper>,
    pub resolved: ResolvedFontFace,
}

/// Immutable manager-created view. No platform handles or mutable registry cross threads.
#[derive(Debug, Clone, Default)]
pub struct FontFaceCatalog {
    faces: Arc<HashMap<u64, CatalogFace>>,
    families: Arc<HashMap<String, Vec<u64>>>,
}

impl FontFaceCatalog {
    pub(crate) fn single(
        binding: FontFaceBinding,
        shaper: Arc<TextShaper>,
        postscript_name: String,
    ) -> Self {
        let id = binding.font_id();
        let family = binding.family_name().to_string();
        let resolved = ResolvedFontFace {
            font_id: id,
            family_name: family.clone(),
            postscript_name,
            weight: binding.weight(),
            italic: binding.italic(),
            coverage_map: CoverageMap::from_font_data(binding.data()),
        };
        Self {
            faces: Arc::new(HashMap::from([(
                id,
                CatalogFace {
                    binding,
                    shaper,
                    resolved,
                },
            )])),
            families: Arc::new(HashMap::from([(family, vec![id])])),
        }
    }

    pub(crate) fn resolve(
        &self,
        default_id: u64,
        style: &crate::core::paragraph::TextStyle,
    ) -> Result<crate::core::text_engine::ResolvedTextStyle, crate::core::text_engine::TextInputError>
    {
        use crate::core::text_engine::{ResolvedTextStyle, TextBinding, TextInputError};
        style.validate()?;
        let family = if style.font_family.name().is_empty() {
            FontFamily::named(
                self.get(default_id)
                    .ok_or(TextInputError::FontUnavailable)?
                    .binding
                    .family_name(),
            )
        } else {
            style.font_family.to_fm_family()
        };
        let resolved = crate::platform::font_resolver::FontResolver::with_catalog(self)
            .resolve(&FontStack::single(family), style.weight, style.italic)
            .ok_or(TextInputError::FontUnavailable)?;
        let face = self
            .get(resolved.font_id)
            .ok_or(TextInputError::FontUnavailable)?;
        Ok(ResolvedTextStyle {
            binding: TextBinding::new(face.binding.clone(), style.cluster_gap),
            font_size: style.font_size,
        })
    }

    pub(crate) fn get(&self, font_id: u64) -> Option<&CatalogFace> {
        self.faces.get(&font_id)
    }

    pub(crate) fn faces(&self) -> impl Iterator<Item = &CatalogFace> {
        self.faces.values()
    }

    pub(crate) fn same_generation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.faces, &other.faces) && Arc::ptr_eq(&self.families, &other.families)
    }

    /// A source-ink qualification can identify this catalog without keeping its faces alive.
    pub(crate) fn weak_faces(&self) -> Weak<HashMap<u64, CatalogFace>> {
        Arc::downgrade(&self.faces)
    }

    pub(crate) fn find(
        &self,
        family: &str,
        weight: FontWeight,
        italic: bool,
    ) -> Option<ResolvedFontFace> {
        let ids = self.families.get(family)?;
        ids.iter()
            .filter_map(|id| self.faces.get(id))
            .filter(|face| face.resolved.italic == italic)
            .min_by_key(|face| {
                (
                    (face.resolved.weight.to_value() as i32 - weight.to_value() as i32).abs(),
                    face.resolved.font_id,
                )
            })
            .map(|face| face.resolved.clone())
    }
}

/// Font weight as specified in OpenType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontWeight {
    Thin,       // 100
    ExtraLight, // 200
    Light,      // 300
    #[default]
    Regular, // 400
    Medium,     // 500
    SemiBold,   // 600
    Bold,       // 700
    ExtraBold,  // 800
    Black,      // 900
}

impl FontWeight {
    /// Convert to numeric weight value (100-900).
    pub fn to_value(self) -> u16 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::ExtraLight => 200,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::SemiBold => 600,
            FontWeight::Bold => 700,
            FontWeight::ExtraBold => 800,
            FontWeight::Black => 900,
        }
    }

    /// Create from numeric weight value.
    pub fn from_value(value: u16) -> Self {
        match value {
            0..=149 => FontWeight::Thin,
            150..=249 => FontWeight::ExtraLight,
            250..=349 => FontWeight::Light,
            350..=449 => FontWeight::Regular,
            450..=549 => FontWeight::Medium,
            550..=649 => FontWeight::SemiBold,
            650..=749 => FontWeight::Bold,
            750..=849 => FontWeight::ExtraBold,
            _ => FontWeight::Black,
        }
    }
}

// =============================================================================
// SPEC-HAL-049: Font Collections, Fallback Rules & Font Resolution
// =============================================================================

/// Logical font family for style resolution (SPEC-049 §4.1).
///
/// Used by FontResolver to map logical families to concrete LoadedFontFace objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// Named family (e.g., "Inter", "Noto Sans")
    Named(String),
    /// Generic serif family
    Serif,
    /// Generic sans-serif family
    SansSerif,
    /// Generic monospace family
    Monospace,
    /// System UI font
    SystemUI,
}

impl FontFamily {
    /// Create a named family.
    pub fn named(name: impl Into<String>) -> Self {
        FontFamily::Named(name.into())
    }
}

/// Ordered font family stack for fallback resolution (SPEC-049 §4.2).
///
/// Families are tried in order: primary → fallbacks.
#[derive(Debug, Clone, Default)]
pub struct FontStack(pub Vec<FontFamily>);

impl FontStack {
    /// Create an empty font stack.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create a font stack with a single family.
    pub fn single(family: FontFamily) -> Self {
        Self(vec![family])
    }

    /// Create a font stack from multiple families.
    pub fn from_families(families: Vec<FontFamily>) -> Self {
        Self(families)
    }

    /// Add a fallback family to the stack.
    pub fn with_fallback(mut self, family: FontFamily) -> Self {
        self.0.push(family);
        self
    }

    /// Iterate over families in order.
    pub fn iter(&self) -> impl Iterator<Item = &FontFamily> {
        self.0.iter()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// High-level style descriptor for font resolution (SPEC-049 §4.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontDescriptor {
    /// Font family
    pub family: FontFamily,
    /// Font weight
    pub weight: FontWeight,
    /// Whether italic
    pub italic: bool,
}

impl FontDescriptor {
    /// Create a new font descriptor.
    pub fn new(family: FontFamily, weight: FontWeight, italic: bool) -> Self {
        Self {
            family,
            weight,
            italic,
        }
    }
}

/// Font source classification (SPEC-049).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontSource {
    /// System-installed font
    System,
    /// Custom/user-provided font
    Custom,
}

/// Unicode coverage map for fallback resolution (SPEC-049 §6.2).
///
/// Tracks which codepoints a font supports for efficient fallback lookup.
/// Built at font load time by inspecting the font's cmap table.
#[derive(Debug, Clone, Default)]
pub struct CoverageMap {
    /// Set of supported codepoints
    supported: Arc<HashSet<char>>,
    /// Whether the map has been populated
    populated: bool,
}

impl CoverageMap {
    /// Create an empty coverage map.
    pub fn new() -> Self {
        Self {
            supported: Arc::new(HashSet::new()),
            populated: false,
        }
    }

    /// Check if a codepoint is covered by this font.
    pub fn covers(&self, ch: char) -> bool {
        self.supported.contains(&ch)
    }

    /// Mark a codepoint as supported.
    pub fn mark_supported(&mut self, ch: char) {
        Arc::make_mut(&mut self.supported).insert(ch);
        self.populated = true;
    }

    /// Get the number of covered codepoints.
    pub fn coverage_count(&self) -> usize {
        self.supported.len()
    }

    /// Check if the coverage map has been populated.
    pub fn is_populated(&self) -> bool {
        self.populated
    }

    /// Build coverage map from font data using ttf-parser.
    ///
    /// HAL049 §15: exact Unicode cmap coverage, shared immutably by catalog views.
    pub fn from_font_data(data: &[u8]) -> Self {
        let mut map = Self::new();

        let Ok(face) = ttf_parser::Face::parse(data, 0) else {
            return map;
        };

        let mut supported = HashSet::new();
        if let Some(cmap) = face.tables().cmap {
            for subtable in cmap.subtables {
                if !subtable.is_unicode() {
                    continue;
                }
                subtable.codepoints(|codepoint| {
                    if let Some(ch) = char::from_u32(codepoint)
                        && face.glyph_index(ch).is_some_and(|glyph| glyph.0 != 0)
                    {
                        supported.insert(ch);
                    }
                });
            }
        }
        map.supported = Arc::new(supported);

        map.populated = true;
        map
    }
}

/// Logical grouping of font faces under a family name (SPEC-051 §4.2).
#[derive(Debug, Clone)]
pub struct FontCollection {
    /// Family name
    pub family: String,
    /// Faces in this collection
    pub faces: Vec<FontAsset>,
}

impl FontCollection {
    /// Create a new empty collection.
    pub fn new(family: impl Into<String>) -> Self {
        Self {
            family: family.into(),
            faces: Vec::new(),
        }
    }

    /// Add a face to the collection.
    pub fn add_face(&mut self, face: FontAsset) {
        self.faces.push(face);
    }
}

/// Raw font resource (source-agnostic).
#[derive(Debug, Clone)]
pub struct FontAsset {
    /// Font family name (e.g., "Inter", "Noto Sans")
    pub family_name: String,
    /// Font weight
    pub weight: FontWeight,
    /// Whether the font is italic
    pub italic: bool,
    /// Raw font file data (TTF/OTF)
    pub data: Arc<Vec<u8>>,
}

/// Platform-specific font handle for rasterization.
///
/// Each platform backend uses its native API to rasterize glyphs.
/// The handle wraps the platform-specific font object created from
/// the same bytes used by rustybuzz for shaping.
pub enum PlatformFontHandle {
    /// Windows DirectWrite (IDWriteFontFace equivalent via dwrote)
    #[cfg(windows)]
    DirectWrite { font_face: dwrote::FontFace },
    /// Apple CoreText (CTFontRef)
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    CoreText {
        // CTFont created from our font data
        // font: core_text::font::CTFont,
    },
    /// Linux/Unix FreeType (FT_Face)
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
    FreeType {
        // FT_Face created from our font data
        // face: freetype::Face,
    },
    /// Placeholder for platforms without native handle yet
    None,
}

impl std::fmt::Debug for PlatformFontHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(windows)]
            PlatformFontHandle::DirectWrite { .. } => f.debug_struct("DirectWrite").finish(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            PlatformFontHandle::CoreText { .. } => f.debug_struct("CoreText").finish(),
            #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
            PlatformFontHandle::FreeType { .. } => f.debug_struct("FreeType").finish(),
            PlatformFontHandle::None => f.debug_struct("None").finish(),
        }
    }
}

/// Fully constructed font with shaper and platform state (SPEC-051 §4.3).
///
/// This is the ONLY object backend code ever interacts with.
/// Guarantees: same bytes for shaping (rustybuzz) and rasterization (backend).
#[derive(Debug)]
pub struct LoadedFontFace {
    /// Stable font identifier (hash of family, weight, italic, SHA-256 of data)
    pub font_id: u64,
    /// Font family name
    pub family_name: String,
    /// PostScript name from the font
    pub postscript_name: String,
    /// Font weight
    pub weight: FontWeight,
    /// Whether the font is italic
    pub italic: bool,
    /// Raw font data (shared with shaper)
    pub data: Arc<Vec<u8>>,
    /// Font design units per em
    pub units_per_em: u16,
    /// Unified text shaper (rustybuzz)
    pub shaper: Arc<TextShaper>,
    /// Platform-specific handle for rasterization
    pub platform_handle: PlatformFontHandle,
    /// Unicode coverage map for fallback resolution (SPEC-049)
    pub coverage_map: CoverageMap,
}

/// Lightweight resolved font face returned by FontResolver (SPEC-049 §4.4).
///
/// Always derived from a LoadedFontFace. Contains the subset of fields
/// needed for shaping and rasterization decisions.
#[derive(Debug, Clone)]
pub struct ResolvedFontFace {
    /// Stable font identifier
    pub font_id: u64,
    /// Font family name
    pub family_name: String,
    /// PostScript name from the font
    pub postscript_name: String,
    /// Font weight
    pub weight: FontWeight,
    /// Whether the font is italic
    pub italic: bool,
    /// Unicode coverage map for fallback decisions
    pub coverage_map: CoverageMap,
}

impl ResolvedFontFace {
    /// Create a ResolvedFontFace from a LoadedFontFace.
    pub fn from_loaded(face: &LoadedFontFace) -> Self {
        Self {
            font_id: face.font_id,
            family_name: face.family_name.clone(),
            postscript_name: face.postscript_name.clone(),
            weight: face.weight,
            italic: face.italic,
            coverage_map: face.coverage_map.clone(),
        }
    }

    /// Check if this face covers a codepoint.
    pub fn covers(&self, ch: char) -> bool {
        self.coverage_map.covers(ch)
    }
}

impl LoadedFontFace {
    /// Check if this face supports a specific codepoint.
    ///
    /// Uses ttf-parser to query the CMAP table.
    pub fn supports(&self, codepoint: char) -> bool {
        if let Ok(face) = ttf_parser::Face::parse(&self.data, 0) {
            face.glyph_index(codepoint).is_some()
        } else {
            false
        }
    }

    /// Get glyph ID for a codepoint, if supported.
    pub fn glyph_index(&self, codepoint: char) -> Option<u16> {
        if let Ok(face) = ttf_parser::Face::parse(&self.data, 0) {
            face.glyph_index(codepoint).map(|g| g.0)
        } else {
            None
        }
    }
}

/// Main font registry.
///
/// All font resolution pulls from here. Backends never load fonts
/// by name from the OS; they only use LoadedFontFace objects.
#[derive(Debug, Default, Clone)]
pub struct FontManager {
    /// Loaded fonts by font_id
    loaded: HashMap<u64, Arc<LoadedFontFace>>,
    /// Family name -> list of font_ids (for weight/style matching)
    families: HashMap<String, Vec<u64>>,
}

impl FontManager {
    pub fn face_catalog(&self) -> FontFaceCatalog {
        FontFaceCatalog {
            faces: Arc::new(
                self.loaded
                    .iter()
                    .map(|(&id, face)| {
                        let parsed = ttf_parser::Face::parse(&face.data, 0)
                            .expect("admitted face remains immutable");
                        let weight = FontWeight::from_value(parsed.weight().to_number());
                        let italic = parsed.is_italic();
                        let mut resolved = ResolvedFontFace::from_loaded(face);
                        resolved.weight = weight;
                        resolved.italic = italic;
                        (
                            id,
                            CatalogFace {
                                binding: FontFaceBinding::new(
                                    id,
                                    face.data.clone(),
                                    face.family_name.clone(),
                                    weight,
                                    italic,
                                ),
                                shaper: face.shaper.clone(),
                                resolved,
                            },
                        )
                    })
                    .collect(),
            ),
            families: Arc::new(self.families.clone()),
        }
    }
    /// Create a new empty FontManager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a font asset and return the stable font_id.
    ///
    /// Creates the TextShaper and platform handle from the same font data.
    pub fn register_font_asset(&mut self, asset: FontAsset) -> Result<u64, FontLoadError> {
        // Parse font metadata using ttf-parser
        let face =
            ttf_parser::Face::parse(&asset.data, 0).map_err(|_| FontLoadError::InvalidFont)?;

        let postscript_name = face
            .names()
            .into_iter()
            .filter(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
            .find_map(|n| n.to_string())
            .unwrap_or_else(|| asset.family_name.clone());

        let units_per_em = face.units_per_em();

        // Create TextShaper from the same font data
        let shaper = TextShaper::new(asset.data.clone(), units_per_em)
            .ok_or(FontLoadError::ShaperCreationFailed)?;

        // Generate stable font_id
        let font_id = Self::compute_font_id(
            &asset.family_name,
            &postscript_name,
            asset.weight,
            asset.italic,
            &asset.data,
        );

        // Create platform handle (platform-specific)
        let platform_handle = Self::create_platform_handle(asset.data.clone())?;

        // Build coverage map for fallback resolution (SPEC-049)
        let coverage_map = CoverageMap::from_font_data(&asset.data);

        let loaded_face = LoadedFontFace {
            font_id,
            family_name: asset.family_name.clone(),
            postscript_name,
            weight: asset.weight,
            italic: asset.italic,
            data: asset.data,
            units_per_em,
            shaper: Arc::new(shaper),
            platform_handle,
            coverage_map,
        };

        // Store in registry
        let face_arc = Arc::new(loaded_face);
        self.loaded.insert(font_id, face_arc);

        // Update family index
        self.families
            .entry(asset.family_name)
            .or_default()
            .push(font_id);

        Ok(font_id)
    }

    /// Load a font from file path.
    pub fn load_font_file(&mut self, path: &str) -> Result<u64, FontLoadError> {
        let data = std::fs::read(path).map_err(|e| FontLoadError::IoError(e.to_string()))?;
        let data = Arc::new(data);

        // Extract family name from font
        let face = ttf_parser::Face::parse(&data, 0).map_err(|_| FontLoadError::InvalidFont)?;

        let family_name = face
            .names()
            .into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
            .and_then(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Detect weight from OS/2 table
        let weight = face
            .tables()
            .os2
            .map(|os2| FontWeight::from_value(os2.weight().to_number()))
            .unwrap_or(FontWeight::Regular);

        // Detect italic from head table's macStyle field
        // Bit 1 of macStyle indicates italic
        let italic = face.is_italic();

        let asset = FontAsset {
            family_name,
            weight,
            italic,
            data,
        };

        self.register_font_asset(asset)
    }

    /// Load a font from memory.
    pub fn load_font_memory(
        &mut self,
        data: Arc<Vec<u8>>,
        family_name: Option<String>,
    ) -> Result<u64, FontLoadError> {
        let face = ttf_parser::Face::parse(&data, 0).map_err(|_| FontLoadError::InvalidFont)?;

        let family_name = family_name.unwrap_or_else(|| {
            face.names()
                .into_iter()
                .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
                .and_then(|n| n.to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        });

        let weight = face
            .tables()
            .os2
            .map(|os2| FontWeight::from_value(os2.weight().to_number()))
            .unwrap_or(FontWeight::Regular);

        // Detect italic from head table's macStyle field
        let italic = face.is_italic();

        let asset = FontAsset {
            family_name,
            weight,
            italic,
            data,
        };

        self.register_font_asset(asset)
    }

    /// Get a loaded font face by font_id.
    pub fn get(&self, font_id: u64) -> Option<Arc<LoadedFontFace>> {
        self.loaded.get(&font_id).cloned()
    }

    /// Find a font face by family name, weight, and italic.
    ///
    /// Returns the best match from registered fonts.
    pub fn find(
        &self,
        family_name: &str,
        weight: FontWeight,
        italic: bool,
    ) -> Option<Arc<LoadedFontFace>> {
        let font_ids = self.families.get(family_name)?;

        font_ids
            .iter()
            .filter_map(|id| self.loaded.get(id))
            .filter(|face| face.italic == italic)
            .min_by_key(|face| {
                (
                    (face.weight.to_value() as i32 - weight.to_value() as i32).abs(),
                    face.font_id,
                )
            })
            .cloned()
    }

    /// Get all loaded font faces.
    pub fn all_faces(&self) -> impl Iterator<Item = &Arc<LoadedFontFace>> {
        self.loaded.values()
    }

    /// Enumerate all faces (SPEC-049 requirement for fallback logic).
    ///
    /// Alias for `all_faces()`.
    pub fn faces(&self) -> impl Iterator<Item = &Arc<LoadedFontFace>> {
        self.loaded.values()
    }

    /// Look up a face by id (SPEC-049 requirement for renderer/backends).
    ///
    /// Alias for `get()`.
    pub fn get_face(&self, font_id: u64) -> Option<Arc<LoadedFontFace>> {
        self.get(font_id)
    }

    /// Register a font from raw bytes (SPEC-049 required API).
    ///
    /// Convenience method that wraps `load_font_memory`.
    pub fn load_font_bytes(
        &mut self,
        data: Arc<Vec<u8>>,
        family_name: String,
        weight: FontWeight,
        italic: bool,
    ) -> u64 {
        let asset = FontAsset {
            family_name,
            weight,
            italic,
            data,
        };
        self.register_font_asset(asset)
            .expect("Failed to load font from bytes")
    }

    /// Get all registered family names.
    pub fn family_names(&self) -> impl Iterator<Item = &String> {
        self.families.keys()
    }

    /// Compute a stable font_id from font properties (SPEC-049 §10).
    ///
    /// Uses SHA-256 of the full font data for deterministic, cross-platform identity.
    fn compute_font_id(
        family_name: &str,
        postscript_name: &str,
        weight: FontWeight,
        italic: bool,
        data: &[u8],
    ) -> u64 {
        use std::hash::{Hash, Hasher};

        // Compute SHA-256 of full font data
        let mut data_hasher = Sha256::new();
        data_hasher.update(data);
        let data_hash = data_hasher.finalize();

        // Combine all identity components into final hash
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        family_name.hash(&mut hasher);
        postscript_name.hash(&mut hasher);
        weight.to_value().hash(&mut hasher);
        italic.hash(&mut hasher);
        data_hash.as_slice().hash(&mut hasher);
        hasher.finish()
    }

    /// Create platform-specific font handle.
    #[cfg(windows)]
    fn create_platform_handle(data: Arc<Vec<u8>>) -> Result<PlatformFontHandle, FontLoadError> {
        // Create DirectWrite font face from memory
        use dwrote::FontFile;

        let font_file =
            FontFile::new_from_buffer(data).ok_or(FontLoadError::PlatformHandleFailed)?;

        let font_face = font_file
            .create_face(0, dwrote::DWRITE_FONT_SIMULATIONS_NONE)
            .map_err(|_| FontLoadError::PlatformHandleFailed)?;

        Ok(PlatformFontHandle::DirectWrite { font_face })
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn create_platform_handle(_data: Arc<Vec<u8>>) -> Result<PlatformFontHandle, FontLoadError> {
        // TODO: Create CoreText font from memory using CGDataProvider
        Ok(PlatformFontHandle::None)
    }

    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
    fn create_platform_handle(_data: Arc<Vec<u8>>) -> Result<PlatformFontHandle, FontLoadError> {
        // TODO: Create FreeType face from memory using FT_New_Memory_Face
        Ok(PlatformFontHandle::None)
    }

    #[cfg(not(any(windows, target_os = "macos", unix)))]
    fn create_platform_handle(_data: Arc<Vec<u8>>) -> Result<PlatformFontHandle, FontLoadError> {
        Ok(PlatformFontHandle::None)
    }
}

/// Errors that can occur during font loading.
#[derive(Debug, Clone)]
pub enum FontLoadError {
    /// Font data is invalid or corrupted
    InvalidFont,
    /// Failed to create TextShaper from font data
    ShaperCreationFailed,
    /// Failed to create platform-specific font handle
    PlatformHandleFailed,
    /// I/O error while reading font file
    IoError(String),
}

impl std::fmt::Display for FontLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontLoadError::InvalidFont => write!(f, "Invalid or corrupted font data"),
            FontLoadError::ShaperCreationFailed => write!(f, "Failed to create text shaper"),
            FontLoadError::PlatformHandleFailed => {
                write!(f, "Failed to create platform font handle")
            }
            FontLoadError::IoError(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for FontLoadError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_weight_conversion() {
        assert_eq!(FontWeight::Regular.to_value(), 400);
        assert_eq!(FontWeight::Bold.to_value(), 700);
        assert_eq!(FontWeight::from_value(400), FontWeight::Regular);
        assert_eq!(FontWeight::from_value(700), FontWeight::Bold);
        assert_eq!(FontWeight::from_value(450), FontWeight::Medium);
    }

    #[test]
    fn test_font_id_determinism() {
        // Same inputs should always produce same font_id
        let data = b"fake font data for testing";
        let id1 = FontManager::compute_font_id(
            "Inter",
            "Inter-Regular",
            FontWeight::Regular,
            false,
            data,
        );
        let id2 = FontManager::compute_font_id(
            "Inter",
            "Inter-Regular",
            FontWeight::Regular,
            false,
            data,
        );
        assert_eq!(id1, id2, "font_id must be deterministic");

        // Different data should produce different font_id
        let data2 = b"different font data";
        let id3 = FontManager::compute_font_id(
            "Inter",
            "Inter-Regular",
            FontWeight::Regular,
            false,
            data2,
        );
        assert_ne!(id1, id3, "Different data should produce different font_id");

        // Different weight should produce different font_id
        let id4 =
            FontManager::compute_font_id("Inter", "Inter-Bold", FontWeight::Bold, false, data);
        assert_ne!(
            id1, id4,
            "Different weight should produce different font_id"
        );

        // Different italic should produce different font_id
        let id5 =
            FontManager::compute_font_id("Inter", "Inter-Italic", FontWeight::Regular, true, data);
        assert_ne!(
            id1, id5,
            "Different italic should produce different font_id"
        );
    }

    #[test]
    fn test_coverage_map_basic() {
        let mut map = CoverageMap::new();
        assert!(!map.is_populated());
        assert_eq!(map.coverage_count(), 0);
        assert!(!map.covers('A'));

        map.mark_supported('A');
        map.mark_supported('B');
        map.mark_supported('C');

        assert!(map.is_populated());
        assert_eq!(map.coverage_count(), 3);
        assert!(map.covers('A'));
        assert!(map.covers('B'));
        assert!(map.covers('C'));
        assert!(!map.covers('D'));
    }

    #[test]
    fn test_font_family_enum() {
        let inter = FontFamily::named("Inter");
        assert_eq!(inter, FontFamily::Named("Inter".to_string()));

        let sans = FontFamily::SansSerif;
        assert!(matches!(sans, FontFamily::SansSerif));

        let serif = FontFamily::Serif;
        assert!(matches!(serif, FontFamily::Serif));

        let mono = FontFamily::Monospace;
        assert!(matches!(mono, FontFamily::Monospace));

        let system = FontFamily::SystemUI;
        assert!(matches!(system, FontFamily::SystemUI));
    }

    #[test]
    fn test_font_stack_operations() {
        let empty = FontStack::new();
        assert!(empty.is_empty());

        let single = FontStack::single(FontFamily::named("Inter"));
        assert!(!single.is_empty());
        assert_eq!(single.iter().count(), 1);

        let multi = FontStack::from_families(vec![
            FontFamily::named("Inter"),
            FontFamily::SansSerif,
            FontFamily::SystemUI,
        ]);
        assert_eq!(multi.iter().count(), 3);

        let chained = FontStack::single(FontFamily::named("Roboto"))
            .with_fallback(FontFamily::SansSerif)
            .with_fallback(FontFamily::SystemUI);
        assert_eq!(chained.iter().count(), 3);
    }

    #[test]
    fn test_font_descriptor() {
        let desc = FontDescriptor::new(FontFamily::named("Inter"), FontWeight::Bold, true);
        assert_eq!(desc.family, FontFamily::Named("Inter".to_string()));
        assert_eq!(desc.weight, FontWeight::Bold);
        assert!(desc.italic);
    }

    #[test]
    fn test_font_collection() {
        let mut collection = FontCollection::new("Inter");
        assert_eq!(collection.family, "Inter");
        assert!(collection.faces.is_empty());

        collection.add_face(FontAsset {
            family_name: "Inter".to_string(),
            weight: FontWeight::Regular,
            italic: false,
            data: Arc::new(vec![]),
        });

        assert_eq!(collection.faces.len(), 1);
    }

    #[test]
    fn test_font_manager_new() {
        let manager = FontManager::new();
        assert_eq!(manager.faces().count(), 0);
        assert_eq!(manager.family_names().count(), 0);
    }
}
