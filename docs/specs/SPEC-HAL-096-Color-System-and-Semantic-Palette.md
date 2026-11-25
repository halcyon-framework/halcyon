# **SPEC-HAL-096 — Color System & Semantic Palette**

**SPEC-HAL-096**
**Color System & Semantic Palette**
**Author:** JC
**Status:** PROPOSED (Amended)
**Version:** 3.0

**Depends On:**

* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-074 — Style Runtime Engine
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules
* SPEC-HAL-091 — Adaptive Density Modes
* SPEC-HAL-093 — Visual Appearance & Radii System

---

## **Implementation Status**

*Not implemented.*

This SPEC defines Halcyon's **complete color model**:

* surface hierarchy (gray base, white content — desktop convention)
* sculpted border colors (highlight/shadow pairs)
* semantic color roles
* contrast logic
* disabled-state rules
* accent mapping
* neutral tones with slight warmth
* theme inheritance
* **reference palette values**
* **z-index layering scale**
* **scrollbar theming**

Color in Halcyon is **functional, structured, and minimalistic**, matching its professional desktop character.

---

# **1. Motivation**

Color is not decoration — it is a **semantic tool**.

Halcyon targets:

* dashboards
* IDE-grade layouts
* inspectors
* trading terminals
* engineering tools
* workstation applications

Therefore the color system must be:

* minimal
* contrast-stable
* mathematically predictable
* themeable
* expressive in states (hover, active, selected)
* safe for accessibility
* comfortable for extended viewing sessions

Colors must never:

* distract,
* overpower geometry,
* reduce readability,
* break compact design language,
* cause eye fatigue over extended sessions.

---

# **2. Surface Hierarchy**

Halcyon defines surfaces as **layers**, following the classic desktop convention:

**The base application surface is a warm gray, NOT white.** White is reserved for content/input surfaces where the user reads or types. This creates a clear visual distinction between the application chrome (gray) and the content regions (white) — the same convention used by every major desktop application framework.

### **Surface-Chrome — Window Frame**

Used for:

* title bar
* window border
* toolbar backgrounds
* status bar

Typically the darkest (dark theme) or most distinct (light theme) surface.

### **Surface-0 — Base Background (Warm Gray)**

The root application background. This is a **warm gray**, not white.
Used in:

* dialog backgrounds
* panel backgrounds
* main application areas
* form backgrounds

This is the surface most widgets sit on. It is the visual "desk" — neutral, warm, comfortable for hours of viewing.

### **Surface-Content — Input & Data Surfaces (White / Near-White)**

Used for:

* text fields
* text areas
* list view contents
* table cells
* tree view contents
* editable content regions
* any surface where the user reads dense text or enters data

Surface-Content is LIGHTER than Surface-0 in light theme (white or near-white), creating a clear "recessed paper" effect for content regions. Combined with sunken sculpted borders (SPEC-HAL-093), input fields are visually distinct from the surrounding gray application surface.

Contrast relative to Surface-0:

```
Δ luminance = 6–8% lighter (light theme)
Δ luminance = 3–5% lighter (dark theme)
```

### **Surface-1 — Panels & Sidebars**

Used for:

* sidebars
* inspectors
* docked tool windows
* secondary panels

Contrast relative to Surface-0:

```
Δ luminance = ±2–4%
```

### **Surface-2 — Hover & Selection States**

Used for:

* hover backgrounds on interactive elements
* selected row backgrounds (non-accent)

Contrast relative to resting surface:

```
Δ luminance = ±4–6%
```

### **Surface-3 — Floating Elements**

Used for:

* dropdown menus
* context menus
* tooltips
* popovers
* floating tool windows

Contrast relative to Surface-0:

```
Δ luminance = ±6–10%
```

---

# **3. Semantic Color Tokens**

Halcyon adopts a **strict semantic color palette**.
Widgets and themes must use semantic roles, never raw RGB.

### **3.1 Typography**

| Token              | Role                              |
|--------------------|-----------------------------------|
| **Text-Primary**   | Body text, data values, labels    |
| **Text-Secondary** | Hints, metadata, descriptions     |
| **Text-Muted**     | Disabled text, placeholders       |
| **Text-Inverse**   | Text on accent backgrounds        |
| **Text-Bold**      | Section headers, group titles     |

Rules:

* Primary must exceed WCAG AA contrast (≥ 4.5:1) on its surface.
* Secondary = Primary × 0.70–0.75 luminance.
* Muted = Primary × 0.45–0.55 luminance.
* Inverse used only on accent-colored backgrounds.
* Bold uses the same color as Primary but at **semi-bold or bold weight**. It is a weight token, not a color token — same hue, different rendering.

---

### **3.2 Icons**

Icons bind to:

| Token              | Role                              |
|--------------------|-----------------------------------|
| **Icon-Primary**   | Default icon color                |
| **Icon-Secondary** | De-emphasized icons               |
| **Icon-Muted**     | Disabled icons                    |
| **Icon-Accent**    | Active/selected state icons       |

Icon alignment and weight defined in SPEC-HAL-095.

---

### **3.3 Borders — Flat**

| Token              | Role                                        |
|--------------------|---------------------------------------------|
| **Border-Subtle**  | Panel dividers, inactive separators         |
| **Border-Default** | Non-sculpted borders on containers          |
| **Border-Strong**  | High-contrast borders, active containers    |
| **Border-Focus**   | Keyboard focus indicator                    |

---

### **3.4 Borders — Sculpted**

Sculpted borders use highlight/shadow pairs to create visual depth (SPEC-HAL-093 §3).

| Token                | Role                                                  |
|----------------------|-------------------------------------------------------|
| **Border-Highlight** | Bright side of sculpted edge (lighter than surface)   |
| **Border-Shadow**    | Dark side of sculpted edge (darker than surface)      |

Sculpted border colors MUST derive from the adjacent surface color:

```
Light theme:
  Border-Highlight = Surface-0 lightened by 8–12% (toward white)
  Border-Shadow    = Surface-0 darkened by 15–20%

Dark theme:
  Border-Highlight = Surface-0 lightened by 10–15%
  Border-Shadow    = Surface-0 darkened by 8–12%
```

The highlight/shadow pair creates the visual groove/raise/recess effect described in SPEC-HAL-093 §3.2.

---

### **3.5 Accents**

Must be limited to **one primary accent hue** per theme.

Accent roles:

| Token                | Role                                   |
|----------------------|----------------------------------------|
| **Accent-Primary**   | Primary actions, active selections     |
| **Accent-Hover**     | Hover state on primary actions         |
| **Accent-Muted**     | Subtle selection backgrounds           |
| **Accent-Background**| Tinted surface for selected regions    |

Accent must be:

* high-contrast
* non-saturated enough for professional UI
* readable on dark/light themes
* not neon or fluorescent

The default Halcyon accent is a restrained blue — functional, not decorative.

---

### **3.6 Semantic States**

| Token       | Usage                        |
|-------------|------------------------------|
| **Success** | Confirmations, valid states  |
| **Warning** | Thresholds, caution          |
| **Error**   | Failed states, invalid input |
| **Info**    | Neutral information          |

State colors must:

1. Never overpower accent colors.
2. Stay strictly within balanced saturation.
3. Not be used for decoration.
4. Maintain 4.5:1 contrast where applicable.

---

# **4. Interaction Colors**

### **4.1 Hover**

```
surface ± 4–6% luminance
text unchanged
sculpted border: slightly brightened highlight, slightly darkened shadow
```

### **4.2 Active (Pressed)**

```
Button: sculpted border INVERTS (raised → sunken)
surface − 4–8%
text unchanged
```

### **4.3 Focus**

```
border = Border-Focus (B-2 thickness from SPEC-HAL-093)
replaces sculpted border
no blur, no glow
```

### **4.4 Selected**

```
background = Accent-Muted
text = Text-Primary (unchanged)
optional: left edge accent bar (2px)
```

### **4.5 Disabled**

```
text     = Text-Muted @ 50% opacity
icon     = Icon-Muted @ 45% opacity
surface  = Surface-0 (no change)
border   = Flat Border-Subtle @ 50% opacity (sculpting removed)
```

No blur. No grayscale filter hacks. Opacity is the canonical method.

---

# **5. Dividers & Lines**

Dividers follow exact geometry and color rules:

### Flat Divider

```
thickness = 1 logical px
color     = Border-Subtle
```

### Etched Divider (Groove)

```
Line 1: 1px Border-Shadow
Line 2: 1px Border-Highlight
Total:  2px visual width
```

Etched dividers are the DEFAULT for section separators and GroupBox borders.
Flat dividers are for minor separations within a section.

---

# **6. Shadows & Elevation Colors**

Shadows are defined in SPEC-HAL-093.
Their color must be:

### Dark Theme

```
shadow_color = #000000 @ 35–45% opacity
```

### Light Theme

```
shadow_color = #000000 @ 12–20% opacity
```

No colored shadows allowed.
No inner glows except on focus states.

---

# **7. Z-Index Layering Scale**

All elevated elements must use canonical z-index tokens.

| Token            | Value | Usage                              |
|------------------|-------|------------------------------------|
| **Z-Base**       | 0     | Normal content, panels             |
| **Z-Sticky**     | 100   | Sticky headers, frozen columns     |
| **Z-Dropdown**   | 200   | Dropdowns, select menus            |
| **Z-Overlay**    | 300   | Sidesheets, floating panels        |
| **Z-Popover**    | 400   | Tooltips, context menus            |
| **Z-Modal**      | 500   | Modal dialogs                      |
| **Z-Toast**      | 600   | Notifications, toasts              |
| **Z-Debug**      | 1000  | Debug overlays (dev only)          |

Rules:

1. Widgets must not hardcode z-index values.
2. Z-index is consumed from theme tokens.
3. Nested elevated elements inherit parent z-context.

---

# **8. Scrollbar Theming**

Scrollbars must be themed consistently with the surface hierarchy.

| Token                  | Role                          |
|------------------------|-------------------------------|
| **Scrollbar-Track**    | Scrollbar background          |
| **Scrollbar-Thumb**    | Draggable thumb               |
| **Scrollbar-Thumb-Hover** | Thumb on hover             |

### Geometry

```
width  = 10 logical px (vertical)
height = 10 logical px (horizontal)
thumb-radius = R-0 (sharp corners — consistent with Halcyon identity)
```

### Behavior

* Thumb visible on scroll or hover (configurable)
* Track may be invisible until interaction
* No arrows by default (configurable)

---

# **9. Reference Palette — Light Theme (Halcyon Light)**

Concrete values implementing the semantic tokens above.

### **9.1 Surfaces**

| Token            | Hex       | Description                              |
|------------------|-----------|------------------------------------------|
| Surface-Chrome   | `#D4D7DC` | Title bar, window frame                  |
| Surface-0        | `#ECECEC` | Base background (warm gray)              |
| Surface-Content  | `#FFFFFF` | Input fields, list contents (white)      |
| Surface-1        | `#E4E5E8` | Sidebars, panels                         |
| Surface-2        | `#D8DAE0` | Hover states, selected rows              |
| Surface-3        | `#F0F1F3` | Dropdowns, popovers, tooltips            |

Note: Surface-0 (`#ECECEC`) is a warm gray, NOT white. Surface-Content (`#FFFFFF`) is white. This creates the classic desktop pattern: gray application background with white input fields.

### **9.2 Borders — Flat**

| Token            | Hex       |
|------------------|-----------|
| Border-Subtle    | `#D0D2D6` |
| Border-Default   | `#B8BCC4` |
| Border-Strong    | `#8A9099` |
| Border-Focus     | `#4A90D9` |

### **9.3 Borders — Sculpted**

| Token              | Hex       | Derivation                    |
|--------------------|-----------|-------------------------------|
| Border-Highlight   | `#FAFAFA` | Surface-0 lightened ~10%      |
| Border-Shadow      | `#A0A4AB` | Surface-0 darkened ~18%       |

### **9.4 Typography**

| Token            | Hex       |
|------------------|-----------|
| Text-Primary     | `#1C1E22` |
| Text-Secondary   | `#5C6370` |
| Text-Muted       | `#9DA3AE` |
| Text-Inverse     | `#FFFFFF` |

### **9.5 Icons**

| Token            | Hex       |
|------------------|-----------|
| Icon-Primary     | `#1C1E22` |
| Icon-Secondary   | `#5C6370` |
| Icon-Muted       | `#9DA3AE` |
| Icon-Accent      | `#4A90D9` |

### **9.6 Accents**

| Token            | Hex       |
|------------------|-----------|
| Accent-Primary   | `#3A7BC8` |
| Accent-Hover     | `#2A6AB8` |
| Accent-Muted     | `#D8E6F5` |
| Accent-Background| `#EBF2FA` |

### **9.7 Semantic States**

| Token   | Hex       |
|---------|-----------|
| Success | `#4A8E5F` |
| Warning | `#B89025` |
| Error   | `#C44A4A` |
| Info    | `#4A7EB9` |

### **9.8 Scrollbar**

| Token                  | Hex       |
|------------------------|-----------|
| Scrollbar-Track        | `#E4E5E8` |
| Scrollbar-Thumb        | `#B8BCC4` |
| Scrollbar-Thumb-Hover  | `#8A9099` |

---

# **10. Reference Palette — Dark Theme (Halcyon Dark)**

### **10.1 Surfaces**

| Token            | Hex       | Description                              |
|------------------|-----------|------------------------------------------|
| Surface-Chrome   | `#0D0E10` | Title bar, window frame                  |
| Surface-0        | `#1E2024` | Base background (warm dark gray)         |
| Surface-Content  | `#282A2F` | Input fields, list contents              |
| Surface-1        | `#252830` | Sidebars, panels                         |
| Surface-2        | `#2E3139` | Hover states, selected rows              |
| Surface-3        | `#363940` | Dropdowns, popovers, tooltips            |

### **10.2 Borders — Flat**

| Token            | Hex       |
|------------------|-----------|
| Border-Subtle    | `#2A2D33` |
| Border-Default   | `#3A3F47` |
| Border-Strong    | `#4A5058` |
| Border-Focus     | `#5A9BF0` |

### **10.3 Borders — Sculpted**

| Token              | Hex       | Derivation                    |
|--------------------|-----------|-------------------------------|
| Border-Highlight   | `#3A3D44` | Surface-0 lightened ~12%      |
| Border-Shadow      | `#111316` | Surface-0 darkened ~10%       |

### **10.4 Typography**

| Token            | Hex       |
|------------------|-----------|
| Text-Primary     | `#E8EAED` |
| Text-Secondary   | `#9DA3AE` |
| Text-Muted       | `#5C6370` |
| Text-Inverse     | `#1E2024` |

### **10.5 Icons**

| Token            | Hex       |
|------------------|-----------|
| Icon-Primary     | `#E8EAED` |
| Icon-Secondary   | `#9DA3AE` |
| Icon-Muted       | `#5C6370` |
| Icon-Accent      | `#5A9BF0` |

### **10.6 Accents**

| Token            | Hex       |
|------------------|-----------|
| Accent-Primary   | `#4A90D9` |
| Accent-Hover     | `#5A9FE8` |
| Accent-Muted     | `#2D4A6B` |
| Accent-Background| `#1E2A3A` |

### **10.7 Semantic States**

| Token   | Hex       |
|---------|-----------|
| Success | `#5A9E6F` |
| Warning | `#C9A035` |
| Error   | `#D45A5A` |
| Info    | `#5A8EC9` |

### **10.8 Scrollbar**

| Token                  | Hex       |
|------------------------|-----------|
| Scrollbar-Track        | `#1E2024` |
| Scrollbar-Thumb        | `#3A3F47` |
| Scrollbar-Thumb-Hover  | `#4A5058` |

---

# **11. Warm Tone Requirement**

Both light and dark themes must have a **slight warmth** to their neutral grays. Pure mathematically neutral grays (equal R, G, B values) read cold and sterile on screen. Professional desktop software has a slight warm bias — not yellow, not beige, but 1-2 points toward warmth.

Implementation: neutral grays should have R ≥ G ≥ B by 1-3 values.

Example: `#ECECEC` (warm) vs `#E8E8E8` (cold neutral). The difference is imperceptible in isolation but accumulates across an entire UI to create either comfort or sterility.

---

# **12. Theme Inheritance Model**

Every theme must provide all tokens defined in Sections 9–10.

Widgets **must not** reference raw colors.

Theme structure:

```
theme.surface.chrome
theme.surface.base           // Surface-0 (warm gray)
theme.surface.content        // Surface-Content (white/near-white)
theme.surface.panel          // Surface-1
theme.surface.hover          // Surface-2
theme.surface.floating       // Surface-3

theme.text.primary
theme.text.secondary
theme.text.muted
theme.text.inverse

theme.icon.primary
theme.icon.secondary
theme.icon.muted
theme.icon.accent

theme.border.subtle
theme.border.default
theme.border.strong
theme.border.focus
theme.border.highlight       // Sculpted: bright side
theme.border.shadow          // Sculpted: dark side

theme.accent.primary
theme.accent.hover
theme.accent.muted
theme.accent.background

theme.semantic.success
theme.semantic.warning
theme.semantic.error
theme.semantic.info

theme.scrollbar.track
theme.scrollbar.thumb
theme.scrollbar.thumb_hover

theme.z.base
theme.z.sticky
theme.z.dropdown
theme.z.overlay
theme.z.popover
theme.z.modal
theme.z.toast
```

---

# **13. Color Interpolation & Animation**

When colors animate:

* interpolation must be **gamma-corrected**
* linear blending allowed only in linear color space
* never animate hue directly
* opacity fades allowed

Animations must remain subtle (≤ 100–150ms).

---

# **14. Light & Dark Theme Parity**

Rules:

1. Typography contrast must always meet AA requirements.
2. Accent priority remains identical in visual hierarchy.
3. Surfaces invert ordering but preserve Δ relationships.
4. Shadows lighten in light theme, darken in dark theme.
5. Sculpted border highlight/shadow swap roles: in dark theme, highlight is above and shadow is below (same as light theme — the direction is physical, not relative to theme brightness).

Dark theme accent must not glow or visually "bleed".

---

# **15. Rendering Constraints**

Color pipeline must:

* support linear blending
* output correctly in sRGB
* be consistent across Vulkan / DX12 / Metal / GL
* avoid color banding in low-contrast surfaces
* allow theme-driven LUT for future HDR work

---

# **16. Implementation Plan**

### **Phase 1 — Token Foundation**

Define all semantic tokens in theme engine, including sculpted border colors.

### **Phase 2 — Reference Palettes**

Implement Halcyon Light and Halcyon Dark as reference themes with the exact values from Sections 9–10.

### **Phase 3 — Widget Integration**

Refactor UI to use semantic tokens exclusively. No raw RGB in widget paint methods.

### **Phase 4 — Surface-Content Integration**

Ensure all input widgets (TextField, TextArea, ComboBox, SpinBox, ListBox) use Surface-Content background, NOT Surface-0.

### **Phase 5 — Sculpted Border Integration**

Ensure all sculpted borders (raised, sunken, etched) use Border-Highlight and Border-Shadow tokens.

### **Phase 6 — Z-Index Integration**

Ensure all elevated elements consume z-index tokens.

### **Phase 7 — Scrollbar Implementation**

Implement themed scrollbars across all scrollable containers.

### **Phase 8 — Rendering**

Ensure linear blending in shader pipelines.

### **Phase 9 — QA**

Check:

* readability
* cross-platform consistency
* AA compliance
* color stability during animations
* z-index stacking correctness
* sculpted border rendering at all DPI scales
* warm tone consistency across themes
