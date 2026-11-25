# **SPEC-HAL-090 — Halcyon Logical Grid System & Compact Layout Rules**

**SPEC-HAL-090**
**Halcyon Logical Grid System & Compact Layout Rules**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 2.0

**Depends On:**

* SPEC-HAL-008 — Layout Engine
* SPEC-HAL-012 — Theming
* SPEC-HAL-024 — Coordinate Model
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-073 — Composition & Nesting Model
* SPEC-HAL-078 — Layout Transitions v1

**Referenced By:**

* SPEC-HAL-091 — Adaptive Density Modes
* SPEC-HAL-092 — Unified Typography Rules
* SPEC-HAL-093 — Visual Appearance & Radii System
* SPEC-HAL-094 — Component Geometry Specification
* SPEC-HAL-095 — Iconography Rules
* SPEC-HAL-096 — Color System & Semantic Palette

---

## **Implementation Status**

*Not implemented.*

This SPEC defines the **canonical spacing, sizing and density system for Halcyon**, and introduces:

* Logical grid units (U4 system)
* DPI scaling rules
* Compact UI spacing tokens
* Hierarchical padding rules
* Window → Panel → Widget spacing pipeline

This is the **foundational document** for all geometry-related SPECs.

---

# **1. Motivation**

Halcyon targets **dense, professional desktop interfaces**, radically different from:

* touch-first mobile UI
* Material/Figma oversized spacing
* web frameworks with 8–12 px minimums

Halcyon's design priorities:

1. **Compactness** — minimal empty space, efficient information density
2. **Precision** — consistent spacing, pixel-aligned geometry
3. **Scalability** — predictable behavior on HiDPI/4K/5K displays
4. **Hierarchy** — clear grouping through structured spacing
5. **Performance** — spacing must integrate cleanly with the layout engine

A unified spacing model is required so:

* widgets do not "drift" visually across platforms
* DPI scaling behaves consistently
* window layouts remain readable yet compact
* spacing decisions become automatic

---

# **2. Logical Grid System (U4)**

Halcyon does **not** use raw pixels for layout.
Instead, it uses **logical units** where:

```
1U = 1 px @ 96 DPI
```

Scaling is applied through SPEC-HAL-024 (Coordinate Model):

```
physical_px = logical_px × scale_factor
```

### **2.1 Base Cell: 4U (U4 Grid)**

All spacing values are multiples of **4 logical units**.

Reasons:

* ideal for compact UI
* multiples divide cleanly (4, 8, 12, 16, 20, 24...)
* easy DPI scaling
* works with typographic baselines (SPEC-HAL-092)
* avoids bloated Material-style 8px grids

### **2.2 Why Not 8U?**

8px base grids (Material, Bootstrap) produce spacing that is too large for professional desktop applications. At 8px minimum, a simple form with 10 fields consumes excessive vertical space.

Halcyon's 4U base allows:
* 4px minimal gaps
* 6px comfortable gaps
* 8px section gaps

—without ever dropping below readable/clickable minimums.

---

# **3. Spacing Tokens**

Compact UI requires **non-linear spacing steps**.
Tokens provide semantic meaning to numeric values.

### **3.1 Token Table (Balanced Mode)**

| Token  | Value | Purpose                                 |
|--------|-------|-----------------------------------------|
| **XS** | 4U    | Minimal gaps, icon-to-text, tight groups|
| **S**  | 6U    | Inner padding, label→input separation   |
| **M**  | 8U    | Standard gaps, panel padding            |
| **L**  | 10U   | Section separation                      |
| **XL** | 12U   | Major section breaks                    |
| **2XL**| 16U   | Large separations (rare)                |

### **3.2 Density Mode Scaling**

Spacing tokens scale per density mode (SPEC-HAL-091):

| Token | Compact | Balanced | Comfortable |
|-------|---------|----------|-------------|
| XS    | 4U      | 4U       | 5U          |
| S     | 6U      | 6U       | 8U          |
| M     | 7U      | 8U       | 10U         |
| L     | 9U      | 10U      | 13U         |
| XL    | 11U     | 12U      | 16U         |

Tokens are used everywhere in layout and theming layers.
Widgets **must not** hardcode pixel values.

---

# **4. Padding Tokens**

Padding is a specialized form of spacing, used for internal element boundaries.

| Token  | Value | Purpose                                 |
|--------|-------|-----------------------------------------|
| **P-XS** | 4U  | Vertical padding in controls            |
| **P-S**  | 6U  | Horizontal padding in controls          |
| **P-M**  | 8U  | Panel internal padding                  |
| **P-L**  | 10U | Section padding, dialog padding         |

### **4.1 Control Padding**

Standard for buttons, inputs, dropdowns:

```
horizontal = P-S (6U)
vertical   = P-XS (4U)
```

### **4.2 Panel Padding**

```
all sides = P-M (8U)
```

### **4.3 Window Padding**

```
content inset from window frame = P-L (10U)
```

---

# **4B. Root Content Inset Contract (INV-ROOT-INSET-CONTRACT)**

The `root_inset` token defines **content padding** between the window edge and scrollable content areas. This token has strict ownership rules to prevent double-application bugs.

### **4B.1 Token Definition**

| Token         | Compact | Regular | Comfortable |
|---------------|---------|---------|-------------|
| `root_inset`  | 6U      | 8U      | 12U         |

The value scales with density mode: tighter in Compact, roomier in Comfortable.

### **4B.2 Ownership Contract**

**SOLE CONSUMER:** `ScrollArea`

`root_inset` is consumed **only** by `ScrollArea` in its `layout()` method. No other widget, container, or layout system may consume this token.

**FORBIDDEN CONSUMERS:**
- `ScreenHost` (pure pass-through container)
- Any widget (no self-insetting)
- Demo/example code
- Any container or layout widget

### **4B.3 Semantic Rules**

1. **Content is inset, not the container.** Content is positioned at `(root_inset, root_inset)` inside ScrollArea. The container allocation remains unchanged.

2. **Scrollbars use container rect.** Scrollbars are positioned relative to the full container allocation, not the inset content area. This keeps scrollbars flush with container edges.

3. **Non-ScrollArea root content gets no inset.** If the root widget is not a ScrollArea (e.g., Flex, Label), no inset is applied. This is intentional; root_inset only affects scrollable content areas.

4. **No fallback padding.** Do not add special-case handling for non-ScrollArea roots.

### **4B.4 Invariants**

```
INV-ROOT-INSET-CONTENT:
  root_inset is content padding, not layout offset.
  Applied inside ScrollArea, never at container level.

INV-SCREENHOST-NO-INSET:
  ScreenHost MUST NOT apply root_inset.
  Children receive full constraints, positioned at (0, 0).
```

### **4B.5 Guard Mechanism**

ScreenHost contains a debug-time guard (`#[cfg(debug_assertions)]`) that touches `root_inset` to enable lint detection. Any attempt to use this value in ScreenHost layout logic should trigger review.

### **4B.6 Anti-Patterns (Forbidden)**

- ScreenHost applying inset to constraints or child positioning
- Widgets reading root_inset to self-inset
- Multiple consumers applying inset (double-inset bug)
- Using root_inset as container margin
- Adding fallback inset for non-ScrollArea roots

---

# **5. DPI Scaling Rules**

Spacing tokens are in **logical units**, not physical pixels.

Conversion:

```
physical_px = logical_px × dpi_scale
```

Where `dpi_scale` = monitor_dpi / 96.

Examples:

| Display              | DPI  | Scale | XS (4U) Physical |
|----------------------|------|-------|------------------|
| Standard 1080p       | 96   | 1.00  | 4 px             |
| Windows 125%         | 120  | 1.25  | 5 px             |
| Windows 150%         | 144  | 1.50  | 6 px             |
| 27" 4K (~163 DPI)    | 163  | 1.70  | 6.8 → 7 px       |
| Retina 5K            | 220  | 2.29  | 9.1 → 9 px       |

Regardless of monitor, **visual proportions remain identical**.

---

# **6. Hierarchical Spacing Rules**

This is the core of the SPEC:
**"What depends on what?"**
**"Where do we measure spacing from?"**

Halcyon uses **closest-parent separation**, not window-root spacing.

### **6.1 Window-Level**

```
Window content padding = P-L (10U)
```

Prevents UI from hitting the outer frame.
Title bar and status bar are separate from content area.

### **6.2 Section-Level**

Sections are major UI blocks:

* Panels
* GroupBoxes
* Inspector sections
* Editor panes

```
Section gap (between sections) = S-L (10U)
Section internal padding       = P-M (8U)
```

### **6.3 Widget-Level**

Between widgets inside a section:

```
Widget gap (siblings)    = S-XS (4U)
Label → Input spacing    = S-S (6U)
Heading → Content        = S-M (8U)
```

### **6.4 Nested Panel Rules**

```
Parent panel padding     = P-M (8U)
Nested panel gap         = S-M (8U)
Nested panel padding     = P-S (6U) — tighter than parent
```

---

# **7. Component Integration**

Spacing tokens integrate with component geometry (SPEC-HAL-094):

### **7.1 Buttons**

```
height           = 24–28 px (per density)
horizontal pad   = P-S (6U)
icon-text gap    = S-XS (4U)
button gap       = S-XS (4U)
```

### **7.2 Inputs**

```
height           = 24–28 px
horizontal pad   = P-S (6U)
vertical pad     = P-XS (4U)
```

### **7.3 Lists**

```
row height       = 20–24 px
horizontal pad   = P-S (6U)
icon-text gap    = S-XS (4U)
```

### **7.4 Trees**

```
row height       = 20–22 px
indent per level = 12U
icon-text gap    = S-XS (4U)
```

See SPEC-HAL-094 for complete component specifications.

---

# **8. Typography Integration**

Typography tokens (SPEC-HAL-092) use the same 4U baseline grid:

```
T-XS: 11px font, 12px line-height (3×4)
T-S:  12px font, 16px line-height (4×4)
T-M:  13px font, 16px line-height (4×4)
T-L:  14px font, 20px line-height (5×4)
T-XL: 16px font, 20px line-height (5×4)
```

This ensures:

* text and spacing align to the same grid
* no fractional positioning in logical space
* stable vertical rhythm

---

# **9. Theme System Integration**

Layout engine receives spacing tokens as constants:

```rust
theme.spacing.xs    // 4
theme.spacing.s     // 6
theme.spacing.m     // 8
theme.spacing.l     // 10
theme.spacing.xl    // 12

theme.padding.xs    // 4
theme.padding.s     // 6
theme.padding.m     // 8
theme.padding.l     // 10
```

Widgets do not hardcode numbers.

The theme system (SPEC-HAL-072, SPEC-HAL-074) supplies spacing tokens to:

* panels
* text fields
* buttons
* tree views
* list items
* inspectors
* toolbars
* menus

---

# **10. Examples**

### **10.1 Simple Form**

```
[Window]
  padding: P-L (10U)

  [Panel "Settings"]
    padding: P-M (8U)

    Label "Username"
    gap: S-S (6U)
    Input

    gap: S-M (8U)

    Label "Password"
    gap: S-S (6U)
    Input

    gap: S-L (10U)

    [ButtonBar]
      gap: S-XS (4U)
      Button "Cancel"
      Button "Save"
```

### **10.2 Tree + Inspector Layout**

```
[Window]
  padding: P-L (10U)

  [Splitter horizontal]

    [Panel "Tree"]
      padding: P-M (8U)
      TreeView
        row-height: 20U
        indent: 12U

    [Panel "Inspector"]
      padding: P-M (8U)

      [Section "Properties"]
        gap: S-M (8U)
        PropertyGrid
          row-height: 24U

      gap: S-L (10U)

      [Section "Actions"]
        gap: S-XS (4U)
        Button "Apply"
        Button "Reset"
```

This yields a **compact, clean, engineer-friendly UI**.

---

# **11. Implementation Plan**

### Phase 1 — Token Definition

Define all spacing and padding tokens in theme engine.

### Phase 2 — Layout Engine

Integrate tokens into layout calculations.

### Phase 3 — Widget Refactor

Update all widgets to use tokens instead of hardcoded values.

### Phase 4 — DPI Testing

Verify spacing at 1.0×, 1.25×, 1.5×, 2.0×, 3.0× DPI.

### Phase 5 — Density Mode Integration

Connect tokens to SPEC-HAL-091 density mode system.

---

# **12. Related SPECs**

* **SPEC-HAL-091** — Adaptive Density Modes (scales these tokens)
* **SPEC-HAL-092** — Typography (uses same 4U baseline)
* **SPEC-HAL-093** — Visual Appearance (radii, borders, shadows)
* **SPEC-HAL-094** — Component Geometry (uses these spacing tokens)
* **SPEC-HAL-095** — Iconography (icon sizing and alignment)
* **SPEC-HAL-096** — Color System (semantic color palette)
