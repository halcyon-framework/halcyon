# **SPEC-HAL-091 — Adaptive Density Modes**

**SPEC-HAL-091**
**Adaptive Density Modes**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 2.0

**Depends On:**

* SPEC-HAL-008 — Layout Engine
* SPEC-HAL-012 — Theming System
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-090 — Logical Grid & Compact Layout Rules

**Referenced By:**

* SPEC-HAL-092 — Unified Typography Rules
* SPEC-HAL-093 — Visual Appearance & Radii System
* SPEC-HAL-094 — Component Geometry Specification
* SPEC-HAL-095 — Iconography Rules

---

## **Implementation Status**

*Not implemented.*

Defines UI density modes:

* **Compact**
* **Balanced**
* **Comfortable**

Based on token scaling, typography multipliers, control geometry presets, and visual appearance adjustments.

---

# **1. Motivation**

Users work on different:

* monitor sizes (24" to 32"+)
* DPI densities (96 to 220+)
* physical distances from screen
* professional tasks (trading vs design vs development)

Trading terminals require *dense* layouts.
Touch-oriented settings require *comfort*.

A single global UI scale is insufficient.
We need **density-aware design tokens** that reshape spacing, typography, control sizes, and visual appearance proportionally.

---

# **2. Density Modes Overview**

### **2.1 Mode Definitions**

| Mode        | Target Use Case                              |
|-------------|----------------------------------------------|
| **Compact** | Maximum information density; power users     |
| **Balanced**| Default Halcyon mode; general desktop use    |
| **Comfortable** | Relaxed spacing; accessibility, touch   |

### **2.2 Selection Criteria**

Density mode is selected by:

1. User preference (explicit setting)
2. Application default (per-app configuration)
3. System hint (accessibility settings, touch mode detection)

Mode can be:
* Global (entire application)
* Per-window
* Per-panel (advanced use cases)

---

# **3. Spacing Token Scaling**

Each mode defines multipliers over Logical Grid tokens (SPEC-HAL-090):

| Token | Base Value | Compact | Balanced | Comfortable |
|-------|------------|---------|----------|-------------|
| XS    | 4U         | 4U      | 4U       | 5U          |
| S     | 6U         | 6U      | 6U       | 8U          |
| M     | 8U         | 7U      | 8U       | 10U         |
| L     | 10U        | 9U      | 10U      | 13U         |
| XL    | 12U        | 11U     | 12U      | 16U         |

Compact mode shrinks larger tokens slightly; small tokens remain unchanged to preserve minimum touch/click targets.

---

# **4. Typography Scaling**

Typography uses adaptive multipliers applied to base token sizes (SPEC-HAL-092):

| Token | Base Size | Compact | Balanced | Comfortable |
|-------|-----------|---------|----------|-------------|
| T-XS  | 11px      | 11px    | 11px     | 12px        |
| T-S   | 12px      | 12px    | 12px     | 13px        |
| T-M   | 13px      | 13px    | 13px     | 14px        |
| T-L   | 14px      | 14px    | 14px     | 15px        |
| T-XL  | 16px      | 16px    | 16px     | 18px        |

**Line height always remains locked to the 4U baseline grid.**

When font size increases, line height rounds to the nearest multiple of 4U:
* 13px → 16px line-height
* 14px → 20px line-height
* 15px → 20px line-height

---

# **5. Component Heights**

All heights in logical pixels (SPEC-HAL-094):

| Component      | Compact | Balanced | Comfortable |
|----------------|---------|----------|-------------|
| Button         | 24      | 28       | 32          |
| Button (small) | 20      | 24       | 28          |
| Input field    | 24      | 28       | 32          |
| Dropdown       | 24      | 28       | 32          |
| Tab            | 28      | 32       | 36          |
| Toolbar row    | 32      | 36       | 40          |
| Menu item      | 24      | 28       | 32          |
| List row       | 20      | 24       | 28          |
| Tree row       | 20      | 22       | 26          |
| Table row      | 24      | 28       | 32          |
| Status bar     | 22      | 24       | 28          |

---

# **6. Visual Appearance Scaling**

Corner radii adapt per mode (SPEC-HAL-093):

| Token | Compact | Balanced | Comfortable |
|-------|---------|----------|-------------|
| R-XS  | 2px     | 2px      | 2px         |
| R-S   | 2px     | 3px      | 4px         |
| R-M   | 3px     | 4px      | 5px         |
| R-L   | 4px     | 6px      | 6px         |

Border thickness remains constant (B-1 = 1px, B-2 = 2px) across all modes.

Shadow intensity scales slightly:

| Mode        | Shadow Multiplier |
|-------------|-------------------|
| Compact     | 0.8× (subtle)     |
| Balanced    | 1.0× (standard)   |
| Comfortable | 1.0× (standard)   |

---

# **7. Icon Sizing**

Icon sizes per context and density mode (SPEC-HAL-095):

| Context        | Compact | Balanced | Comfortable |
|----------------|---------|----------|-------------|
| Tree/List icon | 12px    | 12px     | 14px        |
| Button icon    | 12px    | 14px     | 14px        |
| Toolbar icon   | 12px    | 14px     | 16px        |
| Menu icon      | 12px    | 12px     | 14px        |

Icon stroke weight does not change with density mode.

---

# **8. API & Theme Integration**

### **8.1 Setting Density Mode**

```rust
app.set_density_mode(DensityMode::Compact);
app.set_density_mode(DensityMode::Balanced);
app.set_density_mode(DensityMode::Comfortable);
```

### **8.2 Querying Tokens**

```rust
let spacing = theme.spacing(SpacingToken::M);  // Returns mode-adjusted value
let height = theme.component_height(Component::Button);
let radius = theme.radius(RadiusToken::S);
let font_size = theme.typography(TypographyToken::M).size;
```

### **8.3 Per-Window Override**

```rust
window.set_density_mode(DensityMode::Compact);
```

Overrides global mode for that window only.

---

# **9. Layout Engine Interaction**

Layout computes using **logical units**. Density mode scales tokens before the layout pass.

Sequence:

1. Widget requests spacing token (e.g., `S`)
2. Theme returns mode-adjusted value (e.g., 6U for Balanced, 8U for Comfortable)
3. Layout engine uses adjusted value
4. No widget code changes required

---

# **10. Transition Behavior**

When density mode changes at runtime:

1. All spacing tokens are invalidated
2. Layout is recalculated
3. No animation (instant relayout)
4. Scroll positions are preserved where possible

Mode changes are expected to be infrequent (user preference changes, not per-interaction).

---

# **11. Implementation Plan**

### Phase 1 — Token Tables

Define mode-specific values for:
* Spacing tokens
* Typography tokens
* Component heights
* Radii
* Icon sizes

### Phase 2 — Theme Integration

Implement density-aware token resolution in theme engine.

### Phase 3 — Layout Engine

Ensure layout engine consumes mode-adjusted tokens.

### Phase 4 — API Surface

Expose:
* `set_density_mode()` at app and window level
* Mode query in widgets (read-only)

### Phase 5 — QA

Test:
* Mode switching at runtime
* Consistent visual proportions
* No layout artifacts
* Scroll position preservation

---

# **12. Future Extensions**

* Per-screen density mode (multi-monitor)
* Dynamic density based on window size
* Adaptive density based on DPI
* Touch-mode auto-detection
* Per-panel density override
