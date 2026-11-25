# **SPEC-HAL-094 — Component Geometry & Layout Rules (Unified Edition)**

**SPEC-HAL-094**
**Component Geometry & Layout Rules (Unified Edition)**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 3.0 (Supersedes all previous versions)

**Depends On:**

* SPEC-HAL-008 — Layout Engine
* SPEC-HAL-024 — Coordinate Model
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules
* SPEC-HAL-091 — Adaptive Density Modes
* SPEC-HAL-092 — Unified Typography Rules
* SPEC-HAL-093 — Visual Appearance & Radii System
* SPEC-HAL-095 — Iconography & Pixel Grid Alignment
* SPEC-HAL-096 — Color System & Semantic Palette
* SPEC-HAL-097 — Motion & Micro-Animation Rules
* SPEC-HAL-099 — Input Precision & Interaction Model
* SPEC-HAL-100 — Widget Interaction State Machine

---

# **Implementation Status**

*Not implemented.*

This unified SPEC defines the **canonical geometry system** for Halcyon's interactive components.
It specifies:

* minimum component sizes
* padding & spacing tokens
* icon–text alignment rules
* vertical rhythm & baseline alignment
* component-specific geometry (buttons, inputs, lists, trees)
* DPI behavior & pixel grid constraints
* consistency rules across density modes
* integration with typography, iconography, motion, and the interaction model

This SPEC is the **single source of truth** for all geometry-related decisions.

---

# **1. Geometry Philosophy**

Halcyon follows three principles:

### **1.1 Compactness**

Controls must occupy minimal space while preserving legibility and precision.
Halcyon is not touch-first; it is desktop-first, precision-first.

### **1.2 Precision**

All geometry must align to the logical grid and pixel grid after scaling.
No fractional positioning in final rasterization.

### **1.3 Predictability**

All components must feel visually "related" by sharing:

* a consistent baseline
* consistent padding
* consistent border thickness
* consistent internal spacing
* consistent icon sizing
* consistent radii

UI should look engineered, not ornamental.

---

# **2. Layout Tokens**

Defined in SPEC-HAL-090.

### **2.1 Padding Tokens**

| Token  | Value | Usage                           |
|--------|-------|---------------------------------|
| P-XS   | 4U    | Vertical padding in controls    |
| P-S    | 6U    | Horizontal padding in controls  |
| P-M    | 8U    | Panel internal padding          |
| P-L    | 10U   | Section padding                 |

### **2.2 Spacing Tokens**

| Token  | Value | Usage                           |
|--------|-------|---------------------------------|
| S-XS   | 4U    | Icon-to-text gap, tight groups  |
| S-S    | 6U    | Label-to-input separation       |
| S-M    | 8U    | Between related controls        |
| S-L    | 12U   | Section separation              |

### **2.3 Corner Radii**

(From SPEC-HAL-093)

| Token | Value | Usage                           |
|-------|-------|---------------------------------|
| R-XS  | 2 px  | Inputs, toolbar buttons         |
| R-S   | 3 px  | Buttons, dropdowns, tags        |
| R-M   | 4 px  | Containers, dialog bodies       |
| R-L   | 6 px  | Popovers, floating panels (max) |

Default for most controls: **R-S (3px)**.

---

# **3. Baseline & Typography Alignment**

From SPEC-HAL-092:

* All text aligns to the universal 4U baseline grid.
* First line in multiline fields aligns to the same baseline as adjacent single-line controls.
* Icons align using **optical centering relative to the text baseline**, not bounding-box centering.

This ensures geometric silence across controls.

---

# **3A. Glyph Containment Invariant (Binding)**

**Status:** BINDING — All controls MUST satisfy this.

Control geometry MUST guarantee that rendered glyphs never cross control boundaries.

### 3A.1 Invariants

**INV-GEOM-CTRL-TEXT-1: Glyph Containment**
Control geometry MUST guarantee that rendered glyphs do not cross the
control's top or bottom edges.

**INV-GEOM-CTRL-TEXT-2: Container Definition**
Control height is defined as a container for glyph metrics (ascent + descent),
not line boxes (line_height).

### 3A.2 Verification

For any control with text content:

```
glyph_top    = baseline_y - ascent
glyph_bottom = baseline_y + descent

REQUIRE: glyph_top >= control_rect.y
REQUIRE: glyph_bottom <= control_rect.y + control_rect.height
```

### 3A.3 Implementation Reference

See SPEC-HAL-092 Section 8A for the canonical text placement formula that
satisfies these invariants.

---

# **4. Font Stack**

Halcyon uses platform-optimized font stacks:

### **4.1 UI Font (Sans-Serif)**

```
Primary:   Inter, SF Pro Text, Segoe UI, system-ui, sans-serif
```

Fallback order:
1. **Inter** — preferred, available cross-platform
2. **SF Pro Text** — macOS system font
3. **Segoe UI** — Windows system font
4. **system-ui** — platform default
5. **sans-serif** — ultimate fallback

### **4.2 Monospace Font**

```
Monospace: JetBrains Mono, SF Mono, Cascadia Code, Consolas, monospace
```

Fallback order:
1. **JetBrains Mono** — preferred, excellent for code/data
2. **SF Mono** — macOS
3. **Cascadia Code** — Windows Terminal default
4. **Consolas** — Windows legacy
5. **monospace** — ultimate fallback

### **4.3 Font Loading**

Fonts are loaded at application startup.
Missing fonts trigger fallback chain silently — no runtime errors.

---

# **5. Component Heights by Density Mode**

All heights in logical pixels.

| Component        | Compact | Balanced | Comfortable |
|------------------|---------|----------|-------------|
| Button           | 24      | 28       | 32          |
| Button (small)   | 20      | 24       | 28          |
| Input field      | 24      | 28       | 32          |
| Dropdown         | 24      | 28       | 32          |
| Tab              | 28      | 32       | 36          |
| Toolbar row      | 32      | 36       | 40          |
| Menu item        | 24      | 28       | 32          |
| List row         | 20      | 24       | 28          |
| Tree row         | 20      | 22       | 26          |
| Table row        | 24      | 28       | 32          |
| Status bar       | 22      | 24       | 28          |

---

# **6. Buttons**

### **6.1 Geometry**

| Property         | Compact | Balanced |
|------------------|---------|----------|
| Height           | 24 px   | 28 px    |
| Horizontal pad   | P-S (6) | P-S (6)  |
| Border           | B-1     | B-1      |
| Radius           | R-S (3) | R-S (3)  |

### **6.2 Icon + Text**

```
icon_size         = I-S (12px) or I-M (14px)
icon_to_text_gap  = S-XS (4U)
```

### **6.3 Icon-Only Buttons**

```
width  = height (square)
icon   = centered
radius = R-S
```

### **6.4 Button Variants**

| Variant   | Background      | Border          | Text          |
|-----------|-----------------|-----------------|---------------|
| Default   | Surface-2       | Border-Default  | Text-Primary  |
| Primary   | Accent-Primary  | Accent-Primary  | Text-Inverse  |
| Ghost     | transparent     | transparent     | Text-Primary  |
| Danger    | Error           | Error           | Text-Inverse  |

States (hover, active, disabled) per SPEC-HAL-093 §6.

---

# **7. Input Fields**

### **7.1 Geometry**

| Property         | Compact | Balanced |
|------------------|---------|----------|
| Height           | 24 px   | 28 px    |
| Horizontal pad   | P-S (6) | P-S (6)  |
| Vertical pad     | P-XS (4)| P-XS (4) |
| Border           | B-1     | B-1      |
| Radius           | R-XS (2)| R-XS (2) |

### **7.2 Internal Icons**

Clear button, dropdown arrow, search icon:

```
icon_size    = I-XS (10px) or I-S (12px)
icon_padding = P-XS from edge
alignment    = vertical center
```

### **7.3 States**

| State    | Border         | Background       |
|----------|----------------|------------------|
| Default  | Border-Default | Surface-Content  |
| Hover    | Border-Strong  | Surface-Content  |
| Focus    | Border-Focus   | Surface-Content  |
| Error    | Error          | Surface-Content  |
| Disabled | Border-Subtle  | Surface-1        |

### **7.4 Text Areas (Multiline)**

Text areas follow identical rules to single-line input fields:

* Background: `Surface-Content`
* Border: `Border-Default` (same as input)
* Radius: `R-XS` (2px)
* Padding: `P-S` horizontal, `P-XS` vertical

Min-height should be at least 3× line-height (e.g., 48px for 16px line-height).

---

# **8. Checkbox, Radio, Switch**

### **8.1 Checkbox**

| Property      | Value          |
|---------------|----------------|
| Box size      | 14 × 14 px     |
| Border        | B-1            |
| Radius        | R-XS (2px)     |
| Checkmark     | 10px icon      |
| Stroke weight | 1.25 px        |

### **8.2 Radio**

| Property       | Value          |
|----------------|----------------|
| Outer diameter | 14 px          |
| Inner dot      | 6 px           |
| Border         | B-1            |

### **8.3 Switch (Toggle)**

| Property    | Value          |
|-------------|----------------|
| Track size  | 28 × 14 px     |
| Thumb size  | 12 px (circle) |
| Track radius| R-L (full pill)|
| Thumb inset | 1 px           |

Animation: position only, 100ms ease-out. No scale effects.

---

# **9. Lists & Tables**

### **9.1 List Rows**

| Property       | Compact | Balanced |
|----------------|---------|----------|
| Row height     | 20 px   | 24 px    |
| Horizontal pad | P-S (6) | P-S (6)  |
| Vertical pad   | P-XS (4)| P-XS (4) |
| Icon size      | I-S (12)| I-S (12) |
| Icon-text gap  | S-XS (4)| S-XS (4) |
| Background     | Surface-Content | Surface-Content |

### **9.2 Table Rows**

| Property       | Compact | Balanced |
|----------------|---------|----------|
| Row height     | 24 px   | 28 px    |
| Cell padding   | P-S (6) | P-S (6)  |
| Border         | Bottom only, Border-Subtle |
| Background     | Surface-Content | Surface-Content |

### **9.3 Selection**

```
background = Accent-Muted
text       = Text-Primary (unchanged)
```

Optional: 2px left accent bar on selected row.

---

# **10. Tree Views**

Extends list geometry.

### **10.1 Row Height**

Same as list rows: 20–24 px depending on density.

Background: `Surface-Content` (same as list rows).

### **10.2 Indentation**

```
indent_per_level = 12U (12 logical px)
```

### **10.3 Expand/Collapse Icon**

| Property    | Value          |
|-------------|----------------|
| Size        | 10–12 px       |
| Alignment   | Optical vertical center |
| Rotation    | 90° when expanded |

### **10.4 Node Icon

```
size         = I-S (12px)
icon-to-text = S-XS (4U)
```

---

# **11. Toolbars**

### **11.1 Geometry**

| Property    | Compact | Balanced |
|-------------|---------|----------|
| Height      | 32 px   | 36 px    |
| Padding     | P-S (6) | P-S (6)  |

### **11.2 Toolbar Buttons**

```
icon_size = I-S (12px) or I-M (14px)
gap       = S-XS (4U)
```

Icon-only buttons: square, side = toolbar height − (2 × P-XS).

### **11.3 Dividers**

```
width     = 1 px
height    = toolbar height − 8 px
color     = Border-Subtle
margin    = S-XS (4U) on each side
```

---

# **12. Tabs**

### **12.1 Geometry**

| Property    | Compact | Balanced |
|-------------|---------|----------|
| Height      | 28 px   | 32 px    |
| Horizontal pad | P-S (6) | P-M (8) |

### **12.2 Active Indicator**

```
position  = bottom edge
height    = 2 px
color     = Accent-Primary
```

### **12.3 Close Button (if applicable)

```
size      = 12 × 12 px
icon      = I-XS (10px)
margin    = S-XS from text
```

---

# **13. Dropdowns & Menus**

### **13.1 Dropdown Trigger**

Same geometry as input field.

### **13.2 Menu Panel**

```
padding      = P-XS (4U) vertical
border       = B-1, Border-Default
radius       = R-M (4px)
shadow       = Z-Dropdown level (SPEC-HAL-093)
background   = Surface-3
```

### **13.3 Menu Items**

| Property    | Value          |
|-------------|----------------|
| Height      | 28 px          |
| Padding     | P-S horizontal |
| Icon size   | I-S (12px)     |
| Icon-text   | S-XS (4U)      |

### **13.4 Separator**

```
height    = 1 px
color     = Border-Subtle
margin    = S-XS vertical
```

---

# **14. Modals & Dialogs**

### **14.1 Geometry**

```
min_width = 320 px
max_width = 560 px (soft limit)
padding   = P-L (10U) all sides
radius    = R-M (4px)
shadow    = Z-Modal level
```

### **14.2 Header**

```
height         = 36–40 px
font           = T-L (14px, weight 600)
bottom_border  = Border-Subtle
```

### **14.3 Footer (Button Bar)**

```
height       = 48 px
padding      = P-M (8U)
alignment    = right
button_gap   = S-M (8U)
```

---

# **15. DPI & Pixel Grid Rules**

All component geometry must resolve cleanly after DPI scaling.

### **15.1 Pixel Snap**

```
physical_px = round(logical_px × dpi_scale)
```

### **15.2 Borders**

Always integer physical pixels.
Fractional logical borders round to nearest whole pixel.

### **15.3 Icons**

Must use the DPI variant selected by the iconography system (SPEC-HAL-095).

### **15.4 Text**

Baseline alignment must remain stable under all DPI scales (SPEC-HAL-092, SPEC-HAL-104).

---

# **16. Declarative Tokens (Style System Integration)**

All geometry is exposed through theme tokens:

```
component.button.height.compact = 24
component.button.height.balanced = 28
component.button.padding_h = P-S
component.button.radius = R-S

component.input.height.compact = 24
component.input.height.balanced = 28
component.input.padding_h = P-S
component.input.radius = R-XS

component.list.row_height.compact = 20
component.list.row_height.balanced = 24
component.list.icon_size = I-S

component.tree.indent = 12
component.tree.row_height.compact = 20
component.tree.row_height.balanced = 22

component.toolbar.height.compact = 32
component.toolbar.height.balanced = 36
```

Widgets **must not** introduce ad-hoc geometry outside this spec.

---

# **17. Implementation Plan**

### **Phase 1 — Merge Token Definitions**

Unify the geometry and spacing definitions across the layout engine.

### **Phase 2 — Widget Integration**

Refactor all:

* buttons
* inputs
* lists
* trees
* tabs
* toolbars
* checkboxes
* radios
* switches
* dropdowns
* menus
* modals

To use the canonical geometry tokens.

### **Phase 3 — DPI QA**

Test:

* fractional DPI (125%, 150%, 175%)
* 2×, 3× scaling
* multi-monitor with mixed DPI

### **Phase 4 — Visual QA**

Confirm:

* crisp borders at all DPIs
* correct baseline alignment
* stable hover/press states (no layout shift)
* proper icon alignment
* consistent spacing across all components
