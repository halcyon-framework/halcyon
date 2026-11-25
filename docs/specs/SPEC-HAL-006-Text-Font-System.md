**SPEC-HAL-006: Text & Font System**

**Project:** Halcyon  
**Author:** JC  
**Date:** 2025-11-17  
**Status:** Draft  
**Depends on:** SPEC-HAL-001/002/003/004/005  
**Scope:** Fonts, text layout, text primitives, caching, and integration with Scene/Theme.

---

## 1. Purpose

Text is the spine of a trading UI:

- price ladders  
- PnL numbers  
- symbol headers  
- axis labels  
- order status, logs, alerts  

This SPEC defines **how Halcyon handles text**:

- font loading & selection
- text styling (sizes, weights, colors)
- text layout (single-line now, multi-line later)
- caching & performance
- integration with `Scene` and `Theme`
- API surface for widgets (e.g. `Label`, `Button`, numeric text)

---

## 2. Design Goals

1. **Crisp, predictable rendering**
   - No blurry garbage.
   - Stable across resolutions and scale factors.

2. **Performance**
   - Handles thousands of labels (order grids, ladders, lists) at 60–120 FPS.
   - Caches shaped text, avoids rebuilding glyphs every frame.

3. **Deterministic layout**
   - Text measuring API available during layout.
   - Widgets can compute their exact size based on content and style.

4. **Backend abstraction**
   - High-level text types live in `halcyon::text` and `halcyon::scene`.
   - Concrete implementation (e.g. `glyphon` on top of `wgpu`) hidden behind a trait.

5. **Trading-centric**
   - Numeric text rendered clean, aligned, and stable.
   - No surprise reflows when numbers change.

---

## 3. Module Overview

New module:

```text
src/
  text.rs         // core text types + TextLayout API
````

Existing modules touched:

* `theme.rs` – extend with text styles
* `scene.rs` – refine `Primitive::Text`
* `layout.rs` – add text measurement hook to `LayoutContext`
* `renderer_wgpu.rs` – backend implementation (later SPEC wiring)

---

## 4. Core Types — `text.rs`

### 4.1 Font Identity

```rust
// src/text.rs

/// Logical font family name (not necessarily a specific file).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FontFamily {
    SystemDefault,
    Named(String), // e.g. "JetBrains Mono", "Inter"
}

/// Font weight (subset – enough for trading UIs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Thin,
    Light,
    Regular,
    Medium,
    Semibold,
    Bold,
}

/// Font style (italics are rare in trading, but keep the option).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
}

/// A complete font descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontDescriptor {
    pub family: FontFamily,
    pub weight: FontWeight,
    pub style: FontStyle,
}
```

### 4.2 TextStyle

A reusable style used by theme and widgets.

```rust
use crate::scene::Color;

#[derive(Debug, Clone)]
pub struct TextStyle {
    pub font: FontDescriptor,
    pub size: f32,       // logical px
    pub color: Color,
}
```

### 4.3 Text Alignment

Trading UIs need this especially for numbers.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignHorizontal {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignVertical {
    Top,
    Middle,
    Bottom,
}
```

### 4.4 TextLayout (Logical Layout, Backend-Agnostic)

We need two things:

1. ability to **measure** text (width/height)
2. ability to **reference** an already shaped text blob from the `Scene`.

```rust
/// Result of measuring a text run.
#[derive(Debug, Clone, Copy)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32, // distance from top to baseline
}

/// Opaque ID for a shaped text blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextRunId(pub u64);

/// Public API for text layout / measuring.
pub trait TextEngine {
    /// Measure a single-line text with given style.
    fn measure_single_line(
        &mut self,
        content: &str,
        style: &TextStyle,
    ) -> TextMetrics;

    /// Get or create a cached text run (glyph layout).
    ///
    /// Backend may cache by (content, style) key. Caller doesn’t care.
    fn get_or_create_run(
        &mut self,
        content: &str,
        style: &TextStyle,
    ) -> TextRunId;
}
```

The actual implementation (glyph rasterization, atlas, etc.) lives in the renderer backend (`renderer_wgpu.rs` + some `text_wgpu.rs` helper), implementing `TextEngine`.

---

## 5. Theme Integration — `theme.rs` Extension

Extend `Theme` (from SPEC-002) with text styles for typical use in trading UIs.

```rust
// src/theme.rs (additions)

use crate::text::{FontDescriptor, FontFamily, FontStyle, FontWeight, TextStyle};

#[derive(Debug, Clone)]
pub struct TextStyles {
    pub small: TextStyle,      // minor labels, hints
    pub normal: TextStyle,     // general UI text
    pub large: TextStyle,      // headers, section titles
    pub mono_small: TextStyle, // numeric columns, logs
    pub mono_normal: TextStyle,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub colors: Colors,
    pub text_sizes: TextSizes, // can keep or phase out in favor of TextStyles
    pub text_styles: TextStyles,
}
```

Example default:

```rust
impl Theme {
    pub fn dark_default() -> Self {
        let base_font = FontDescriptor {
            family: FontFamily::SystemDefault,
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
        };

        let mono_font = FontDescriptor {
            family: FontFamily::Named("JetBrains Mono".to_string()),
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
        };

        Theme {
            colors: Colors {
                background: Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 },
                foreground: Color { r: 0.90, g: 0.90, b: 0.90, a: 1.0 },
                primary: Color { r: 0.36, g: 0.75, b: 1.0, a: 1.0 },
                border: Color { r: 0.30, g: 0.30, b: 0.30, a: 1.0 },
            },
            text_sizes: TextSizes {
                small: 11.0,
                normal: 13.0,
                large: 16.0,
            },
            text_styles: TextStyles {
                small: TextStyle {
                    font: base_font.clone(),
                    size: 11.0,
                    color: Color { r: 0.80, g: 0.80, b: 0.85, a: 1.0 },
                },
                normal: TextStyle {
                    font: base_font.clone(),
                    size: 13.0,
                    color: Color { r: 0.90, g: 0.90, b: 0.95, a: 1.0 },
                },
                large: TextStyle {
                    font: base_font.clone(),
                    size: 16.0,
                    color: Color { r: 0.95, g: 0.95, b: 1.0, a: 1.0 },
                },
                mono_small: TextStyle {
                    font: mono_font.clone(),
                    size: 11.0,
                    color: Color { r: 0.82, g: 0.88, b: 0.98, a: 1.0 },
                },
                mono_normal: TextStyle {
                    font: mono_font,
                    size: 13.0,
                    color: Color { r: 0.87, g: 0.93, b: 1.0, a: 1.0 },
                },
            },
        }
    }
}
```

Numeric columns in trading tables should use `mono_*` styles.

---

## 6. Scene Graph Integration — `scene.rs`

### 6.1 Refine Primitive::Text

Current draft:

```rust
Primitive::Text {
    x: f32,
    y: f32,
    content: String,
    size: f32,
    color: Color,
}
```

Update to a more backend-friendly form that supports caching and multiple styles:

```rust
// src/scene.rs (replace Text primitive)

use crate::text::{TextAlignHorizontal, TextAlignVertical, TextRunId, TextStyle};

#[derive(Debug, Clone)]
pub enum Primitive {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        radius: f32,
    },
    Text {
        x: f32,
        y: f32,
        content: String,
        style: TextStyle,
        h_align: TextAlignHorizontal,
        v_align: TextAlignVertical,
        /// Optional reusable shaped text blob.
        ///
        /// If None, renderer may shape on the fly and cache.
        /// If Some, renderer uses pre-shaped run directly.
        run_id: Option<TextRunId>,
    },
}
```

**Coordinate semantics:**

* `(x, y)` is **anchor point**, interpreted by `(h_align, v_align)`:

    * Left/Top: x,y is top-left.
    * Right/Middle: e.g. for numbers, x,y can be right-aligned baseline or middle baseline – we decide:

**Proposal:**

* `(x, y)` = **top-left in UI space** for now.
* Alignment primarily affects how text is *placed inside allocated width* (widgets handle that).
* Later we can extend to baseline-aware positioning if needed.

SceneBuilder helpers:

```rust
impl<'a> SceneBuilder<'a> {
    pub fn text(
        &mut self,
        x: f32,
        y: f32,
        content: impl Into<String>,
        style: TextStyle,
        h_align: TextAlignHorizontal,
        v_align: TextAlignVertical,
        run_id: Option<TextRunId>,
    ) {
        self.scene.primitives.push(Primitive::Text {
            x,
            y,
            content: content.into(),
            style,
            h_align,
            v_align,
            run_id,
        });
    }
}
```

---

## 7. LayoutContext Text Measuring Hook — `layout.rs`

Widgets must be able to compute their size from text content.

Extend `LayoutContext` with a text engine reference:

```rust
// src/layout.rs

use crate::text::{TextEngine, TextMetrics, TextStyle};

pub struct LayoutContext<'a> {
    text_engine: &'a mut dyn TextEngine,
    // later: theme, scale_factor, etc.
}

impl<'a> LayoutContext<'a> {
    pub fn new(text_engine: &'a mut dyn TextEngine) -> Self {
        Self { text_engine }
    }

    pub fn measure_text(&mut self, content: &str, style: &TextStyle) -> TextMetrics {
        self.text_engine.measure_single_line(content, style)
    }
}
```

Widget `layout` methods can then do:

```rust
fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints) -> Size {
    let style = theme.text_styles.normal; // via some Theme access
    let metrics = cx.measure_text(&self.label, &style);
    Size {
        width: metrics.width,
        height: metrics.height,
    }
}
```

Integration of `Theme` into `LayoutContext` is a separate SPEC; for now, assume widgets can access it somehow (e.g. via context or injection).

---

## 8. Widget-Level Usage (Label Example)

A simple `Label` widget:

```rust
// src/widgets/label.rs (conceptual)

use crate::{
    layout::{Constraints, LayoutContext, Size},
    scene::{SceneBuilder},
    text::{TextAlignHorizontal, TextAlignVertical},
    theme::Theme,
    widget::Widget,
};

pub struct Label {
    pub text: String,
    pub style: TextStyle,
    measured_size: Size,
}

impl Label {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
            measured_size: Size::ZERO,
        }
    }
}

impl Widget for Label {
    fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints) -> Size {
        let metrics = cx.measure_text(&self.text, &self.style);
        // basic: respect min and max
        let width = metrics.width.clamp(constraints.min_width, constraints.max_width);
        let height = metrics.height.clamp(constraints.min_height, constraints.max_height);
        let size = Size { width, height };
        self.measured_size = size;
        size
    }

    fn paint(&self, builder: &mut SceneBuilder) {
        // For now: top-left text, no run_id pre-caching
        builder.text(
            0.0,
            0.0,
            &self.text,
            self.style.clone(),
            TextAlignHorizontal::Left,
            TextAlignVertical::Top,
            None,
        );
    }

    fn handle_event(&mut self, _event: &crate::event::Event) {}
}
```

Later we can:

* pre-create `TextRunId` and store it in widget state
* use that run_id in `SceneBuilder::text` for faster rendering.

---

## 9. Backend Implementation Strategy (Renderer Side)

This SPEC only defines the *interfaces*. Implementation details land in `renderer_wgpu.rs` and an internal helper (e.g. `text_wgpu.rs`):

### 9.1 Trait Impl

```rust
// renderer_wgpu.rs or text_wgpu.rs

use crate::text::{TextEngine, TextMetrics, TextStyle, TextRunId};

pub struct WgpuTextEngine {
    // e.g. glyphon context, font cache, atlas, etc.
    next_id: u64,
    // maps (content, style) -> TextRunId
    // maps TextRunId -> backend-specific shaped blob
}

impl TextEngine for WgpuTextEngine {
    fn measure_single_line(
        &mut self,
        content: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        // call glyph engine, return width/height/baseline
        unimplemented!()
    }

    fn get_or_create_run(
        &mut self,
        content: &str,
        style: &TextStyle,
    ) -> TextRunId {
        // look up hash key, shape if missing, return TextRunId
        unimplemented!()
    }
}
```

`RendererWgpu` holds a `WgpuTextEngine` instance and uses it both:

* when building `LayoutContext` each frame
* when rendering `Primitive::Text` primitives

---

## 10. Trading-Specific Constraints

1. **Numeric text stability**

    * Should not jitter when values change (avoid subpixel wobble).
    * Use monospaced fonts for ladders/order grids where column alignment matters.

2. **High density**

    * Many rows (50–200 visible rows) are common.
    * Many columns (price, size, side, status, etc).

3. **Performance**

    * Text layout must not dominate frame time.
    * Caching by `(content, style)` is mandatory.
    * Updating a few numbers per frame should not re-shape entire table.

4. **Precision**

    * Price strings with many decimals must remain legible.
    * No weird kerning artifacts around decimal points.

---

## 11. Implementation Phases (for Text)

### Phase T1 — Skeleton

* Add `text.rs` with:

    * `FontFamily`, `FontWeight`, `FontStyle`, `FontDescriptor`
    * `TextStyle`, `TextAlignHorizontal`, `TextAlignVertical`
    * `TextMetrics`, `TextRunId`, `TextEngine` trait

* Extend `theme.rs` with `TextStyles`.

* Refine `Primitive::Text` and `SceneBuilder::text`.

* Extend `LayoutContext` with `measure_text`.

### Phase T2 — Backend Wiring

* Implement `WgpuTextEngine` on top of a text rendering library (e.g. glyphon).
* Integrate into `RendererWgpu`:

    * supply `TextEngine` to layout and render passes.
    * shape + cache glyphs.
    * draw all text via a dedicated text pipeline.

### Phase T3 — Widgets

* Implement `Label` using text measurement.
* Implement `Button` with label and padding.
* Implement basic numeric label (e.g. `ValueLabel`) with right alignment and mono font.

### Phase T4 — Optimization

* Introduce `run_id` caching in widgets for ultra-hot paths (ladders).
* Performance test: 1000+ text cells in a grid at 120 FPS.

---

## 12. Success Criteria

Text system is considered “good enough for trading” when:

* ✅ Halcyon can render hundreds of labels, numeric and non-numeric, at 60–120 FPS.
* ✅ Text is crisp, with consistent size and spacing, no flicker.
* ✅ Widgets can accurately measure text and size themselves accordingly.
* ✅ Theme can define and swap multiple text styles (regular vs mono, small/normal/large).
* ✅ Renderer uses caching for repeated text patterns (e.g. price column).
* ✅ Numeric columns are stable: no column jitter when values update.
* ✅ No hidden dependencies between layout and renderer — all through `TextEngine` + `Scene`.
