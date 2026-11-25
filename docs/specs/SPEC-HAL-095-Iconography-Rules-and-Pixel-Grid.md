# **SPEC-HAL-095 — Iconography Rules & Pixel Grid Alignment**

**SPEC-HAL-095**
**Iconography Rules & Pixel Grid Alignment**
**Author:** Halcyon Architect
**Status:** PROPOSED (§1–10 active; §11–12 superseded by SPEC-HAL-264 + SPEC-HAL-265)
**Version:** 2.0

**Depends On:**

* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-074 — Style Runtime Engine
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules
* SPEC-HAL-091 — Adaptive Density Modes
* SPEC-HAL-092 — Unified Typography Rules
* SPEC-HAL-093 — Visual Appearance & Radii System
* SPEC-HAL-094 — Component Geometry Specification
* SPEC-HAL-096 — Color System & Semantic Palette

---

## **Implementation Status**

*Not implemented.*

This SPEC defines the **canonical icon system for Halcyon**, including:

* pixel grid
* icon sizes
* stroke weight
* DPI strategy
* alignment rules
* contextual icon geometry (in lists, buttons, inspectors, trees)
* color logic

This ensures that Halcyon avoids all common iconography failures:

* blurry icons
* misaligned baselines
* inconsistent weights
* fuzzy outlines at fractional DPI
* oversized or "floaty" icon placement
* platform inconsistencies

---

# **1. Motivation**

Professional UI uses icons as **functional glyphs**, not decorative artwork.

Halcyon must be:

* compact
* extremely crisp
* pixel-aligned
* DPI-correct
* visually balanced with typography
* consistent across controls and densities

Inconsistent iconography is one of the fastest ways to make an otherwise clean UI look amateurish.
This SPEC defines the **mathematical and optical rules** for all icons.

---

# **2. Pixel Grid Alignment**

Halcyon icons follow a strict pixel grid:

```
1 logical px @ 96 DPI → becomes physical px via DPI scale
All strokes must snap to physical-pixel centers
```

Rules:

1. **Odd-sized icons use centered strokes**
   (e.g., 13px icon → 1px stroke centered on pixel grid)

2. **Even-sized icons use boundary-aligned strokes**
   (14px icon → stroke sits exactly on pixel grid edges)

3. **No fractional stroke widths** allowed in physical pixels
   (prevents blur)

4. **SVG/icon source is normalized** before raster or SDF transform.

---

# **3. Icon Size Tokens**

Halcyon defines canonical icon sizes:

| Token    | Size | Usage                                  |
|----------|------|----------------------------------------|
| **I-XS** | 10px | Ultra-compact: dense lists, indicators |
| **I-S**  | 12px | Primary: tree icons, list icons, small buttons |
| **I-M**  | 14px | Standard: toolbar buttons, menu items  |
| **I-L**  | 16px | Emphasis: section headers, inspectors  |
| **I-XL** | 20px | Large: dialogs, empty states           |

**I-S (12px) and I-M (14px)** are the primary Halcyon sizes.

This makes Halcyon more compact than Material (24px) or Fluent (20px default).

### Density Mode Scaling

Icon sizes adjust slightly per density mode:

| Context         | Compact | Balanced | Comfortable |
|-----------------|---------|----------|-------------|
| Tree/List icon  | I-S (12)| I-S (12) | I-M (14)    |
| Button icon     | I-S (12)| I-M (14) | I-M (14)    |
| Toolbar icon    | I-S (12)| I-M (14) | I-L (16)    |
| Menu icon       | I-S (12)| I-S (12) | I-M (14)    |

---

# **4. Stroke Weight**

Canonical stroke weights:

| Icon Size | Stroke   |
|-----------|----------|
| 10px      | 1.00 px  |
| 12px      | 1.00 px  |
| 14px      | 1.25 px  |
| 16px      | 1.25 px  |
| 20px      | 1.50 px  |

Stroke weight must match:

* button border thickness (B-1 = 1px)
* input border thickness (B-1 = 1px)
* typography weights (T-M text looks correct next to 1.25px stroke)

### Stroke Rendering

* Strokes are centered on the path
* No outside or inside stroke alignment
* Stroke caps: round or square (context-dependent)
* Stroke joins: round for organic shapes, miter for geometric

---

# **5. DPI Behavior**

Icons scale via:

```
physical_px = logical_px × dpi_scale
stroke_width also scales
snap-to-pixel always applied
```

### 5.1 Vector Icons (SDF/MSDF)

* Glyphs generated at multiple scales
* Chosen glyph is nearest to target DPI
* Shader AA preserves crispness
* SDF distance field resolution: 32px minimum

### 5.2 Raster Icons (PNG)

* Variants required: 1×, 1.5×, 2×, 3×
* Loader selects closest match, then scales if needed
* Never scale up; always scale down from higher-res variant

Halcyon tolerates no fuzzy icons.

---

# **6. Icon Alignment Rules**

Icons visually align to:

1. **Typography baseline** (when inline with text)
2. **Optical vertical center** (when in buttons or standalone)
3. **Grid-based offsets** (for list/tree rows)

### 6.1 Baseline Alignment Formula

When an icon appears inline with text:

```
icon_y = text_baseline − (icon_height / 2) + optical_offset
```

Where `optical_offset` is typically 1–2px depending on icon shape.

### 6.2 Optical vs Geometric Center

Halcyon uses **optical center alignment**, not bounding-box alignment.

Icons with visual weight toward the bottom (e.g., download arrow) appear centered when their optical center—not geometric center—aligns with the text.

---

# **7. Placement in Controls**

## **7.1 Buttons**

| Property        | Value                      |
|-----------------|----------------------------|
| Icon size       | I-S (12px) or I-M (14px)   |
| Icon-text gap   | S-XS (4U)                  |
| Icon position   | Before text (leading)      |
| Alignment       | Optical vertical center    |

Icon-only button:

```
width  = height (square)
icon   = centered horizontally and vertically
padding = symmetric
```

## **7.2 Inputs**

Use only small icons (I-XS or I-S):

* clear button (×)
* dropdown arrow (▾)
* search icon (🔍)

| Property        | Value                      |
|-----------------|----------------------------|
| Icon size       | I-XS (10px) or I-S (12px)  |
| Position        | Right edge, inset P-XS     |
| Alignment       | Optical vertical center    |

## **7.3 Lists**

| Property        | Compact    | Balanced   |
|-----------------|------------|------------|
| Row height      | 20px       | 24px       |
| Icon size       | I-S (12px) | I-S (12px) |
| Icon-text gap   | S-XS (4U)  | S-XS (4U)  |
| Icon position   | Leading    | Leading    |

Icons snap to row vertical center, adjusted for optical alignment.

## **7.4 Tree Views**

Same as lists, with additions:

| Property           | Value                  |
|--------------------|------------------------|
| Expand/collapse    | 10–12px chevron        |
| Node icon          | I-S (12px)             |
| Chevron-icon gap   | S-XS (4U)              |
| Icon-text gap      | S-XS (4U)              |
| Indentation        | 12U per level          |

Chevron rotates 90° when node is expanded.

## **7.5 Toolbars**

| Property        | Compact    | Balanced   |
|-----------------|------------|------------|
| Icon size       | I-S (12px) | I-M (14px) |
| Button size     | 24×24px    | 28×28px    |
| Icon alignment  | Centered   | Centered   |

## **7.6 Menus**

| Property        | Value                      |
|-----------------|----------------------------|
| Icon size       | I-S (12px)                 |
| Icon-text gap   | S-XS (4U)                  |
| Icon position   | Leading, left-aligned      |

Menu items without icons: indent text to align with items that have icons.

## **7.7 Tabs**

| Property        | Value                      |
|-----------------|----------------------------|
| Icon size       | I-S (12px)                 |
| Icon-text gap   | S-XS (4U)                  |
| Close button    | I-XS (10px), trailing      |

---

# **8. Color Model for Icons**

Icons must follow the theme's semantic color tokens (SPEC-HAL-096):

| Token          | Usage                          |
|----------------|--------------------------------|
| Icon-Primary   | Default icon color             |
| Icon-Secondary | De-emphasized, secondary icons |
| Icon-Muted     | Disabled icons                 |
| Icon-Accent    | Active/selected state icons    |

### 8.1 State Opacity Rules

| State      | Opacity           |
|------------|-------------------|
| Default    | 100%              |
| Secondary  | 70–80%            |
| Hover      | 100% + 6% lighter |
| Active     | 100% + 10% darker |
| Disabled   | 45%               |

### 8.2 Semantic Icon Colors

For status indicators:

| Token         | Usage                  |
|---------------|------------------------|
| Success (096) | Checkmarks, success    |
| Warning (096) | Caution indicators     |
| Error (096)   | Error, delete, close   |
| Info (096)    | Information indicators |

No other colored icons except semantic uses.

---

# **9. Interaction States**

| State    | Visual Change                        |
|----------|--------------------------------------|
| Hover    | +6% luminance                        |
| Active   | −10% luminance                       |
| Selected | Icon-Accent color                    |
| Disabled | 45% opacity                          |
| Focus    | No icon change; focus on container   |

Always avoid:

* large glows
* animated shifts on hover
* blurred focus halos
* scale animations

Consistency is key.

---

# **10. Integration with Theme System**

Tokens accessible via theme:

```
theme.icon.size.xs    // 10
theme.icon.size.s     // 12
theme.icon.size.m     // 14
theme.icon.size.l     // 16
theme.icon.size.xl    // 20

theme.icon.stroke.s   // 1.0
theme.icon.stroke.m   // 1.25
theme.icon.stroke.l   // 1.5

theme.icon.color.primary
theme.icon.color.secondary
theme.icon.color.muted
theme.icon.color.accent

theme.icon.gap        // S-XS (4U)
```

Widget geometry pulls these automatically; widgets must not override them manually.

---

# **11. Icon Asset Requirements**

### 11.1 Source Format

* SVG preferred for vector icons
* Paths must be optimized (no redundant nodes)
* Viewbox must match logical size (e.g., 0 0 14 14 for I-M)

### 11.2 SDF Generation

* Generate SDF at 4× logical size minimum
* Store in texture atlas
* Per SPEC-HAL-066 subpixel binning rules

### 11.3 Raster Fallback

* PNG at 1×, 1.5×, 2×, 3×
* Alpha channel for transparency
* No pre-baked colors (tint at render time)

---

# **12. Implementation Plan**

### Phase 1 — Define Tokens

Add:

* size tokens
* stroke tokens
* alignment rules
* DPI strategy

### Phase 2 — Asset Pipeline

Implement:

* SDF generator
* multi-DPI raster loader
* optical centering metadata
* texture atlas packing

### Phase 3 — Widget Integration

Refactor:

* button
* input
* list
* tree
* inspector
* toolbar
* menu
* tab

### Phase 4 — Pixel QA

Test:

* DPI 1.0, 1.25, 1.5, 2.0, 3.0
* zoom transitions
* baseline alignment
* stroke weight retention
* cross-platform rendering parity
