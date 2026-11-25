**SPEC-HAL-102**
**Halcyon Widget Template (Final Form)**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 1.0

**File: `src/widgets/<widget_name>.rs`**

```rust
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::utils::math::{RectF, Size};

#[derive(Debug)]
pub struct MyWidget {
    id: WidgetInternalId,
    // widget state fields...
}

impl MyWidget {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            // init state...
        }
    }
}

impl Widget for MyWidget {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    // ------------------------------------------------------------
    // 1. Layout
    // ------------------------------------------------------------
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // ALWAYS use cx.theme
        let geom = cx.theme.geometry.button;      // example
        let height = geom.height;

        // width = constrained by parent
        let width = constraints.max_width;

        LayoutResult::fit(Size::new(width, height))
    }

    // ------------------------------------------------------------
    // 2. Paint
    // ------------------------------------------------------------
    fn paint(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _children: &[NodeId],
    ) {
        let t = cx.theme;

        // Use color + radius from theme
        let bg = t.colors.surface;
        let radius = t.geometry.button.radius;

        cx.scene.push_rounded_rect(rect, bg, radius, 0);

        // Example text paint
        let text_color = t.colors.text;
        let font_size = t.typography.font_size;

        // Your text engine call (example):
        // cx.scene.push_text(rect.origin, "Hello", text_color, font_size);
    }

    // ------------------------------------------------------------
    // 3. Input Events (optional)
    // ------------------------------------------------------------
    fn on_pointer_down(&mut self, _cx: &mut crate::core::event::EventContext, _pos: (f32, f32)) {
        // Handle press logic...
    }

    fn on_pointer_up(&mut self, _cx: &mut crate::core::event::EventContext, _pos: (f32, f32)) {
        // Handle release logic...
    }

    fn on_pointer_move(&mut self, _cx: &mut crate::core::event::EventContext, _pos: (f32, f32)) {
        // Handle hover/drag...
    }

    fn accepts_keyboard_focus(&self) -> bool {
        false
    }

    fn on_keyboard_activate(&mut self, _cx: &mut crate::core::event::EventContext) {
        // Activation logic...
    }
}

impl WidgetDowncast for MyWidget {}
```

---

# **Mandatory Rules Encoded in This Template**

### 1. **Theme Access**

Only via:

```
let t = cx.theme;
```

No constants.
No literals for padding, radius, font-size, colors.

### 2. **Geometry**

Always:

```
let geom = t.geometry.xyz;
```

Never:
❌ `20.0`, `14.0`, `6.0`, etc.

### 3. **Colors**

Always:

```
t.colors.surface
t.colors.text
t.colors.button_active
```

Never:
❌ `Color::from_hex(#...)` inside widgets.

### 4. **Typography**

Always:

```
t.typography.font_size
t.typography.line_height
```

### 5. **Interaction Timing**

Always:

```
t.interaction.click_interval_ms
t.interaction.hover_delay_ms
```

### 6. **Widget ID**

Mandatory:

```
id: WidgetInternalId
fn id(&self) -> WidgetInternalId
```

### 7. **Children**

If used, must be fed cleanly through layout and paint; otherwise ignored as `_children`.

### 8. **Input**

Pointer, keyboard, hover events must always use EventContext.

---

# **Optional Enhancements**

If you want, I can generate:

* A **TextField Template** with all the cursor logic hooks already wired.
* A **Composite Widget Template** (label + icon + layout rules).
* A **State Machine Template** implementing SPEC-HAL-100.
* A **Theme-driven Control variant template** (primary, danger, destructive, ghost, quiet, etc.).

---

# **Cheatsheet**

**Halcyon Theme Access Cheatsheet**
Direct, exhaustive, no fluff.
This is exactly what every widget must use — nothing else, ever.

---

# 1. Entry Point

From **LayoutContext** or **PaintContext**:

```
let t = cx.theme;
```

All widget geometry, colors, typography, interaction timing — everything — comes from `t`.

---

# 2. Geometry Access

## TextField

```
t.geometry.text_field.height
t.geometry.text_field.padding_x
t.geometry.text_field.padding_y
t.geometry.text_field.radius
t.geometry.text_field.icon_size
t.geometry.text_field.min_hit_size
```

## Button

```
t.geometry.button.height
t.geometry.button.padding_x
t.geometry.button.radius
```

## Checkbox

```
t.geometry.checkbox.size
t.geometry.checkbox.radius
```

## Icon

```
t.geometry.icon.size_small
t.geometry.icon.size_regular
t.geometry.icon.size_large
```

## Density

```
t.density   // Compact | Regular | Comfortable
```

---

# 3. Typography Access

```
t.typography.font_size
t.typography.line_height
t.typography.baseline_offset
t.typography.placeholder_opacity
t.typography.caret_thickness
```

---

# 4. Color Access

## Text

```
t.colors.text
t.colors.text_muted
t.colors.placeholder
```

## Surfaces

```
t.colors.background
t.colors.surface
t.colors.surface_hover
t.colors.surface_active
```

## Borders

```
t.colors.border
t.colors.border_focused
```

## Interactive

```
t.colors.primary
t.colors.selection
t.colors.caret
```

## Buttons

```
t.colors.button_background
t.colors.button_hover
t.colors.button_active
```

---

# 5. Interaction Timing

```
t.interaction.caret_blink_ms
t.interaction.click_interval_ms
t.interaction.hover_delay_ms
t.interaction.press_slop_px
```

---

# 6. Theme Logic

```
t.is_dark()                // bool
t.toggle()                 // flips dark/light
t.set_density(mode)        // Compact/Regular/Comfortable
```

---

# 7. Widget Implementation Patterns

## Layout

```
let geom = t.geometry.text_field;
let height = geom.height;
let width = constraints.max_width;
Size::new(width, height)
```

## Paint: Background

```
cx.scene.push_rounded_rect(rect, t.colors.surface, t.geometry.button.radius, 2);
```

## Paint: Focus Ring

```
cx.scene.push_rounded_rect(
    expanded_rect,
    t.colors.border_focused,
    t.geometry.button.radius + 2.0,
    1,
);
```

## Paint: Text

```
cx.scene.push_text(
    position,
    text,
    t.colors.text,
    t.typography.font_size,
)
```

## Caret

```
let caret_color = t.colors.caret;
let caret_thickness = t.typography.caret_thickness;
```

## Placeholder

```
Color::from_linear(
    t.colors.placeholder.r,
    t.colors.placeholder.g,
    t.colors.placeholder.b,
    t.typography.placeholder_opacity,
)
```

---

# 8. Forbidden

Widgets must **never** use:

❌ Hardcoded radii
❌ Hardcoded font sizes
❌ Hardcoded padding
❌ Hardcoded colors
❌ Legacy theme fields
❌ Accessing any appearance constants outside `cx.theme`

---

# 9. One-liner Summary

**Every pixel drawn or measured must come from one place: `cx.theme` — geometry, colors, typography, interaction timing.**

Nothing else.
