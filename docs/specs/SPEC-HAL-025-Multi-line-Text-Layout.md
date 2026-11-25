# SPEC-HAL-025 — Multi-line Text Layout (TextBlock)

## 1. Purpose

Introduce a first-class multi-line text layout engine for Halcyon, enabling:

* line wrapping
* paragraph breaking
* multi-line measurement
* multi-line painting
* text alignment (left, center, right)
* soft and hard line breaks

This provides the foundation for future rich text and editable text.

## 2. Scope

This SPEC defines:

* `TextBlock` widget
* `LineBreaker` subsystem
* `Paragraph` model
* integration rules for layout and paint

Not included:

* rich text spans
* markdown/HTML parsing
* cursor/selection/editing
* hyphenation

## 3. Coordinate Model

`TextBlock` adheres to standard Halcyon layout rules:

* layout produces a **rect** with top-left origin
* paint uses **top-left origin** for each line
* no baseline semantics are introduced in this SPEC

## 4. Text Model

### 4.1 Input

`TextBlock` accepts:

* `String text`
* `f32 font_size`
* `Color color`
* `TextAlign align` (Left, Center, Right)
* `bool wrap` (default true)

### 4.2 Paragraphs

Text is split into paragraphs by `\n`.
Each paragraph is independently line-broken.

## 5. LineBreaker

### 5.1 Inputs

* `&str text`
* `f32 max_width`
* `TextMetrics`
* word boundary rules: break at whitespace; fallback to grapheme

### 5.2 Output

`Vec<Line>` where `Line` contains:

* byte range into original string
* measured width
* metrics (line_height)
* shaped run

### 5.3 Algorithm

1. Split paragraph into tokens: words + whitespace
2. Greedy packing:

    * start a new line
    * add next word if width <= max_width
    * else break line
3. Fallback: if single word > max_width, break by characters

## 6. Layout Phase

### 6.1 measure()

For `TextBlock.measure(constraints)`:

1. If wrap = false:

    * shape entire text
    * width = run width
    * height = line_height
2. If wrap = true:

    * max_width = constraints.max_width
    * break into lines
    * width = max(line.width)
    * height = sum(line_height for all lines)

Return `(width, height)`.

## 7. Paint Phase

### 7.1 paint()

Given `rect`:

* start at `y = rect.y`
* for each line:

    * compute x based on alignment
    * draw shaped run at `(x, y)`
    * y += line_height

### 7.2 Clipping

`TextBlock.clip_bounds()` returns full rect.
Scrolling handled by ancestor `ScrollArea`.

## 8. TextBlock Widget

### 8.1 State

```
struct TextBlock {
    text: String,
    font_size: f32,
    color: Color,
    align: TextAlign,
    wrap: bool,
}
```

### 8.2 Behavior

* pure leaf widget
* no children
* layout computes internal line model and caches it
* paint uses cached lines

## 9. Caching Rules

* LineBreaker results cached per layout pass
* Recomputed only if:

    * text changed
    * font size changed
    * width constraint changed

## 10. Integration

### 10.1 With FlowGrid

FlowGrid treats a `TextBlock` as a normal sized widget.

### 10.2 With ScrollArea

Long text is scrollable.
No special handling.

## 11. Future Extensions

* SPEC-HAL-032: RichText spans
* SPEC-HAL-033: Editing model (TextField v2)
* SPEC-HAL-034: Full typography overhaul

## 12. Summary

This SPEC introduces a minimal but complete multi-line text layout system using Halcyon's existing coordinate model and text engine. It preserves stability, avoids baseline semantics, and creates the necessary foundation for richer text in future SPECs.
