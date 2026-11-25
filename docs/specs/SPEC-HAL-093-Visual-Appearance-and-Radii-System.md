# **SPEC-HAL-093 — Visual Appearance & Radii System**

**SPEC-HAL-093**
**Visual Appearance & Radii System**
**Author:** JC
**Status:** PROPOSED (Amended)
**Version:** 3.0

**Depends On:**

* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-074 — Style Runtime Engine
* SPEC-HAL-078 — Layout Transitions v1
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules
* SPEC-HAL-092 — Unified Typography Rules

---

## **Implementation Status**

*Not implemented.*

This SPEC defines Halcyon's **canonical visual language**:

* corner radii
* sculpted border model (highlight/shadow pairs)
* border thickness
* inset and groove effects
* surface layering
* geometry anti-aliasing rules
* visual density cohesion

The goal:
**clean, compact, professional desktop UI with sculpted depth cues**, avoiding both flat web minimalism and heavy skeuomorphism.

Halcyon's visual identity draws from the tradition of professional desktop software — IDEs, engineering tools, data dashboards, workstation applications. It is NOT a web aesthetic. It is NOT a mobile aesthetic. It is dense, functional, and visually refined.

---

# **1. Motivation**

Modern UI trends either:

* overshoot with excessive shadows, radii, blur, cards (Apple, Fluent, Material),
* or undershoot, producing flat, bland, undifferentiated geometry (web frameworks, early Linux Qt/GTK).

Both fail professional software. Excessive decoration wastes density. Flat rendering destroys visual hierarchy.

Halcyon requires:

* sharp, compact geometry
* sculpted depth cues via highlight/shadow border pairs
* mathematically predictable borders
* no shadows on base-level elements
* strict layering rules
* compatibility with GPU pipelines (R-series SPECs)

Halcyon's look should resemble:

* professional IDEs and property panels
* data dashboards and workstation tools
* trading terminals and medical imaging software
* engineering and CAD applications

—not websites, not mobile apps, not consumer "modern" UIs.

The visual language must be something people WANT to look at for extended sessions — comfortable, refined, and information-dense without being sterile or fatiguing.

---

# **2. Corner Radius System**

Halcyon defines **4 radii tokens**. Professional desktop software uses sharp corners. Rounded corners are the exception, not the rule.

| Token    | Radius (logical px) | Usage                                            |
|----------|---------------------|--------------------------------------------------|
| **R-0**  | 0                   | DEFAULT. Inputs, buttons, panels, tables, trees, list rows, GroupBox borders, toolbar buttons |
| **R-XS** | 2                   | Tooltips, dropdown menus, floating panels        |
| **R-S**  | 3                   | Popovers, dialog bodies                          |
| **R-M**  | 4                   | Maximum radius: modal dialogs only               |

### Rules:

1. **R-0 covers 90%+ of Halcyon.** Buttons, inputs, containers, panels — all sharp corners.
2. **Radius must never exceed 4 logical px.** Larger radii break compact visual language.
3. Rounded corners (R-XS and above) are reserved for **floating/overlay elements only**.
4. Nested elements use **equal or smaller** radii than their parent.
5. No pill shapes. No bubbly corners. No web-style border-radius.

### Density Mode Scaling

| Mode        | R-0 | R-XS | R-S | R-M |
|-------------|-----|------|-----|-----|
| Compact     | 0   | 1    | 2   | 3   |
| Balanced    | 0   | 2    | 3   | 4   |
| Comfortable | 0   | 2    | 3   | 4   |

R-0 is ALWAYS 0 regardless of density mode.

---

# **3. Sculpted Border Model**

This is the defining visual characteristic of Halcyon.

Professional desktop software uses **sculpted borders** — pairs of highlight and shadow pixels that create the perception of physical depth. This is NOT skeuomorphism. It is a proven technique used in every major desktop toolkit for 30 years. It provides visual hierarchy without consuming space, without shadows, without cards.

### 3.1 Border Tokens

| Token              | Thickness    | Usage                                        |
|--------------------|--------------|----------------------------------------------|
| **B-0**            | 0 px         | Borderless mode, separators via contrast     |
| **B-1**            | 1 px logical | Primary border: flat single-color borders    |
| **B-2**            | 2 px logical | Focus states, active containers, emphasis    |
| **B-Sculpted**     | 2 px visual  | Sculpted edge: 1px highlight + 1px shadow    |

### 3.2 Sculpted Edge Types

Three sculpted edge types, built from highlight/shadow pairs:

#### **Raised** — Buttons, toolbar buttons

```
Top edge:    1px Border-Highlight
Left edge:   1px Border-Highlight
Bottom edge: 1px Border-Shadow
Right edge:  1px Border-Shadow
```

Creates the perception of a raised, pressable surface. The button appears to sit above the background.

#### **Sunken (Recessed)** — Input fields, text areas, list views

```
Top edge:    1px Border-Shadow
Left edge:   1px Border-Shadow
Bottom edge: 1px Border-Highlight
Right edge:  1px Border-Highlight
```

Creates the perception of a recessed surface. The input appears carved into the background. This is the canonical treatment for any editable or content-display region.

#### **Etched (Groove)** — GroupBox borders, toolbar separators, section dividers

```
Line 1: 1px Border-Shadow
Line 2: 1px Border-Highlight
```

Total: 2px visual width. Creates an inset groove in the surface — a clean section divider that reads as a physical indent. This is the primary visual technique for grouping.

### 3.3 Sculpted Border Color Tokens

Defined in SPEC-HAL-096. Summary:

| Token              | Role                                              |
|--------------------|----------------------------------------------------|
| **Border-Highlight** | Bright side of sculpted edge (lighter than surface) |
| **Border-Shadow**    | Dark side of sculpted edge (darker than surface)    |
| **Border-Default**   | Flat single-color border (non-sculpted)             |
| **Border-Subtle**    | Dividers, inactive separators                       |
| **Border-Strong**    | High-contrast, active containers                    |
| **Border-Focus**     | Keyboard focus indicator                            |

### 3.4 Sculpted Border Rules

1. **Raised edges are for interactive controls only.** Buttons, toolbar buttons, tab headers. Never for containers or labels.
2. **Sunken edges are for content regions.** Text fields, text areas, list views, table bodies, editable surfaces. Content is always recessed.
3. **Etched edges are for grouping.** GroupBox borders, toolbar separators, panel dividers. Grouping is always grooved.
4. **Focus overrides sculpting.** When a sculpted element receives keyboard focus, the sculpted border is replaced by `Border-Focus` at `B-2` thickness.
5. **Pressed state inverts sculpting.** A raised button becomes sunken when pressed (highlight/shadow swap). This creates physical "pressed into surface" feedback.
6. **Disabled elements flatten.** Sculpted borders become flat `Border-Subtle` at reduced opacity.

### 3.5 Pixel Grid Snap Rule

Borders must snap to **physical pixel grid** — no fractional raster width.

```
if (physical_border_px < 1.0) → round up to 1 physical px
else → round to nearest integer
```

DPI scaling:

```
physical_border_px = logical_border_px × dpi_scale
```

---

# **4. Shadow Model**

Halcyon avoids shadows on base-level elements entirely.

### Core Principles:

1. **No shadows on base-level UI.** Panels, buttons, inputs, containers — none have shadows. Depth is communicated through sculpted borders, not shadows.
2. Shadows exist ONLY for **floating elements** (dropdowns, popovers, modals).
3. Shadows must be barely perceptible — functional, not decorative.
4. No colored shadows. Shadow color is always `#000000` at specified opacity.
5. No blur on anything below Z-Dropdown.

### Elevation Shadow Table

| Level         | Z-Index   | Offset (x,y) | Blur | Spread | Opacity (Dark) | Opacity (Light) |
|---------------|-----------|--------------|------|--------|----------------|-----------------|
| **Base**      | Z-Base    | —            | —    | —      | —              | —               |
| **Sticky**    | Z-Sticky  | —            | —    | —      | —              | —               |
| **Dropdown**  | Z-Dropdown| (0, 2)       | 4    | 0      | 30%            | 12%             |
| **Overlay**   | Z-Overlay | (0, 2)       | 6    | 0      | 35%            | 15%             |
| **Popover**   | Z-Popover | (0, 3)       | 8    | 0      | 40%            | 18%             |
| **Modal**     | Z-Modal   | (0, 4)       | 12   | 0      | 45%            | 20%             |

Base and Sticky levels have NO shadow. Depth is sculpted borders only.

All values in logical units. Shadow color = `#000000` at specified opacity.

### Shadow Rendering

Shadows must render through GPU pipelines (R-series), not CPU.
Gaussian blur kernel size derived from blur value.

---

# **5. Surfaces & Layering**

Halcyon uses **5 surface tones** (defined in SPEC-HAL-096):

| Surface          | Role                                         |
|------------------|----------------------------------------------|
| Surface-Chrome   | Window frame, title bar, status bar          |
| Surface-0        | Dialog/panel background (warm gray, NOT white)|
| Surface-Content  | Input fields, list contents, editable areas (white/near-white) |
| Surface-1        | Panels, sidebars, inspectors                 |
| Surface-2        | Hover states, selected rows                  |
| Surface-3        | Floating: dropdowns, popovers, tooltips      |

**Critical distinction:** Surface-0 is the application background — a warm gray. Surface-Content is for editable/readable content — lighter (white in light theme). Input fields use Surface-Content, NOT Surface-0. This is the classic desktop software pattern: gray dialogs with white input fields.

Contrast is minimal between adjacent non-content surfaces:
Δ ~ 2–4% luminance difference between Surface-0, Surface-1.

Contrast between Surface-0 and Surface-Content is deliberate:
Δ ~ 6–8% luminance — enough to clearly distinguish editable regions from the background.

---

# **6. Interaction States**

Interaction elements (inputs, buttons, toggles) use consistent state model:

### 6.1 Default (Resting)

```
Button:    Raised sculpted border, Surface-1 background
Input:     Sunken sculpted border, Surface-Content background
Container: Flat Border-Default or etched border
Text:      Text-Primary
```

### 6.2 Hover

```
background = Surface-2 (4–6% shift from resting)
border     = Sculpted border brightened slightly
text       = Text-Primary (unchanged)
```

No movement. No shadows added on hover. Hover is a surface color shift only.

### 6.3 Focus (Keyboard)

```
border     = Border-Focus (B-2 thickness), replaces sculpted border
background = unchanged
```

Focus is indicated by border only — no glows, no outlines offset from element.
Focus border REPLACES the sculpted border, it does not wrap around it.

### 6.4 Active (Pressed)

```
Button:    Sculpted border INVERTS (raised → sunken)
           background = Surface-2 shifted −4%
Input:     No change (already sunken)
```

The pressed state of a raised button is its sculpted border flipping from raised to sunken. This creates immediate physical "pressed into surface" feedback without any animation or movement.

### 6.5 Selected

```
background = Accent-Muted
border     = Border-Default or Accent-Primary
text       = Text-Primary
```

### 6.6 Disabled

```
background = Surface-1 (unchanged)
border     = Flat Border-Subtle @ 50% opacity (sculpting removed)
text       = Text-Muted @ 50% opacity
icon       = Icon-Muted @ 45% opacity
```

Disabled elements lose their sculpted borders and become flat. They remain in place; they do not visually "recede" via blur or grayscale.

---

# **7. GroupBox Rendering**

GroupBox is one of Halcyon's most important visual elements. It uses the **etched (groove)** border style.

### 7.1 Border

```
Outer line: 1px Border-Shadow
Inner line: 1px Border-Highlight
Corner radius: R-0 (sharp corners)
```

This creates a clean groove in the surface that visually groups its contents.

### 7.2 Title

The GroupBox title text sits on the top edge, interrupting the etched border. The title area has a background fill matching Surface-0 to mask the border behind it.

Title text uses **Text-Primary at bold weight** for clear section hierarchy.

### 7.3 Content Padding

Content inside GroupBox is inset by the theme's standard padding token (typically 8-12px logical).

---

# **8. Geometry Anti-Aliasing Rules**

To avoid artifacts on compact UI:

### 8.1 Sharp Corners (R-0)

Sharp corners at R-0 require no AA — they are axis-aligned pixel rectangles.
This is the majority of Halcyon's geometry.

### 8.2 Rounded Corners (R-XS and above)

Radii must be:

* rasterized through shader-based AA
* blended within 1 logical pixel
* never use CPU-side masks or stencil hacks

### 8.3 Lines & Borders

Lines = always **1 logical px**, snapped to physical pixel center.

```
For 1px logical line at DPI scale 1.5:
  physical = 1.5px → round to 2px, centered on pixel grid
```

### 8.4 Sculpted Border Rendering

Sculpted borders (highlight + shadow pairs) are rendered as two adjacent 1px rects, NOT as a single 2px line with a gradient. Each line is a distinct color snapped to the pixel grid.

### 8.5 Subpixel Positioning

Geometry positions may be fractional in logical space.
Physical positions snap to pixel grid before rasterization.

---

# **9. Integration With Density Modes**

Visual tokens adapt according to SPEC-HAL-091:

| Mode        | Radii       | Border    | Shadow              | Sculpted |
|-------------|-------------|-----------|---------------------|----------|
| Compact     | R-0 only    | B-1       | Minimal             | Yes      |
| Balanced    | R-0 primary | B-1       | Standard            | Yes      |
| Comfortable | R-0 primary | B-1 / B-2 | Standard            | Yes      |

Core geometry relationships remain unchanged across modes.
Sculpted borders are present in ALL density modes.

---

# **10. Integration With Theme System**

Tokens become part of theme definition:

```
theme.radii.none     // 0 — R-0
theme.radii.xs       // 2 — R-XS
theme.radii.s        // 3 — R-S
theme.radii.m        // 4 — R-M

theme.border.width_1   // 1 logical px
theme.border.width_2   // 2 logical px

theme.border.highlight // Bright side of sculpted edge
theme.border.shadow    // Dark side of sculpted edge
theme.border.default   // Flat single-color border
theme.border.subtle    // Dividers, inactive separators
theme.border.strong    // High-contrast
theme.border.focus     // Focus indicator

theme.shadow.dropdown.offset
theme.shadow.dropdown.blur
theme.shadow.dropdown.opacity

theme.surface.chrome
theme.surface.base       // Surface-0 (warm gray background)
theme.surface.content    // Surface-Content (white/near-white for inputs)
theme.surface.panel      // Surface-1
theme.surface.hover      // Surface-2
theme.surface.floating   // Surface-3
```

Theme inheritance ensures:

* light/dark palettes remain consistent
* custom themes cannot break geometry
* density modes apply correct scaling
* sculpted border colors derive from surface colors automatically

---

# **11. Sculpted Rendering Examples**

### Button (Raised)

```
┌─────────────────────┐  ← 1px Border-Highlight (top + left)
│  Button Text         │
└─────────────────────┘  ← 1px Border-Shadow (bottom + right)
Background: Surface-1
```

### TextField (Sunken)

```
┌─────────────────────┐  ← 1px Border-Shadow (top + left)
│  Input text          │
└─────────────────────┘  ← 1px Border-Highlight (bottom + right)
Background: Surface-Content (white)
```

### GroupBox (Etched)

```
─ Section Title ──────  ← 1px Border-Shadow + 1px Border-Highlight (groove)
│                      │
│  Contents            │
│                      │
──────────────────────  ← same groove on bottom
```

### Pressed Button (Inverted)

```
┌─────────────────────┐  ← 1px Border-Shadow (top + left) — INVERTED
│  Button Text         │
└─────────────────────┘  ← 1px Border-Highlight (bottom + right) — INVERTED
Background: Surface-2 darkened
```

---

# **12. Implementation Plan**

### Phase 1 — Token Definition

Define in theme:

* radius tokens (R-0 through R-M)
* border tokens (B-0 through B-2, B-Sculpted)
* sculpted color tokens (Border-Highlight, Border-Shadow)
* surface palette (Surface-Chrome through Surface-3 + Surface-Content)
* elevation shadow table (floating elements only)

### Phase 2 — Sculpted Border Primitives

Implement rendering primitives:

* `paint_raised_border(rect, highlight, shadow)` — for buttons
* `paint_sunken_border(rect, highlight, shadow)` — for inputs
* `paint_etched_border(rect, highlight, shadow)` — for GroupBox, separators
* These are simple 1px rect pairs, not new shader features

### Phase 3 — Widget Integration

Update paint methods for:

* Button — raised border
* TextField, TextArea — sunken border
* ComboBox, SpinBox — sunken border (input portion)
* GroupBox — etched border
* Divider — etched line
* Toolbar separators — etched line
* ScrollArea — sunken content region
* Panel — flat or etched border

### Phase 4 — Visual QA

Validate:

* pixel snapping at 1.0×, 1.25×, 1.5×, 2.0×, 3.0× DPI
* sculpted borders remain crisp at all scales
* highlight/shadow colors derive correctly from surface colors
* pressed state inversion reads clearly
* focus border replacement works cleanly
* disabled state flattening is visible
