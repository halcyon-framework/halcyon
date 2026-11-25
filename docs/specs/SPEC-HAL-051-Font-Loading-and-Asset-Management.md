# **SPEC-HAL-051: Font Loading, Runtime Injection & Font Asset Management**

**Unified Cross-Backend Font Pipeline**

**Status:** Implemented
**Author:** JC

Scoped addition (delegated technical A1 acceptance, 2026-10-10): HAL-048's dated
UI typography contract admits optional immutable FontAssets through App bootstrap
before the first Screen build/measurement. Existing omitted configuration keeps
the same SystemUI bytes/identity. The catalog transfers owned bytes/metadata,
not LoadedFontFace platform handles; native owners initialize matching faces from
those bytes. No paint-time loading/registration, new asset, hot swap or worker is
admitted. Implementation pending source proof and independent review.
**Depends on:**

* **SPEC-HAL-006** (Text & Font System)
* **SPEC-HAL-047** (Native Text Rendering Backends)
* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-049** (Font Collections & Fallback Resolution)
* **SPEC-HAL-065** (HarfBuzz Complex Text Shaping via rustybuzz)

**Planned follow-ups:**

* SPEC-HAL-052 (Interactive Text Editing)
* SPEC-HAL-054 (TextField Architecture)
* SPEC-HAL-060 (RichText / inline spans)

---

# **1. Purpose**

This SPEC defines the **single, universal font pipeline** for Halcyon:

* Loading fonts from **file**, **memory**, **embedded assets**, **WASM bundles**, **TTC collections**
* Registering fonts into the engine at runtime
* Hot-swapping fonts without restarting
* Creating **LoadedFontFace** objects with complete cross-platform metadata
* Providing byte-for-byte consistent input to:

    * rustybuzz (shaping)
    * NativeFontBackend (rasterization)

The FontManager defined here becomes the *only* mechanism for acquiring fonts across all platforms.

---

# **2. Problems This Solves**

Without a unified pipeline:

* DirectWrite may rasterize a different font than rustybuzz shapes → **glyph ID mismatch**
* macOS CoreText may pick another weight/style → **broken metrics**
* Linux FreeType may use fallback faces not known to the engine
* WASM cannot access OS fonts at all
* Fallback is inconsistent across platforms
* Custom fonts cannot be loaded deterministically
* ParagraphEngine cannot trust font identity or coverage

A real-world consequence you already saw:

**Arabic text shaped using “Inter” but rasterized as Segoe UI → garbage output.**

This SPEC fixes that permanently.

---

# **3. Guarantees**

The pipeline defined here guarantees:

### **3.1 Same bytes for shaping & rasterization**

rustybuzz and DirectWrite/CoreText/FreeType always use the exact same `Arc<Vec<u8>>`.

### **3.2 Deterministic font identity**

`font_id` is stable across processes and platforms.

### **3.3 Complete font lifetime control**

The engine owns all font data; no backend may load its own fonts.

### **3.4 Hot-swapping**

Fonts can be:

* added
* removed
* replaced
* updated in memory
* swapped between families

### **3.5 WASM compatibility**

No platform APIs required; only pure Rust.

---

# **4. Core Data Structures**

## **4.1 FontAsset**

Raw font resource.

```
pub struct FontAsset {
    pub family_name: String,
    pub weight: FontWeight,
    pub italic: bool,
    pub data: Arc<Vec<u8>>,
}
```

Source-agnostic: may come from file, from memory, from embedded resource.

## **4.2 FontCollection**

Logical family grouping.

```
pub struct FontCollection {
    pub family: String,
    pub faces: Vec<FontAsset>,
}
```

A family may have multiple weights/styles.

## **4.3 LoadedFontFace**

Fully constructed font with platform + shaper state.

```
pub struct LoadedFontFace {
    pub font_id: u64,

    pub family_name: String,
    pub postscript_name: String,
    pub weight: FontWeight,
    pub italic: bool,

    pub data: Arc<Vec<u8>>,
    pub units_per_em: u16,

    pub shaper: Arc<TextShaper>,              // rustybuzz shaping (SPEC-065)
    pub platform_handle: PlatformFontHandle,  // rasterization
    pub coverage_map: CoverageMap,            // for fallback (SPEC-049)
}
```

**This is the only object backend code ever interacts with.**

Note: `TextShaper` wraps rustybuzz::Face and provides the shaping API per SPEC-065.

## **4.4 PlatformFontHandle**

```
pub enum PlatformFontHandle {
    #[cfg(windows)]
    DirectWrite { font_face: dwrote::FontFace },
    #[cfg(target_os = "macos")]
    CoreText { /* CTFont from CGDataProvider */ },
    #[cfg(all(unix, not(target_os = "macos")))]
    FreeType { /* FT_Face from FT_New_Memory_Face */ },
    None,  // Fallback for platforms without native handle
}
```

Platform handles are conditionally compiled. `None` is used when platform support is incomplete or for WASM (future).

## **4.5 FontManager**

The main registry.

```
pub struct FontManager {
    loaded: HashMap<u64, Arc<LoadedFontFace>>,
    families: HashMap<String, Vec<u64>>,  // family name → font_ids
}
```

All resolution (SPEC-049) pulls from here. Uses `Arc` for shared ownership across threads.

---

# **5. Font Loading API**

The engine must support loading from:

* File path
* Memory bytes
* WASM embedded resources
* Runtime user injection
* TTC index selection

### **5.1 Load from file**

```
fn load_font_file(&mut self, path: &str) -> Result<u64, FontLoadError>;
```

Extracts metadata from font and registers it. Returns `font_id`.

Extracts:

* family name
* weight
* italic
* postscript name
* units_per_em

### **5.2 Load from memory**

```
fn load_font_memory(
    &mut self,
    data: Arc<Vec<u8>>,
    family_name: Option<String>,
) -> Result<u64, FontLoadError>;
```

Loads from bytes, optionally overriding family name. Returns `font_id`.

### **5.3 Load with explicit metadata**

```
fn load_font_bytes(
    &mut self,
    data: Arc<Vec<u8>>,
    family_name: String,
    weight: FontWeight,
    italic: bool,
) -> u64;
```

Convenience method with explicit metadata (panics on failure).

### **5.4 Individual face registration**

```
fn register_font_asset(&mut self, asset: FontAsset) -> Result<u64, FontLoadError>;
```

Core registration. Returns a stable `font_id` or error.

---

# **6. Creating a LoadedFontFace**

When a FontAsset is registered, we do:

### **6.1 Parse metadata**

Using ttf-parser:

```
let face = ttf_parser::Face::parse(&asset.data, 0)?;
let postscript = face.post_script_name();
let upem = face.units_per_em();
```

### **6.2 Create TextShaper**

```
let shaper = TextShaper::new(asset.data.clone(), units_per_em)?;
```

TextShaper wraps rustybuzz::Face internally (SPEC-065).

### **6.3 Create coverage map**

Precompute coverage (Unicode block/bitset).

### **6.4 Create platform handle**

* **Windows** → IDWriteFontFace from memory
* **macOS** → CTFontRef with CGDataProvider
* **Linux** → FT_New_Memory_Face
* **WASM** → upload bytes to JS / CanvasKit / FontFace API

### **6.5 Create font_id**

```
hash64(
    family_name,
    postscript_name,
    weight,
    italic,
    SHA256(data)
)
```

### **6.6 Store LoadedFontFace**

```
loaded.insert(font_id, LoadedFontFace {...});
```

---

# **7. Hot-Swapping**

## **7.1 Add new font**

```
FontManager::register_font_asset(asset)
```

ParagraphEngine receives invalidation event and recalculates layout.

## **7.2 Replace a font**

Remove old `font_id`, insert new one.

## **7.3 Remove a font**

* Remove LoadedFontFace
* Remove from family
* ParagraphEngine must degrade gracefully to fallback

---

# **8. Integration with SPEC-049 (FontResolver)**

FontResolver never touches OS:

* Never calls DirectWrite font enumeration
* Never calls CoreText font matching
* Never uses FontConfig
* Never loads fonts by name

It works only with:

```
FontManager::families
FontManager::loaded
coverage maps
```

Missing-glyph fallback uses:

```
LoadedFontFace.coverage_map
```

---

# **9. Integration with NativeFontBackend**

Backends must:

* Accept only LoadedFontFace
* Never load fonts by name
* Never do fallback
* Never consult platform APIs to choose fonts

Rasterization uses:

```
font.platform_handle
```

Shaping uses:

```
font.shaper  // TextShaper with rustybuzz
```

Always consistent.

---

# **10. WASM behavior**

WASM support is planned but not yet implemented. When implemented:

* `PlatformFontHandle::None` serves as placeholder
* JS `FontFace` API or CanvasKit integration
* Fallback remains engine-controlled, not browser-controlled

No browser fallback chains allowed.

---

# **11. Implementation Phases**

### **Phase 1 — Core structures**

Implement FontManager, FontAsset, LoadedFontFace, PlatformFontHandle.

### **Phase 2 — File + memory loaders**

Stable metadata extraction.

### **Phase 3 — Platform handles**

Implement all 4 platform branches.

### **Phase 4 — CoverageMap**

For script-aware fallback (needed by SPEC-049).

### **Phase 5 — Replace Glyphon/fontdb**

Complete removal.

### **Phase 6 — Integrate with ParagraphEngine**

Runs generated from LoadedFontFace.

### **Phase 7 — Hot-swap support**

Live font updates.

---

# **12. Tests**

* Load/shape/rasterize same glyph across all platforms → identical output
* Emoji + CJK fallback
* Arabic/Hebrew shaping using fallback faces
* Deterministic `font_id` across machines
* WASM consistency
* Memory-only fonts

---

# **13. Security Notes**

* Validate TTF/OTF table offsets before exposing bytes to backend
* rustybuzz is memory-safe, but malformed fonts must not lead to UB in FreeType
* Custom fonts should be loaded from trusted sources in production
* Prevent hot-swap race conditions by using Arc throughout

---

# **14. Implementation Notes**

Implemented in `src/platform/font_manager.rs` and `src/platform/font_resolver.rs`.

## **14.1 Core Structures**

* `FontAsset` - Raw font resource with family_name, weight, italic, data
* `FontCollection` - Logical grouping (family → faces), used for bulk registration
* `LoadedFontFace` - Complete font with:
  - `shaper: Arc<TextShaper>` for rustybuzz shaping (SPEC-065)
  - `platform_handle: PlatformFontHandle` for rasterization
  - `coverage_map: CoverageMap` for fallback resolution (SPEC-049)
* `FontManager` - Registry with:
  - `loaded: HashMap<u64, Arc<LoadedFontFace>>`
  - `families: HashMap<String, Vec<u64>>` (family index)
* `PlatformFontHandle` - Enum with platform-specific variants + `None` fallback
* `ResolvedFontFace` - Lightweight subset for resolution results (SPEC-049)

## **14.2 Font Loading Methods**

* `register_font_asset(FontAsset) -> Result<u64, FontLoadError>` - Core registration
* `load_font_file(&str) -> Result<u64, FontLoadError>` - Auto-detect metadata
* `load_font_memory(Arc<Vec<u8>>, Option<String>) -> Result<u64, FontLoadError>` - From bytes
* `load_font_bytes(Arc<Vec<u8>>, String, FontWeight, bool) -> u64` - Explicit metadata

## **14.3 Query Methods**

* `get(font_id)` / `get_face(font_id)` - Lookup by ID → `Option<Arc<LoadedFontFace>>`
* `find(family_name, weight, italic)` - Style-based lookup with weight approximation
* `faces()` / `all_faces()` - Iterate all loaded faces
* `family_names()` - Iterate registered family names

## **14.4 Platform Handle Creation**

Windows DirectWrite implemented via:
```rust
dwrote::FontFile::new_from_buffer(data)
font_file.create_face(0, DWRITE_FONT_SIMULATIONS_NONE)
```

macOS CoreText and Linux FreeType return `PlatformFontHandle::None` (stub).

## **14.5 Integration with SPEC-049**

* `CoverageMap::from_font_data()` builds coverage at load time
* `font_id` uses SHA-256 of full font data for cross-platform stability
* `FontResolver` borrows `&FontManager` for pure resolution logic
* `ResolvedFontFace::from_loaded()` extracts lightweight reference for resolution

## **14.6 Integration with SPEC-065**

* `TextShaper` wraps rustybuzz::Face with Halcyon's shaping API
* Created via `TextShaper::new(data, units_per_em)`
* Provides `shape()`, `shape_auto()`, `measure_width()`
