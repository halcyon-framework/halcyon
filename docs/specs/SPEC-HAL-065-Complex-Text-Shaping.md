# **SPEC-HAL-065 — Complex Text Shaping (rustybuzz Integration)**

**Cross-Platform Unicode Shaping for All Scripts**

**Status:** Draft
**Author:** JC
**Depends on:**

* **SPEC-HAL-049** (Font Resolution & Fallback)
* **SPEC-HAL-051** (Unified Font Loading & Platform Font Handles)
* **SPEC-HAL-048** (Paragraph & Text Layout)
* **SPEC-HAL-047** (Native Text Rendering Backends)

**Provides:**

* Complete Unicode-correct shaping
* RTL handling
* Ligatures
* Marks & reordering
* Cluster mapping
* Script detection
* Complex scripts (Arabic, Hebrew, Indic, Thai, etc.)
* Emoji sequences (ZWJ)
* Direction resolution

---

# **1. Purpose**

Shaping = turning text (`&str`) into **glyph clusters** with exact positions in physical pixels.

This SPEC defines:

1. The **engine’s unified shaping pipeline**, used on all platforms
2. **rustybuzz** as the official shaping engine (pure Rust HarfBuzz)
3. Interaction with **FontManager** and **LoadedFontFace**
4. Correct behavior for:

    * Arabic contextual forms
    * Hebrew RTL
    * Devanagari reordering
    * Thai tone marks
    * ZWJ emoji sequences
    * Mixed LTR/RTL
    * Multi-font runs

Shaping runs **before** rasterization.
Native backends no longer do shaping.

---

# **2. Why rustybuzz**

rustybuzz is:

* Official HarfBuzz port
* Pure Rust
* No C ABI
* No native dependencies
* WASM-safe
* Byte-for-byte identical shaping behavior to HarfBuzz
* Works directly with `Arc<Vec<u8>>` from SPEC-051

Exactly what we need.

---

# **3. Invariants**

### **3.1 Font invariant**

Shaper must always receive:

```
LoadedFontFace.data  // exact same bytes as backend rasterizer uses
LoadedFontFace.units_per_em
LoadedFontFace.hb_face
```

No OS-provided fonts.
No “pick by name”.
No “query system font”.

### **3.2 Run invariant**

Each shaping run uses exactly **one** LoadedFontFace.

If missing glyphs are detected:

* SPEC-049 fallback logic splits a new run with a fallback face.

### **3.3 Coordinate invariant**

rustybuzz outputs **design-unit values**.
We convert to physical pixels:

```
scale = (font_size * device_scale) / units_per_em
x_offset_phys = pos.x_offset * scale
advance_phys  = pos.x_advance * scale
```

Consistent across all platforms.

---

# **4. Public API**

```
pub struct ShapedCluster {
    pub glyph_id: u32,
    pub x_offset_phys: f32,
    pub y_offset_phys: f32,
    pub advance_phys: f32,
    pub cluster: u32,
}
```

### **TextShaper**

```
pub struct TextShaper {
    face: rustybuzz::Face<'static>,
    units_per_em: u16,
}
```

Constructed from LoadedFontFace.

### **Methods**

```
pub fn shape(
    &self,
    text: &str,
    font_size: f32,
    device_scale: f32,
    direction: TextDirection,
) -> Vec<ShapedCluster>;
```

Direction comes from SPEC-049 / ParagraphEngine.

---

# **5. Direction & Script Detection**

### **5.1 Direction Resolution**

`TextDirection` enum:

```
LeftToRight
RightToLeft
```

SPEC-065 disallows TTB (Top-to-Bottom); not relevant for UI.

Direction detection rules:

* First **strong** directional character decides
* Arabic/Hebrew → RTL
* Latin/Cyrillic/Greek → LTR

ParagraphEngine may override direction for explicit style.

### **5.2 rustybuzz buffer setup**

```
let mut buf = UnicodeBuffer::new();
buf.set_direction(Direction::LeftToRight or RightToLeft);
buf.push_str(text);
```

No script guessing from HarfBuzz—rustybuzz handles subtables automatically.

---

# **6. Shaping Pipeline**

### **6.1 Overview**

```
ParagraphEngine
    ↓ resolve run (SPEC-049)
LoadedFontFace
    ↓
TextShaper (rustybuzz)
    ↓ shaping
Vec<ShapedCluster>
    ↓
NativeFontBackend rasterization
```

### **6.2 Steps**

1. Convert style → LoadedFontFace (via FontResolver)
2. Create TextShaper for that face
3. Build UnicodeBuffer
4. Apply direction
5. Call `rustybuzz::shape(face, features, buffer)`
6. Convert design units → physical pixels
7. Emit ShapedCluster sequence

### **6.3 No font fallback in rustybuzz**

Fallback is upstream in SPEC-049.
Each cluster belongs to exactly one face.

---

# **7. RTL Output Rules**

For RTL:

* Advancements remain positive (physical pixels)
* Rendering order is controlled in ParagraphEngine
* Runs are reversed at glyph order level if necessary
* Logical clustering preserved

rustybuzz guarantees correct cluster ordering.

---

# **8. ZWJ & Emoji Handling**

rustybuzz fully supports:

* ZWJ sequences
* Skin-tone modifiers
* Regional indicator pairs

No extra logic required.

We rely on:

* coverage maps (SPEC-051)
* fallback (SPEC-049)

If emoji unsupported → fallback to color glyph font → fallback to BW glyph font.

---

# **9. Integration With Backends**

### **9.1 rasterization must use LoadedFontFace.platform_handle**

Glyph ID is correct only if rasterizer uses same font.

Backends must **not** shape.
Backends must **not** pick fonts by name.

### **9.2 Exact contract**

Shaper outputs glyph IDs.
Rasterizer receives glyph IDs **unchanged**.

Backends must only map:

```
glyph_id → glyph bitmap
```

using their platform handle (DirectWrite/CT/FT).

---

# **10. Missing-Glyph Fallback Logic**

Placed in SPEC-049:

* rustybuzz doesn’t do fallback
* ParagraphEngine checks coverage map
* On missing glyph → split run
* Repeat shaping with fallback face

rustybuzz receives clean runs with guaranteed coverage.

---

# **11. WASM Behavior**

Fully supported:

* No native HarfBuzz needed
* No C library
* No system fonts
* No dynamic linking

PlatformFontHandle::WasmWebFont handles rasterization via:

* CanvasKit
* Canvas2D
* JS FontFace
* WebGL text pipeline

Shaping always uses TextShaper.

---

# **12. Implementation Plan**

### **Phase 1 — Core Types**

* ShapedCluster
* TextDirection
* TextShaper

### **Phase 2 — rustybuzz Integration**

* Implement TextShaper from LoadedFontFace
* API fully independent of platform

### **Phase 3 — ParagraphEngine Integration**

* Runs produced per LoadedFontFace
* Missing-glyph fallback wired correctly

### **Phase 4 — Backends**

* Remove backend-level shaping (DirectWrite/CT)
* Restrict backends to rasterization only
* Map glyph IDs directly

### **Phase 5 — Complex Script Test Suite**

* Arabic
* Hebrew
* Devanagari
* Thai
* Bengali
* Mixed Latin/RTL
* Emoji sequences

---

# **13. Testing**

### **Shaping Tests**

* Arabic joining
* Indic matra reordering
* Kerning
* ZWJ emoji
* Digit mixing
* Hebrew diacritics
* Clusters trimmed at run boundaries

### **Metrics Tests**

* Compare shaped advances vs reference HarfBuzz
* RTL metrics parity
* Roundtrip frame rendering

### **Fallback Tests**

* Latin + CJK
* Arabic + Latin
* Emoji fallback if primary font lacks color glyphs

---

# **14. Performance**

* rustybuzz is extremely fast
* pure Rust → no FFI overhead
* shaping cost amortized via caching at ParagraphEngine
* no backend extra passes
* run splitting minimal

---

# **15. Security**

* Input font bytes validated in SPEC-051
* rustybuzz is memory-safe
* No raw pointer exposure
* No platform shaping APIs invoked
