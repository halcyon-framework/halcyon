# SPEC-HAL-062: Inline Widgets in RichText — Embedded Elements, Chips, Icons, Images, UI Nodes Inside Text

Unified Inline Element Model for RichTextField & Document Engine

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-060** (RichTextField)
* **SPEC-HAL-061** (Rich Clipboard)
* **SPEC-HAL-048** (Paragraph Engine / Runs)
* **SPEC-HAL-050** (Shaping / HarfBuzz)
* **SPEC-HAL-052** (Editing Model)
* **SPEC-HAL-053** (IME)
* **SPEC-HAL-055** (Undo/Redo)
* **SPEC-HAL-054** (TextField Architecture)

Planned follow-ups:

* SPEC-HAL-063 (Block-level Document Model: Paragraphs, Blocks, Lists)
* SPEC-HAL-064 (Image Loading, Caching, Async Glyphs)
* SPEC-HAL-065 (Interactive Embedded Widgets API)
* SPEC-HAL-066 (Markdown+HTML Import/Export w/ Inline Elements)

---

## 1. Summary

This SPEC defines **inline widgets** — arbitrary UI elements embedded directly inside text runs:

* Images (emoji-like)
* Icons (vector or raster)
* Chips / tags / badges
* Inline code tokens (rendered as mini-blocks)
* Inline UI nodes (toggle, checkbox, etc.)
* Custom embedded components rendered via user logic

These inline elements behave like **single glyphs** for:

* hit-testing
* cursor movement
* deletion/backspace
* selection
* line wrapping
* copy/paste
* undo/redo

This SPEC formalizes the InlineElement model and how it integrates with the ParagraphEngine and RichTextEditingState.

---

## 2. Motivation

Modern editors and chat apps include rich inline elements:

* Discord / Slack style emoji
* VSCode inline error chips
* Notion inline UI tokens
* Code editors with inline widgets
* Embedded image thumbnails inside text

RichTextField cannot reach modern parity without:

* an extensible inline element representation
* integrated with shaping, editing, layout, clipboard, undo/redo

Inline widgets must be first-class citizens in Halcyon’s text graph.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Inline elements behave as atomic cluster units.
* R2: They occupy space in layout with explicit advance width + ascent/descent.
* R3: Cursor can be before/after, not inside them.
* R4: Backspace/delete removes them as whole units.
* R5: Copy/cut/paste preserves inline elements (via Halcyon JSON).
* R6: Hit-testing treats them as supersized glyphs.
* R7: Inline widgets can have custom painting logic.
* R8: Inline widgets participate in selection (highlight fully).
* R9: Undo/Redo includes insertion/removal of inline widgets.

### 3.2 Structural invariants

* I1: Inline element occupies exactly one cluster.
* I2: Inline element is stored as a span containing:

    * placeholder character (U+FFFC “Object Replacement Character”)
    * metadata representing the inline widget
* I3: InlineRenderingObject defines its metrics: width, ascent, descent.
* I4: Inline elements live inside RichSpans, but with dedicated style “inline style”.

### 3.3 Editing invariants

* E1: Cursor never placed inside inline widget; only before or after.
* E2: IME composition cannot insert inline widgets (but can appear before/after).
* E3: Word navigation treats inline elements as standalone “word units”.
* E4: Paste with inline widgets allowed only via Halcyon JSON; HTML/RTF degrade to plain if unsupported.

---

## 4. Inline Element Model

### 4.1 Placeholder mechanism

Inline elements are represented in text as:

```
U+FFFC (OBJECT REPLACEMENT CHARACTER)
```

Underlying RichSpan text looks like:

```
"Hello ", "\u{FFFC}", " world"
```

### 4.2 Metadata storage

```
pub enum InlineElementKind {
    Image(InlineImage),
    Icon(InlineIcon),
    Chip(InlineChip),
    Widget(InlineWidget),
    Custom(Box<dyn InlineRenderable>),
}
```

Each inline element has:

```
pub struct InlineElement {
    id: InlineElementId,
    kind: InlineElementKind,
    metrics: InlineMetrics,
}
```

### 4.3 InlineMetrics

```
pub struct InlineMetrics {
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
}
```

Metrics are precalculated by:

* Image intrinsic size
* Icon size
* Chip layout engine
* Widget-rendering logic
* User-override

---

## 5. Rendering Integration

ParagraphEngine treats inline widgets as:

* Single cluster
* Single glyph run
* Advance = metrics.advance
* Ascent/Descent override text style
* No font fallback (they’re not glyphs)

Renderer draws inline widgets after glyphs in the same run, in Z-order.

Pseudo-pipeline:

1. Shape regular glyphs
2. For OBJECT REPLACEMENT:

    * skip HarfBuzz shaping
    * insert synthetic glyph quad
    * attach inline element to that quad
3. Renderer:

    * draws text
    * then calls inline_element.paint(context, rect)

---

## 6. Editing Integration

Inline widgets behave as clusters:

### 6.1 Cursor movement

* Left of inline
* Right of inline
* Never inside

### 6.2 Backspace/Delete

* Backspace removes inline element to the left
* Delete removes inline element to the right
* Removal is full-span if span contains only the object

### 6.3 Insert behavior

Inserting text next to inline widget splits span if required.

### 6.4 Selection

Selection highlight covers the placeholder’s rect.

### 6.5 Word navigation

Inline widget = separate “word unit”.

### 6.6 IME

IME composition cannot modify inside inline widget.
Cursor placed before/after.

---

## 7. Clipboard Integration

### 7.1 Halcyon JSON (full fidelity)

Inline elements serialized into JSON:

```
{
  "spans": [
    { "text": "Hello ", "style": {...} },
    {
      "text": "\uFFFC",
      "style": {... inline style ...},
      "inline": {
         "id": "uuid",
         "kind": "Image",
         "data": { ... image metadata ... }
      }
    },
    { "text": " world", "style": {...} }
  ]
}
```

### 7.2 HTML

Limited support:

* Convert inline widgets to `<img src="...">` if data URL or safe URL available
* Otherwise degrade to empty span

### 7.3 RTF

Unsupported → degrade to plain text

### 7.4 Markdown

Unsupported → degrade to plain text

### 7.5 Plain text

U+FFFC removed; no placeholder-written.

---

## 8. Paste

### Paste priority

1. Halcyon JSON (full fidelity, inline preserved)
2. HTML (may attempt to rehydrate images)
3. RTF (ignored inline)
4. Markdown (ignored inline)
5. Plain text (inline removed)

---

## 9. Interaction & Event Routing

Inline widgets may be interactive:

* Chip with “X” button
* Inline toggle
* Inline label with hover
* Inline dropdown indicator
* Image preview

### 9.1 Hit-testing

Hit-test pipeline checks:

1. ParagraphEngine cluster index
2. Check if placeholder is inline element
3. If inline is interactive:

    * route tap/click to inline element
    * allow custom behavior (toggle, open menu, etc.)

### 9.2 Hover/click

InlineElementKind::Widget receives:

```
fn on_pointer_down(...)
fn on_pointer_up(...)
fn on_pointer_move(...)
fn on_hover(...)
```

### 9.3 Focus model

RichTextField remains main focus owner; inline widgets do not steal focus unless explicitly allowed.

---

## 10. Layout of Inline Widgets

* Inline elements participate fully in text flow
* Metrics control baseline alignment
* Chips and icons can be taller than text; ascent/descent adjusted
* If inline widget too large → baseline shift or line-height increase

Line breaks:

* Inline elements break like regular glyphs
* If too wide → wrap to next line

---

## 11. Undo/Redo

Inline widget operations:

* Insert inline element
* Remove inline element
* Replace range with text + inline
* Move cursor around inline

Undo/Redo stores full span list and inline metadata.

---

## 12. SecureEntry Mode

SecureEntry replaces **all characters** visually with mask glyph.

Inline widgets still visible or not depending on configuration:

* Default: *visible* (like password manager bubble icons)
* Optional: hidden or replaced with a generic mask element

Clipboard: inline elements never copied in secure mode.

---

## 13. Performance Considerations

* Inline widgets may require off-thread image decode (future SPEC-064)
* Metrics precomputed; painting cheap
* Renderer draws inline in a separate pass
* Large embedded images must be async-loaded

---

## 14. Migration Plan

### Phase 0

Add InlineElement structures + metrics.

### Phase 1

Add U+FFFC handling in ParagraphEngine.

### Phase 2

Add RichTextField support for inline insertion/deletion.

### Phase 3

Add renderer path for inline elements.

### Phase 4

Add interactive inline widget API.

### Phase 5

Add clipboard support.

### Phase 6

Integrate with user-facing RichTextField.

---

## 15. Testing Strategy

* Insert inline icons/images
* Delete inline widgets
* Backspace across inline boundaries
* Selection ranges including inline
* Copy/paste (JSON → HTML → plain)
* Line wrapping around inline widgets
* Interactive inline widget hit-testing
* SecureEntry masking
* Undo/Redo full fidelity
* Mixed spans text+inline navigation
* Multi-line rich text with embedded icons
