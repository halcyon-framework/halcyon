# SPEC-HAL-061: Rich Clipboard Serialization & Styled Copy/Paste

RTF / HTML / Markdown Export, Styled Paste Import, Internal Rich-Span Encoding

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-060** (RichTextField / Rich Editing Model)
* **SPEC-HAL-056** (Clipboard Integration)
* **SPEC-HAL-052/055** (Editing & Undo/Redo)
* **SPEC-HAL-048/050/054** (Paragraph, Shaping, TextField Architecture)

Planned follow-ups:

* SPEC-HAL-062 (Embedded Widgets / Inline UI Elements)
* SPEC-HAL-063 (Document Model API: Blocks, Paragraphs)
* SPEC-HAL-064 (Export/Import to external formats)

---

## 1. Summary

This SPEC defines Halcyon’s **Rich Clipboard Model**:

* Copy/cut from RichTextField preserves styles and spans
* Clipboard contains multiple formats simultaneously:

    * Plain text (fallback)
    * Internal Rich-Span format (Halcyon-native)
    * RTF
    * HTML
    * Markdown (subset)
* Paste supports:

    * Full-fidelity Halcyon → Halcyon styled paste
    * Rich formats from other applications (HTML/RTF)
    * Fallback to plain text when styling cannot be preserved
* SecureEntry mode disallows styled content entirely
* Styled clipboard entries integrate into Undo/Redo as a single atomic transaction

This SPEC ensures Halcyon becomes a fully interoperable editor environment, not a sandbox.

---

## 2. Motivation

The modern clipboard is multi-format:

* Browsers copy HTML
* Word/Pages copy RTF + HTML
* Slack/Discord copy Markdown-like HTML text
* OS clipboards often contain both plain text + HTML simultaneously

Without styled clipboard integration, RichTextField becomes an isolated island.

We want:

* Copy styled text from Halcyon → paste into external apps
* Paste styled text from external apps → preserve formatting in RichTextField
* Intra-app rich copy/paste with perfect fidelity

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Copy places multiple formats on clipboard.
* R2: RichTextField copy includes:

    * Plain UTF-8
    * Halcyon-RichSpan JSON
    * HTML
    * RTF
    * Markdown (subset)
* R3: Paste prioritizes:

    1. Halcyon-RichSpan
    2. HTML
    3. RTF
    4. Markdown
    5. Plain text
* R4: Paste merges spans, handles IME cancellation, creates single undo transaction.
* R5: Copy/Cut in SecureEntry mode always plain-text empty (“”).
* R6: All styles cluster-safe and span-correct after paste.
* R7: Unsupported styles are gracefully downgraded.

### 3.2 Safety invariants

* I1: Clipboard deserialization must sanitize foreign HTML/RTF.
* I2: No script, style, or unsafe attributes allowed.
* I3: RichSpan JSON is private to Halcyon; never executed.
* I4: Undo/Redo must record full structured inserts/deletes.

---

## 4. Clipboard Data Formats

Halcyon will publish multiple MIME types.

### 4.1 Plain text (mandatory)

```
text/plain; charset=utf-8
```

### 4.2 Halcyon RichSpan format (primary)

```
application/x-halcyon-richtext
```

Payload:

```
{
  "spans": [
    {
      "text": "Hello ",
      "style": { ... full TextStyle ... }
    },
    {
      "text": "bold",
      "style": { "weight": "Bold", ... }
    }
  ]
}
```

Properties:

* Perfect fidelity
* All styles preserved
* Used for intra-Halcyon clipboard operations
* Fastest code path

### 4.3 HTML format

```
text/html
```

HTML subset:

* `<span style="">`
* `<b>`, `<strong>`, `<i>`, `<em>`
* `<u>`, `<mark>`
* `<font>` (legacy mapping)
* `<br>`
* Inline CSS whitelisted:

    * font-weight
    * font-style
    * color
    * background-color
    * text-decoration

Sanitization removes:

* `<script>`
* `<style>`
* `<img>` external sources
* `<a href>` optional, sanitized

### 4.4 RTF format

```
text/rtf
```

Limited conversion:

* bold, italic, underline
* colors
* font-size
* line breaks

Used for interoperability with Windows productivity apps.

### 4.5 Markdown format

```
text/markdown
```

Supports:

* `**bold**`
* `*italic*`
* `` `code` `` (mapped to monospace style)
* emojis as plain characters
* no multi-line paragraphs (converted)

---

## 5. Copy Pipeline (RichTextField)

### 5.1 Determine selection

If selection is empty → nothing to copy.
Else extract:

* Selected spans
* Trim spans to selection boundaries
* Remove empty spans

### 5.2 Produce formats

1. Plain text (UTF-8)
2. Halcyon JSON (lossless)
3. HTML
4. RTF
5. Markdown

ClipboardBackend receives all formats simultaneously.

### 5.3 SecureEntry mode

Only plain text = “” is output.

---

## 6. Cut Pipeline

Same as Copy, then:

* Begin undo transaction
* Remove selected spans
* End undo transaction

SecureEntry: cut deletes selection but clipboard empty.

---

## 7. Paste Pipeline (RichTextField)

On paste:

1. Cancel any IME composition

2. Retrieve clipboard available MIME types

3. Choose best-supported format in priority order:

    * Halcyon JSON
    * HTML
    * RTF
    * Markdown
    * Plain text

4. Deserialize to temporary `Vec<RichSpan>`

5. Begin undo transaction

6. If selection exists → delete

7. Insert deserialized spans

8. Merge spans

9. End undo transaction

10. Move cursor to end of paste

### 7.1 Span merging

Adjacent spans with identical style must be merged automatically.

### 7.2 Cluster safety

Cluster boundaries recomputed by ParagraphEngine after paste.

---

## 8. Deserialization Rules

### 8.1 Halcyon JSON

* Direct mapping back to spans and styles
* Guaranteed safe
* Fast-path

### 8.2 HTML → spans

Mapping:

* `<b>` → weight = Bold
* `<i>` → italic = true
* `<u>` → underline
* `<span style="">` → parse inline CSS
* `<mark>` → background highlight
* `<br>` → newline cluster

HTML sanitized via whitelist.

### 8.3 RTF → spans

* Parse basic RTF control words:

    * `\b`, `\i`, `\ul`
    * `\cfN` (color)
    * `\fsN` (size)
* Convert to spans

Ignore unsupported constructs.

### 8.4 Markdown → spans

Minimal parser:

* `**text**` → bold
* `*text*` → italic
* `` `text` `` → font = monospace
* newlines preserved

### 8.5 Plain text → one span

Plain text inserted using current style.

---

## 9. Rendering & Span Behavior After Paste

After paste:

* ParagraphEngine shapes runs with new spans
* Cursor moves to end of newly inserted content
* Multi-line paste introduces line breaks
* Selection removed

SecureEntry: result masked visually.

---

## 10. Undo/Redo Integration

Styled paste is a single undo transaction.

Undo restores previous span list.
Redo re-applies inserted spans.

Undo never reveals SecureEntry text visually; underlying spans preserved but masked.

---

## 11. Multi-Line & Mixed-Format Handling

* Multi-line HTML/RTF → newline mapping preserved
* Complex HTML spans that cannot be represented map to simplest-equivalent spans
* Hyperlinks preserved if sanitized

Examples:

```
<p>Hello <b>world</b></p>
```

→ two spans
→ newline if multiple `<p>` tags.

---

## 12. Performance Considerations

* JSON fast path for intra-Halcyon copy/paste
* HTML/RTF/Markdown parsing done on paste only
* Cache attributes for large text
* Parsing incremental only for pasted content, not whole document

---

## 13. Security Considerations

**Critical**:

* Sanitization mandatory
* Remove:

    * `<script>`
    * `<style>`
    * event handlers like `onclick=`
    * untrusted URLs
* Escape HTML entities correctly
* Ensure no RTF escapes trigger external loads

SecureEntry:

* All styled formats ignored
* Clipboard always receives empty plain-text

---

## 14. Migration Plan

### Phase 0

Implement Halcyon JSON serialization & deserialization.

### Phase 1

Implement plain-text integration with spans.

### Phase 2

HTML serializer + sanitizing parser.

### Phase 3

RTF importer/exporter (subset).

### Phase 4

Markdown importer/exporter.

### Phase 5

ClipboardBackend updated to support multiple MIME types.

### Phase 6

Integrate full pipeline into RichTextField.

---

## 15. Testing Strategy

* Copy/paste across Halcyon instances
* Paste HTML from browser
* Paste RTF from Word
* Paste Markdown snippets
* Multi-span cut/paste
* Styled undo/redo
* SecureEntry copy/cut behavior
* Mixed emoji + styled spans
* Newline handling
* Cluster correctness after paste
