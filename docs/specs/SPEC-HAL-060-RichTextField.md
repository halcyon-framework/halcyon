# SPEC-HAL-060: RichTextField — Multi-Span Editable Text With Inline Styling

Bold/Italic/Color Spans, Mixed Fonts, Embedded Emojis, Hyperlinks, Unified Editing Model

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-048** (Paragraph Engine)
* **SPEC-HAL-049** (Font Resolver / Fallback)
* **SPEC-HAL-050** (HarfBuzz Shaping)
* **SPEC-HAL-052** (Editing Model)
* **SPEC-HAL-053** (IME Composition)
* **SPEC-HAL-054** (TextField Architecture)
* **SPEC-HAL-055** (Undo/Redo)
* **SPEC-HAL-056** (Clipboard)
* **SPEC-HAL-057** (SecureEntry)
* **SPEC-HAL-058** (Word Navigation)
* **SPEC-HAL-059** (Touch/Virtual Keyboard Model)

Planned follow-ups:

* SPEC-HAL-061 (RichText Clipboard Model: RTF/HTML/Markdown serialization)
* SPEC-HAL-062 (Inline Widgets / Embedded Controls)
* SPEC-HAL-063 (Document Model API: Blocks, Paragraphs, Runs)

---

## 1. Summary

This SPEC defines **RichTextField**, Halcyon’s canonical multi-style editable text widget:

* Multiple spans inside a single editable node
* Inline formatting attributes (weight, italic, color, background, underline, etc.)
* Mixed fonts & font sizes
* Emoji inline support
* Hyperlinks with click/hover states
* Single paragraph or multi-line
* Robust editing & cursor navigation with spans
* Copy/Cut/Paste preserving attributes (future SPEC-061)
* Cluster-safe editing just like TextField
* Unified interaction model for keyboard, mouse, touch, IME

This is the beginning of Halcyon’s higher-level document editing architecture.

---

## 2. Motivation

Plain TextField cannot support:

* syntax highlighting
* inline bold/italic
* text coloring
* clickable links
* inline icons/emoji
* rich clipboard content
* true text editors / document interfaces
* chat messages with emojis & styled segments

We need a higher-level widget maintaining **styled runs** inside a single editing environment.

This must extend—not replace—the entire editing stack already built.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Editable text consists of a list of styled spans.
* R2: Spans merge automatically when adjacent and share style.
* R3: Cursor & selection operate across spans seamlessly.
* R4: Editing operations respect span boundaries (but can merge/split).
* R5: Rendering uses ParagraphEngine’s multi-run shaping.
* R6: Copy/Cut optionally preserves style (future SPEC-061).
* R7: Paste defaults to plain-text (styles stripped) unless extended.
* R8: IME composition inserted as its own temporary style span.
* R9: Undo/Redo reversible and span-safe.
* R10: RichTextField works in single-line or multi-line mode.

### 3.2 Style invariants

Each span has:

```
style: TextStyle {
    families: FontStack,
    weight: FontWeight,
    italic: bool,
    color: Color,
    background: Option<Color>,
    underline: Option<UnderlineStyle>,
    font_size: f32,
}
```

Invariant:

* I1: Style applies uniformly to all clusters in span.
* I2: Spans are minimal: no adjacent spans share identical style.
* I3: Span break boundaries = cluster boundaries as produced by shaping.

### 3.3 Editing invariants

* E1: Cursor positions always between clusters, NOT inside spans.
* E2: Insert/delete may split spans.
* E3: Selection may span multiple spans.
* E4: Undo/Redo stores full span list per transaction.
* E5: Composition temporarily creates its own span with IME decorations.

---

## 4. Data Model

### 4.1 RichSpan

```
pub struct RichSpan {
    pub text: String,
    pub style: TextStyle,
}
```

### 4.2 RichTextModel

```
pub struct RichTextModel {
    pub spans: Vec<RichSpan>,
}
```

### 4.3 RichEditingState

Superset of EditingState:

```
pub struct RichEditingState {
    pub model: RichTextModel,
    pub cursor: TextCursor,
    pub selection: Option<TextSelection>,
    pub composition: Option<CompositionState>,
    pub history: UndoHistory,
}
```

### 4.4 Selection Range

Same cluster boundary rules; selection spans may intersect multiple spans.

---

## 5. Rendering Pipeline

The RichTextField organizes spans into **Paragraph Spans**:

```
ParagraphSpan {
    text: span.text,
    style: span.style,
}
```

ParagraphEngine shapes runs in logical order:

* span boundaries → run boundaries
* style changes → run boundaries
* font fallback split → run boundaries

RichTextField is essentially a ParagraphField with editing enabled.

---

## 6. Editing Rules

### 6.1 Insert text

Insertion at cursor:

1. Find span containing cursor.
2. If inserting plain text:

    * insert into existing span
3. If inserting styled text (future SPEC):

    * create new span(s)
4. After insertion:

    * Split span if necessary
    * Merge adjacent spans with same style

### 6.2 Delete (selection delete)

1. For each span intersecting selection:

    * Remove selection portion
2. If span becomes empty → remove
3. Merge neighbors if style identical

### 6.3 Backspace (cluster-delete left)

Cluster deletion works like TextField, but:

* If deletion crosses span boundary, merge logic kicks in
* Cluster-level intersection resolved per underlying cluster map

### 6.4 Delete forward

Same but to the right.

### 6.5 Span merging

After any edit:

```
if spans[i].style == spans[i+1].style:
    merge into single span
```

Span merging ensures minimal representation.

---

## 7. IME Integration

IME composition becomes its own temporary span:

```
[ span_before ][ composition_span ][ span_after ]
```

Where:

```
composition_span.style = base_style cloned + composition_decoration
```

On commit:

* Replace composition_span with final text using final style
* Merge neighbors if needed

On cancel:

* Remove composition_span
* Cursor stays at boundary.

---

## 8. Hit-Testing

Hit-testing uses paragraph cluster map, not spans directly.

Pipeline:

1. Hit-test into ParagraphEngine output
2. Obtain cluster index
3. Map cluster index → span + byte offset
4. Cursor moves accordingly

Span boundaries do not affect hit-testing; cluster maps bridge the two.

---

## 9. Undo/Redo Integration

RichEditingState history entries include:

* Full span list before
* Full span list after
* Cursor + selection before/after

Transactions created exactly like SPEC-055.

Span merging/splitting is part of the after-state.

Undo never partially restores spans; entire model restored.

---

## 10. Clipboard Behavior

### 10.1 Default behavior (this SPEC)

Clipboard operations use **plain text** only.

Copy:

* Extract underlying text across spans
* Lose styles

Cut:

* Remove spans corresponding to selection
* Clipboard receives plain text

Paste:

* Insert plain text using current span’s style

### 10.2 Extended Rich Clipboard (SPEC-061)

Future SPEC introduces:

* Styled serialization (RTF/HTML/Markdown)
* Preservation of formatting
* Cross-app rich paste

This SPEC only defines plain text handling.

---

## 11. Cursor Movement & Word Navigation

### 11.1 Cursor movement

Same as SPEC-052, but across span boundaries seamlessly.

### 11.2 Word navigation

Word boundaries are derived from underlying full text across spans; styling irrelevant.

### 11.3 Home/End

Per-line and per-document rules still apply.

---

## 12. SecureEntry Mode

SecureEntry on RichTextField applies only to the **visual text**:

* Replace all visible characters with mask glyph
* All spans forced to same masked style
* Underlying spans preserved but not visible
* Clipboard defaults to empty copy/cut
* Paste inserts unstyled text

Span information preserved for undo/redo but never revealed visually.

---

## 13. Hyperlinks & Interactive Spans

Not fully defined here; this SPEC reserves:

* `style.link: Option<Url>`
* future event routing for click/hover
* future visual decoration (underline, color, hover highlight)

Editing inside a hyperlink is allowed; hyperlink attribute remains on edited span unless split.

---

## 14. Multi-Line Model

RichTextField supports:

* Soft-wrapping
* Hard line breaks (newline insertion)
* Multi-line selection
* Multi-span paragraph layout

ParagraphEngine handles visual layout; RichTextField provides underlying structure.

---

## 15. Rendering

Rendering relies on:

* ParagraphEngine’s shaped runs
* Span attributes for:

    * foreground color
    * background color
    * underline
    * font size
    * fallback font resolution

Rich spans → paragraph runs → glyph quads → renderer.

Selection rendering crosses spans visually.

---

## 16. Migration Plan

### Phase 0

Add RichSpan, RichTextModel, RichEditingState.

### Phase 1

Integrate ParagraphEngine with spans → paragraph spans → shaping.

### Phase 2

Implement editing ops:

* insert/delete/backspace
* span splitting and merging
* composition spans

### Phase 3

Render rich spans using renderer.

### Phase 4

Enable selection handles, IME, touch, scroll.

### Phase 5

Expose public RichTextField API.

---

## 17. Testing Strategy

* Mix bold/plain text editing
* Backspace across span boundaries
* Emoji spans
* Mixed font families
* Hyperlinks
* IME composition in styled spans
* Undo/Redo restoring full styling
* Clipboard plain-text behavior
* Multi-line rich text editing
* Span merge correctness
* SecureEntry masking across spans
