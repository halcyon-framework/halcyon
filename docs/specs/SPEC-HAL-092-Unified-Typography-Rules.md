# **SPEC-HAL-092 — Unified Typography Rules for Halcyon UI**

**SPEC-HAL-092**
**Unified Typography Rules for Halcyon UI**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 2.0

**Depends On:**

* SPEC-HAL-006 — Text & Font System
* SPEC-HAL-024 — Coordinate Model
* SPEC-HAL-048 — Text Layout Engine
* SPEC-HAL-066 — Unified Glyph Raster Pipeline
* SPEC-HAL-067 — Subpixel & SDF Text Rendering v2
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules

**Implemented By:**

* SPEC-HAL-104 — Pixel-Stable Text Metrics (physical realization of this grid)

---

## **Implementation Status**

*Not implemented.*

This SPEC defines the **typographic foundation of Halcyon**, covering:

* font stack
* unified baseline rhythm
* line-height rules
* font size tokens
* DPI scaling
* optical alignment
* control text placement
* cross-platform rendering consistency

Typography becomes a **first-class layout primitive**, not an afterthought.

---

# **1. Motivation**

Halcyon is an engineer-class UI system.
Typography must support:

* high-density dashboards
* tree-heavy inspectors
* data tables
* editors and terminals
* multi-window professional apps

Modern UI frameworks often fail because:

* line-height is inconsistent
* baseline grid is absent
* DPI scaling breaks geometry
* fonts jitter on animation
* text alignment is "eyeballed"

Halcyon must provide a **precise, deterministic, compact typographic model** tied directly to the Logical Grid (SPEC-HAL-090).

---

# **2. Font Stack**

Halcyon defines canonical font stacks for all platforms.

### **2.1 UI Font (Sans-Serif)**

```
Inter, SF Pro Text, Segoe UI, system-ui, sans-serif
```

| Priority | Font         | Platform      | Notes                      |
|----------|--------------|---------------|----------------------------|
| 1        | Inter        | Cross-platform| Preferred; ship with app   |
| 2        | SF Pro Text  | macOS/iOS     | System font                |
| 3        | Segoe UI     | Windows       | System font                |
| 4        | system-ui    | Any           | Platform default           |
| 5        | sans-serif   | Any           | Ultimate fallback          |

### **2.2 Monospace Font**

```
JetBrains Mono, SF Mono, Cascadia Code, Consolas, monospace
```

| Priority | Font           | Platform      | Notes                      |
|----------|----------------|---------------|----------------------------|
| 1        | JetBrains Mono | Cross-platform| Preferred; ship with app   |
| 2        | SF Mono        | macOS/iOS     | System monospace           |
| 3        | Cascadia Code  | Windows 10+   | Windows Terminal default   |
| 4        | Consolas       | Windows       | Legacy system mono         |
| 5        | monospace      | Any           | Ultimate fallback          |

### **2.3 Font Loading**

* Fonts are loaded at application startup
* Missing fonts trigger fallback chain silently
* No runtime errors for missing fonts
* Metrics are normalized per SPEC-HAL-104 regardless of which font loads

### **2.4 Font Weights**

| Weight | Value | Usage                            |
|--------|-------|----------------------------------|
| Regular| 400   | Body text, labels, data          |
| Medium | 500   | Emphasized labels, section heads |
| SemiBold| 600  | Panel titles, dialog headers     |
| Bold   | 700   | Rarely used; strong emphasis     |

Halcyon avoids weight extremes (100-300, 800-900).

---

# **3. Baseline Grid**

Halcyon typography aligns to a **4-unit logical baseline grid**:

```
1 unit = 1 px @ 96 DPI
Baseline step = 4 units
```

### 3.1 Requirements

1. **Every line-height MUST be a multiple of 4 logical units.**
2. **Every glyph baseline MUST align to this grid in logical space.**
3. **No fractional vertical placement in logical space.**

### 3.2 Example Mappings

```
11px font → 12px line-height (3×4)
12px font → 16px line-height (4×4)
13px font → 16px line-height (4×4)
14px font → 20px line-height (5×4)
16px font → 20px line-height (5×4)
18px font → 24px line-height (6×4)
```

### 3.3 Grid Benefits

This ensures:

* stable scrolling
* no "dancing text"
* predictable layout
* excellent readability in dense UI

### 3.4 Logical-Space Only Rule

Typography tokens (T-XS, T-S, T-M, etc.) operate purely in logical coordinates. They MUST NOT depend on physical pixel values, DPI scale, or device characteristics. Physical snapping is exclusively the domain of SPEC-HAL-104.

### 3.5 Physical Realization (SPEC-HAL-104)

Physical font metrics MUST be normalized via SPEC-HAL-104 so that the actual physical ascent/descent/line-gap result in a `line_height_logical` that satisfies this grid constraint. If the font's raw metrics do not align exactly, SPEC-HAL-104 defines how to snap them to the nearest acceptable value.

See SPEC-HAL-104 Section 4, Rule 2 for the complete normalization algorithm.

---

# **4. Typography Tokens**

Halcyon defines canonical typographic roles:

| Token    | Size | Line Height | Weight | Typical Use               |
|----------|------|-------------|--------|---------------------------|
| **T-XS** | 11px | 12px        | 400    | Status bar, timestamps    |
| **T-S**  | 12px | 16px        | 400    | Labels, metadata, hints   |
| **T-M**  | 13px | 16px        | 400    | Primary UI text, data     |
| **T-L**  | 14px | 20px        | 500    | Section headers           |
| **T-XL** | 16px | 20px        | 600    | Panel titles, dialogs     |
| **T-2XL**| 18px | 24px        | 600    | Large headings (rare)     |

### 4.1 Token Requirements

Each token defines:

* `font_size_logical` — the font size in logical px
* `line_height_logical` — MUST be a multiple of 4 logical px
* `font_weight` — numeric weight value

### 4.2 Monospace Tokens

For code, terminals, and numeric data:

| Token     | Size | Line Height | Usage                      |
|-----------|------|-------------|----------------------------|
| **Mono-XS** | 11px | 12px      | Dense logs, hex dumps      |
| **Mono-S**  | 12px | 16px      | Code editor, terminal      |
| **Mono-M**  | 13px | 16px      | Property values, data grid |

### 4.3 Mixed Font Handling

When fallback fonts (emoji, symbols, alternate scripts) are used within a typography token context, the `line_height_logical` MUST NOT change. Fallback glyphs adjust their internal positioning within the established line box but do not alter the grid.

### 4.4 Ephemeral Layout Rule

Typography tokens MUST NOT store shaped runs, paragraph layouts, or cached line metrics across frames. All shaped/layout data is ephemeral per SPEC-HAL-002.

---

# **5. DPI Behavior**

Typography is not raster-scaled by Halcyon.
Fonts follow:

```
physical_font_px = logical_font_px × dpi_scale
physical_line_height = logical_line_height × dpi_scale
```

Where:

```
dpi_scale = monitor_dpi / 96
```

This guarantees:

* no blurry glyphs
* no off-grid placement
* consistent rhythm across high-density displays
* identical proportions across systems

---

# **6. Cross-Platform Rendering Rules**

Halcyon UI must produce **identical geometry** on:

* DirectWrite (Windows)
* CoreText (macOS)
* FreeType (Linux/BSD)

### Requirements:

1. **Glyph metrics are normalized.**
   Differences in ascent/descent are unified in logical space.

2. **Baseline offset is platform-independent.**
   CoreText's optical baseline and DirectWrite's mathematical baseline converge.

3. **Subpixel and grayscale rasterization adhere to a unified contrast profile.**

4. **Text shaping uses the same HarfBuzz path across all platforms.**

5. Layout engine consumes **logical metrics only**, ignoring platform-specific physical ones.

This eliminates the cross-platform jitter common in Qt, GTK, and WPF.

---

# **7. Optical Margin & Alignment Rules**

Typography must visually align with UI geometry, not just mathematically.

### 7.1 Label Alignment

```
Label baseline aligns to input text baseline + optical offset
```

Ensures:

* label isn't optically "low"
* classic desktop alignment (like Win32 inspectors)

### 7.2 Icon-Text Alignment

Icons must:

```
align to text optical center, not bounding-box center
```

The optical center is approximately:

```
baseline − (x_height / 2)
```

### 7.3 Paragraphs

* leading-trim applied automatically
* no accidental extra top whitespace
* consistent bottom-trim for multi-line blocks

### 7.4 Headings

* above spacing: S-M (8U) or S-L (12U)
* below spacing: S-S (6U)

This creates natural vertical rhythm.

---

# **8. Control Text Placement**

Buttons, inputs, dropdowns, tags — all use consistent padding rules:

```
vertical padding   = P-XS (4U)
horizontal padding = P-S  (6U)
```

**DEPRECATED:** The formula below is superseded by Section 8A for control text placement.

Optical vertical centering (legacy, for layout spacing only):

```
text_y = (control_height − line_height) / 2 + baseline_correction
```

Where `baseline_correction` is a font-specific offset (typically 0–2px) to achieve optical centering.

This prevents:

* upward text drift
* uneven perceived padding
* jitter during animated resizing

This is one of the core foundations for **pixel-perfect compact UI**.

---

# **8A. Control Text Placement Invariant (Binding)**

**Status:** BINDING — All controls MUST implement this.

The formula in Section 8 centers based on `line_height`, but the renderer positions
glyphs using `ascent`. This mismatch causes glyphs to visually touch or cross the
control's top edge even when the math appears correct.

### 8A.1 Invariants

**INV-TYPO-CTRL-1: Metrics-Based Placement**
Control text vertical placement MUST be computed from actual font metrics
(ascent + descent), not from line_height.

**INV-TYPO-CTRL-2: Glyph Containment**
The glyph bounding box (ascent + descent) MUST be fully contained within the
control's content rect.

**INV-TYPO-CTRL-3: line_height Prohibition**
`line_height` is a layout spacing metric only and MUST NOT be used to position
glyphs vertically inside controls.

**INV-TYPO-CTRL-4: Baseline-Anchored Computation**
All controls MUST place text via a baseline-anchored computation.
Top-left text placement derived from line_height is architecturally invalid.

### 8A.2 Canonical Formula

```rust
/// Compute baseline Y for control text placement.
fn compute_control_text_baseline_y(
    control_height: f32,
    ascent: f32,
    descent: f32,
    baseline_offset: f32,
) -> f32 {
    // Center actual glyph box (ascent + descent), not line_height
    let glyph_height = ascent + descent;
    let top = (control_height - glyph_height) * 0.5;
    // Baseline = top of glyph box + ascent
    top + ascent + baseline_offset
}
```

### 8A.3 Usage Pattern

```rust
let metrics = cx.text_engine.font_metrics(font_size);
let baseline_y = rect.y + compute_control_text_baseline_y(
    rect.height,
    metrics.ascent,
    metrics.descent,
    cx.theme.typography.baseline_offset,
);
// draw() takes top-of-text-box, so subtract ascent from baseline
let text_top_y = baseline_y - metrics.ascent;
cx.text_engine.draw(&run, x, text_top_y, color, text_z, scene);
```

### 8A.4 Deprecation

Any formula of the form `(control_height - line_height) / 2 + offset` is
**deprecated** for control text placement. It is retained only for:

* Layout spacing calculations (line box stacking, multiline flow)
* Non-control contexts where line_height is the correct abstraction

### 8A.5 Implementation Reference

See `src/core/text_engine.rs`:
* `compute_control_text_baseline_y()` — canonical helper (REQUIRED)
* `compute_control_text_top_y()` — deprecated for controls

---

# **9. Interaction With Layout System**

Typography interacts with:

**Layout Engine (HAL-008)**

* every text node provides: ascent, descent, baseline, line-height

**Spacing Tokens (HAL-090)**

* tokens use the same logical grid as typography

**Density Modes (HAL-091)**

* font sizes scale via mode multipliers
* line-height remains locked to baseline grid (round to nearest 4U)

**Widget System (HAL-010)**

* every widget consumes type tokens indirectly via the theme
* spacing + typography is unified in one geometry model

**Component Geometry (HAL-094)**

* control heights derive from typography tokens + padding
* all measurements must be self-consistent

---

# **10. Rendering Pipeline Integration**

Typography requires:

### 10.1 Snap-to-Pixel Grid

When DPI scale is fractional (e.g. 1.25):

```
logical coords remain fractional
physical coords snap to appropriate pixel grid
```

Ensures minimal blur.

### 10.2 Baseline Grid & Physical Pixel Alignment

The baseline grid is defined in logical coordinates. When mapped to physical pixels:

1. **Block origins** are snapped to the nearest physical pixel positions as per SPEC-HAL-104 Stage F.
2. **Baselines within blocks** are computed from normalized metrics and then snapped per SPEC-HAL-104 Rule 5.

This snapping is **global**: it applies to all text blocks in the system, not only specific widgets.

The snapping does not violate the logical baseline grid — it aligns the grid to the actual framebuffer pixel grid for crisp rendering.

### 10.2.1 Phase Separation Rule

Baseline grid snapping is applied during Phase 4 (Layout). It MUST NOT alter widget identity resolution (Phase 3) or cause any state mutation.

### 10.3 Glyph Atlas Behavior

* Atlas always stores glyphs in physical pixels
* Changes to DPI or UI scale regenerate relevant glyphs
* Subpixel bins (SPEC-HAL-066) ensure stable atlas reuse across frames
* SDF and MSDF render modes use fixed bin=Zero (SPEC-HAL-067)

### 10.4 Subpixel AA Compatibility

Halcyon must support:

* grayscale AA
* LCD AA (with per-bin rasterization per SPEC-HAL-067)
* SDF AA

All MUST obey baseline constraints defined in this SPEC and pixel-stability rules defined in SPEC-HAL-104.

---

# **11. Text Color Roles**

Typography colors are defined in SPEC-HAL-096.

| Role           | Usage                              |
|----------------|------------------------------------|
| Text-Primary   | Body text, data, labels            |
| Text-Secondary | Hints, metadata, descriptions      |
| Text-Muted     | Disabled text, placeholders        |
| Text-Inverse   | Text on accent backgrounds         |

Widgets consume these semantic tokens; they never specify raw colors.

---

# **12. Future Extensions**

* variable font optical sizing
* dynamic font-weight mapping
* typographic themes
* responsive typography for multi-window apps
* advanced paragraph shaping with ligature policies

---

# **13. Implementation Plan**

### **Phase 1 — Core Metrics**

* unify logical ascent/descent
* create typography token table
* enforce 4-unit grid in text layout

### **Phase 2 — Font Stack**

* bundle Inter and JetBrains Mono
* implement fallback chain
* normalize metrics across fonts

### **Phase 3 — Rendering**

* normalize baseline placement
* integrate DirectWrite/CoreText metrics
* update glyph atlas scaling

### **Phase 4 — Widget System**

* update all controls to use typography tokens
* normalize padding and label alignment

### **Phase 5 — DPI Integration**

* hook typography into HAL-024 scaling pipeline
* test 1.0×, 1.25×, 1.5×, 2×, 3× DPI

### **Phase 6 — Visual Audit**

* ensure no jitter in animated resizing
* ensure tree rows and controls align visually
* verify cross-platform parity
