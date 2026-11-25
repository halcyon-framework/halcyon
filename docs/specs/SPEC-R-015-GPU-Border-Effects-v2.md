# SPEC-R-015-v2: Border Effects & Visual Affordances

### *Per-Side Borders, Focus Ring Integration, Border Radius Polish*

**Author:** JC
**Status:** Design
**Version:** 2.0
**Supersedes:** SPEC-R-015 (GPU Border Effects v2)

**Depends On:**

* SPEC-R-001 Clip Stack Rework (Complete)
* SPEC-R-004 GPU Path Renderer (Complete)
* SPEC-R-013-v2 Shadow Pipeline (focus ring rendered via shadow shader)

---

## 1. Purpose

Polish Halcyon's border system from the current minimal implementation to production quality. Three areas:

1. **Per-side border width** — top/right/bottom/left independently. Required for table cells, card headers, dividers with emphasis.
2. **Focus ring integration** — wire SPEC-R-013-v2's ShadowMode::Ring into the framework paint system. Automatic, theme-driven, overridable.
3. **Border radius consistency** — per-corner radius on BorderRect, consistent rendering between straight and rounded borders.

This SPEC does NOT introduce dash patterns, join styles, double borders, or animated transitions. Those are deferred to a future revision when actual widget demand justifies them.

---

## 2. Current State

### What Works

| Feature | Implementation | Notes |
|---|---|---|
| Uniform border | `BorderRect` SceneNode | Fill + 4 border rects (straight edges) |
| Rounded border | `add_rounded_border` in batcher | CPU tessellated via path pipeline |
| Per-corner radius | `RoundedRect` | Single radius value, applied uniformly |
| Clip stack | R-001, R-020 | Rounded clipping in production |

### What's Missing

| Feature | Impact |
|---|---|
| Per-side border width | Cannot do header-bottom-border-only, table cell borders |
| Per-corner radius on borders | Rounded borders always uniform radius |
| Focus ring | No keyboard focus affordance on any widget |
| Border color per-side | Cannot emphasize one edge |

---

## 3. Per-Side Border Width

### 3.1 Data Model

Replace the current single `border_width: f32` on `BorderRectNode` with:

```rust
#[derive(Debug, Clone, Copy)]
pub struct BorderWidths {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BorderWidths {
    pub fn uniform(w: f32) -> Self {
        Self { top: w, right: w, bottom: w, left: w }
    }

    pub fn zero() -> Self {
        Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 }
    }

    /// True if all sides are equal
    pub fn is_uniform(&self) -> bool {
        (self.top - self.right).abs() < 0.001
            && (self.right - self.bottom).abs() < 0.001
            && (self.bottom - self.left).abs() < 0.001
    }
}
```

### 3.2 BorderRectNode Update

```rust
pub struct BorderRectNode {
    pub rect: RectF,
    pub fill_color: Color,
    pub border_color: Color,
    pub border_widths: BorderWidths,   // was: border_width: f32
    pub radius: f32,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}
```

### 3.3 Rendering — Straight Borders

Current implementation draws 4 border rects around the fill rect. This naturally supports per-side width — each border rect's thickness is its side's width value. Sides with width 0.0 produce no geometry.

```rust
// Top border
if widths.top > 0.0 {
    add_rect(x, y, width, widths.top, border_color);
}
// Bottom border
if widths.bottom > 0.0 {
    add_rect(x, y + height - widths.bottom, width, widths.bottom, border_color);
}
// Left border (between top and bottom)
if widths.left > 0.0 {
    add_rect(x, y + widths.top, widths.left, height - widths.top - widths.bottom, border_color);
}
// Right border (between top and bottom)
if widths.right > 0.0 {
    add_rect(x + width - widths.right, y + widths.top, widths.right, height - widths.top - widths.bottom, border_color);
}
```

### 3.4 Rendering — Rounded Borders

For rounded borders with non-uniform widths, the path pipeline is required. The border shape is the difference between the outer rounded rect and the inner rounded rect (with adjusted corner radii).

For uniform-width rounded borders, the existing `add_rounded_border` path works unchanged.

For non-uniform-width rounded borders, generate two paths:
- Outer: `rounded_rect(rect, radius)`
- Inner: `rounded_rect(rect.inset(widths), radius - max(widths))`

Fill the region between them using even-odd fill rule, or render as two separate path fills with the inner one cutting a hole via clip.

**Simplification for v2:** If border widths are non-uniform AND radius > 0, fall back to rendering 4 separate border-side arcs via the path pipeline. This avoids complex boolean path operations. Full CSG can be added later if needed.

### 3.5 Backward Compatibility

The existing single `border_width: f32` API on widget builders should continue to work by constructing `BorderWidths::uniform(w)`.

Add new builder methods:

```rust
fn border_widths(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self
fn border_top(mut self, w: f32) -> Self
fn border_bottom(mut self, w: f32) -> Self
fn border_left(mut self, w: f32) -> Self
fn border_right(mut self, w: f32) -> Self
```

---

## 4. Focus Ring Integration

Focus ring rendering is defined in SPEC-R-013-v2 Section 7.2. This section specifies the framework integration.

### 4.1 Paint Traversal Hook

After each widget's `paint()` returns and before the widget's clip is popped, the paint traversal checks focus ring eligibility:

```rust
// In paint traversal, after widget.paint() returns:
if focus_manager.focused() == Some(widget_id) && widget.hit_testable() {
    let style = widget.focus_ring_style().unwrap_or(FocusRingStyle::Default);
    match style {
        FocusRingStyle::Default => {
            emit_focus_ring(widget_bounds, widget_radius, theme.focus_ring);
        }
        FocusRingStyle::Custom(params) => {
            emit_focus_ring(widget_bounds, params.radius, params.ring_theme);
        }
        FocusRingStyle::None => {
            // Widget explicitly suppresses focus ring
        }
    }
}
```

### 4.2 FocusRingStyle

```rust
pub enum FocusRingStyle {
    /// Use framework default (theme accent color, standard distance/softness)
    Default,
    /// Custom focus ring parameters
    Custom(FocusRingParams),
    /// Suppress focus ring entirely (widget handles its own focus indication)
    None,
}

pub struct FocusRingParams {
    pub radius: f32,
    pub ring_theme: FocusRingTheme,
}
```

### 4.3 Widget Trait Addition

```rust
pub trait Widget {
    // ... existing methods ...

    /// SPEC-R-015-v2: Override focus ring behavior.
    /// Return None to use framework default.
    fn focus_ring_style(&self) -> Option<FocusRingStyle> {
        None
    }
}
```

### 4.4 Theme

```rust
pub struct FocusRingTheme {
    /// Ring color (typically accent color, semi-transparent)
    pub color: Color,
    /// Distance from shape boundary to ring center (logical pixels)
    pub distance: f32,
    /// Gaussian sigma controlling ring softness (logical pixels)
    pub softness: f32,
}

impl Default for FocusRingTheme {
    fn default() -> Self {
        Self {
            color: Color::rgba(0.26, 0.52, 0.96, 0.6), // Blue accent, 60% opacity
            distance: 2.0,
            softness: 1.5,
        }
    }
}
```

### 4.5 Clip Considerations

The focus ring extends *outside* the widget's bounds. If the widget has a clip region, the ring would be clipped. The paint traversal must emit the focus ring SceneNode **after** the widget's own clip is popped, at the parent's clip level. This ensures the ring is visible outside the widget boundary but still clipped by the parent container.

### 4.6 Z-Ordering

Focus ring z-index = `widget_z + 2`. This places it:
- Above the widget's own paint (z)
- Above any inner shadow (z + 1)
- Below sibling widgets that paint later

---

## 5. Per-Corner Radius

### 5.1 Current Limitation

`RoundedRectNode` and `BorderRectNode` use a single `radius: f32`. This produces uniform corners. Many UI patterns need per-corner control:
- Card with rounded top, flat bottom (attached to a container)
- Tab with rounded top-left and top-right only
- Button group where end buttons have outer corners rounded, inner corners flat

### 5.2 Data Model

```rust
#[derive(Debug, Clone, Copy)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub bottom_right: f32,
}

impl CornerRadii {
    pub fn uniform(r: f32) -> Self {
        Self { top_left: r, top_right: r, bottom_left: r, bottom_right: r }
    }

    pub fn zero() -> Self {
        Self::uniform(0.0)
    }

    pub fn is_uniform(&self) -> bool {
        (self.top_left - self.top_right).abs() < 0.001
            && (self.top_right - self.bottom_right).abs() < 0.001
            && (self.bottom_right - self.bottom_left).abs() < 0.001
    }

    pub fn max_radius(&self) -> f32 {
        self.top_left.max(self.top_right).max(self.bottom_left).max(self.bottom_right)
    }
}
```

### 5.3 SceneNode Update

Replace `radius: f32` with `radii: CornerRadii` on:
- `RoundedRectNode`
- `BorderRectNode`
- `ShadowNode` (from R-013-v2)
- `GradientRectNode` (from R-012-v2)

When `radii.is_uniform()`, the existing single-radius code paths apply unchanged. When non-uniform, the path pipeline tessellates a rounded rect with per-corner arcs.

### 5.4 SDF Update for Per-Corner Radius

The shadow shader (R-013-v2) and gradient shader (R-012-v2) use an SDF for rounded corners. The current SDF assumes uniform radius. For per-corner radius, the SDF selects the appropriate radius based on which quadrant the fragment is in:

```wgsl
fn sd_rounded_rect_4(p: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    // radii = (top_left, top_right, bottom_right, bottom_left)
    let r = select(
        select(radii.w, radii.z, p.x > center.x),  // bottom: bl or br
        select(radii.x, radii.y, p.x > center.x),  // top: tl or tr
        p.y < center.y  // above or below center
    );
    let d = abs(p - center) - half_size + vec2(r);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;
}
```

### 5.5 Backward Compatibility

All existing APIs that accept `radius: f32` continue to work. `CornerRadii::uniform(radius)` is used internally. New builder methods:

```rust
fn corner_radii(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self
fn corner_radius_top(mut self, r: f32) -> Self
fn corner_radius_bottom(mut self, r: f32) -> Self
```

---

## 6. Implementation Stages

**Stage 1:** `BorderWidths` struct. Update `BorderRectNode` from `border_width: f32` to `border_widths: BorderWidths`. Backward-compatible constructors. Update batcher's straight-border rendering for per-side width.

**Stage 2:** `CornerRadii` struct. Update `RoundedRectNode`, `BorderRectNode`. Update path tessellation to use per-corner arcs when non-uniform. Backward-compatible constructors.

**Stage 3:** Update `ShadowNode` (R-013-v2) to use `CornerRadii`. Update shadow shader with `sd_rounded_rect_4`. Update `GradientRectNode` (R-012-v2) similarly.

**Stage 4:** `FocusRingStyle` enum. `focus_ring_style()` on Widget trait. `FocusRingTheme` in Theme.

**Stage 5:** Focus ring paint hook. Paint traversal emits ShadowMode::Ring SceneNode for focused widgets. Clip-aware emission (after widget clip pop).

**Stage 6:** Apply focus ring to existing widgets. Button, Checkbox, RadioButton, Switch, Slider, SpinBox, ComboBox, TextField, TextArea, TabBar items, ListView rows, TreeView rows.

**Stage 7:** Widget builder APIs. `border_widths()`, `border_top()`, `corner_radii()`, `corner_radius_top()`, etc.

**Stage 8:** Demo. `border_demo.rs` showing per-side borders, per-corner radii, focus ring on interactive widgets.

---

## 7. What This SPEC Does NOT Cover

- Dash patterns (deferred — no current widget demand)
- Join styles (deferred — rounded joins from path tessellation are sufficient)
- Double borders (deferred)
- Animated border transitions (handled by scene rebuild — animate node params)
- Per-side border color (deferred — single border color sufficient for now)
- Border images / nine-patch (deferred to R-019)
- Gradient borders (deferred — can be done via gradient rect behind shape)
