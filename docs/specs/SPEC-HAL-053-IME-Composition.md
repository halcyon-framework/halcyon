# SPEC-HAL-053: IME Composition Model

Inline Composition, Pre-Edit Text, Candidate Windows, Commit/Cancel Semantics

Status: Design – Not Implemented
Author: JC
Depends on:

* **SPEC-HAL-052** (Interactive Text Editing Model)
* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-065** (Complex Text Shaping – rustybuzz)
* **SPEC-HAL-033/036** (Input Fusion & Coalescing)
* **SPEC-HAL-009** (Event Routing)

Planned follow-ups:

* SPEC-HAL-054 (TextField Architecture & Editable Paragraph Node)
* SPEC-HAL-055 (Undo/Redo Model)
* SPEC-HAL-056 (Clipboard Integration)

---

## Implementation Status

This SPEC describes the **planned** IME composition model for Halcyon. It is a pure design specification.

Current implementation status:

| Component | Status |
|-----------|--------|
| Paragraph layout (SPEC-HAL-048) | Implemented |
| Font resolution & fallback (SPEC-HAL-049) | Implemented |
| Shaping architecture (SPEC-HAL-065) | Implemented |
| Interactive editing model (SPEC-HAL-052) | **Not implemented** |
| IME composition engine | **Not implemented** |
| CompositionState type | **Not implemented** |
| Pre-edit text rendering | **Not implemented** |
| Composition spans in ParagraphEngine | **Not implemented** |
| Caret-in-composition logic | **Not implemented** |
| OS IME event mapping | **Not implemented** |
| Composition decoration rendering | **Not implemented** |

---

## 1. Summary

This SPEC defines Halcyon's **IME (Input Method Editor) composition model**, specifying:

* Pre-edit text (ongoing composition)
* Composition spans and temporary shaping
* Integration with ParagraphEngine
* Mapping OS-level IME events → Halcyon text editing pipeline
* Candidate window positioning (optional, provided by OS)
* Commit/cancel semantics
* Cursor behavior during composition
* Selection merging rules

IME is required for:

* CJK input (Chinese, Japanese, Korean)
* Complex-script keyboard entry (Indic, Thai)
* Phonetic input (Zhuyin, Pinyin)
* Predictive composition
* OS-provided autocorrect / candidate systems

This SPEC defines **how Halcyon must treat IME events**, and how they will integrate into the authoritative editing model defined in SPEC-HAL-052.

---

## 2. Motivation

All serious UI frameworks need:

* Inline IME composition
* Accurate positioning of composition text
* Stable pre-edit caret
* Synchronized commit/cancel with text editing model

Current Halcyon has:

* No IME integration
* No concept of pre-edit text
* No support for IME caret or composition spans

IME MUST NOT be simulated with normal text insertion — that leads to corrupted text, broken clusters, or lost candidate-state metadata.

A dedicated IME subsystem is required.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

These are **design requirements** for future IME implementation.

* R1: Support inline pre-edit text with composition styling.
* R2: Pre-edit glyphs must be shaped normally (rustybuzz).
* R3: Composition does not modify the underlying text until commit.
* R4: Cancelling composition restores the previous editing state.
* R5: Cursor must move inside composition region if needed.
* R6: IME events must not corrupt grapheme or cluster boundaries.
* R7: Selection logic must integrate with IME:

    * starting composition deletes selection
* R8: Pre-edit region must be visually distinguished (underlines, colors).

### 3.2 Event invariants

These invariants are **design requirements** for future IME event handling.

* I1: OS IME events are authoritative:

    * BeginComposition
    * UpdateComposition
    * CommitComposition
    * CancelComposition
* I2: Halcyon never guesses composition state.
* I3: All IME text must pass UTF-8 validation.
* I4: Composition may consist of multiple clusters.

### 3.3 Editing invariants

These invariants are **design requirements** for future EditingState extensions.

* E1: EditingState will store:

    * underlying text
    * composition text
    * composition cursor
* E2: Composition text will be shaped separately but integrated visually into paragraph.
* E3: ParagraphEngine will see composition text as a temporary span.

---

## 4. Data Model

> **Implementation status: Not implemented.** This section defines the planned data structures.

### 4.1 CompositionState

> Status: Planned type. Not present in the current codebase.

This struct defines the contract that future IME handling must implement.

```rust
pub struct CompositionState {
    pub active: bool,
    pub text: String,              // pre-edit text
    pub cursor: usize,             // position inside composition text (UTF-8 boundary)
    pub underline: CompositionDecoration,
}
```

### 4.2 CompositionDecoration

> Status: Planned type. Not present in the current codebase.

```rust
pub enum CompositionDecoration {
    Underline,
    DotUnderline,
    Highlight(Color),
    None,
}
```

These indicate IME visual feedback (platform-dependent).

### 4.3 EditingState extension

> Status: Planned extension to the EditingState type defined in SPEC-HAL-052. Not present in the current codebase.

This struct defines the contract that future editing state management must implement.

```rust
pub struct EditingState {
    pub text: String,
    pub cursor: TextCursor,
    pub selection: Option<TextSelection>,
    pub composition: Option<CompositionState>,
}
```

### 4.4 ParagraphEngine integration

> Status: Planned behavior. Not implemented.

Composition text will become an inserted span:

```
… underlying text before cursor …
[composition text]  <-- special style
… underlying text after cursor …
```

But underlying text will NOT change until commit.

---

## 5. IME Event Model

Halcyon's internal event model must handle IME-specific types:

### 5.1 BeginComposition

> Status: Planned event. Not implemented.

Triggered by OS IME start event.

Planned behavior:

* If selection exists → delete selection.
* Create `composition = CompositionState { active=true, text="", cursor=0 }`.
* Cursor remains at boundary inside composition.

### 5.2 UpdateComposition(text, caret_pos, decoration?)

> Status: Planned event. Not implemented.

Triggered when IME updates candidate string.

Planned behavior:

* composition.active must be true.
* Replace `composition.text` with new pre-edit string.
* Update `composition.cursor = caret_pos` (cluster boundary).
* Update decoration (underline/highlight).
* Underlying text remains unchanged.

ParagraphEngine will be re-run to display composition visually.

### 5.3 CommitComposition(text)

> Status: Planned event. Not implemented.

Triggered when IME finalizes the pre-edit text.

Planned behavior:

* Delete selection if exists (already done at begin).
* Insert `text` at cursor.position.
* cursor.position moves to end of inserted text.
* composition = None.

### 5.4 CancelComposition

> Status: Planned event. Not implemented.

Triggered when IME abandons pre-edit text.

Planned behavior:

* composition = None.
* Underlying text remains unchanged.
* Cursor remains where it was at begin.

---

## 6. Rendering of Composition Text

> **Implementation status: Not implemented.** This section defines the planned rendering behavior.

Composition text will:

* Use paragraph shaping with special span style.
* Typically be drawn with:

    * underline
    * highlighted background
    * dotted underline

### 6.1 Special composition span style

> Status: Planned styling. Not implemented.

```rust
TextStyle {
    families: same as base style,
    weight: same,
    italic: same,
    font_size: same,
    color: base color,
    composition_decoration: Some(decoration)
}
```

Decoration will be applied in the renderer (not ParagraphEngine).

### 6.2 Cursor inside composition

> Status: Planned behavior. Not implemented.

Cursor will be drawn inside the composition span at:

```
composition.byte_cursor_position → cluster boundary
```

---

## 7. Cluster-Safe IME Cursor Handling

> **Implementation status: Not implemented.** This section defines the planned cursor handling.

Cursor inside composition text must follow cluster boundaries determined by:

* Composition text → shaped independently
* rustybuzz clusters → authoritative
* ParagraphEngine merges composition glyphs temporarily

Cursor Left/Right inside composition:

* Will move within composition text clusters, not underlying text.

When exiting composition (commit/cancel), cursor will jump back to underlying text boundary.

---

## 8. Composition & Selection Interactions

> **Implementation status: Not implemented.** This section defines the planned interaction behavior.

### 8.1 If selection exists when composition begins:

Planned behavior:

```
delete selection
cursor = selection.anchor
start composition at cursor
```

### 8.2 During composition

Selections will not span across composition boundaries.
If user tries to select underlying + composition region simultaneously:

* Selection applies only to composition text (expected IME behavior).

### 8.3 After commit

A new selection will not be created; cursor ends at end of committed text.

---

## 9. Hit-Testing With Composition Text

> **Implementation status: Not implemented.** This section defines the planned hit-testing behavior.

Hit-testing must:

* Treat composition glyphs as part of text flow.
* Map pointer position → cluster start in:

    * composition text if pointer hits composition glyphs
    * underlying text otherwise

Separate mapping needed for:

* underlying clusters
* composition clusters

ParagraphEngine will merge both for visual layout; hit-test will use merged cluster list.

---

## 10. Integration With ParagraphEngine

> **Implementation status: Not implemented.** This section defines the planned integration.

ParagraphEngine must treat composition text as a temporary span inserted at runtime.

Planned pipeline:

```
underlying text
insert: composition text at cursor.position
shaping
cluster creation
line breaking
visual assembly
```

Composition spans will not affect:

* line metrics across paragraphs
* font fallback caches
* resolved font faces

Composition spans are ephemeral.

---

## 11. Backend Integration

IME composition is not backend-specific.
Native backends (DirectWrite, CoreText, FreeType) have **no awareness** of composition state.

IME handling will occur purely inside:

* EditingState
* ParagraphEngine
* Renderer (for decoration)
* Event routing

---

## 12. Migration Plan

### Phase 0 — Extend EditingState with composition field

Infra only.

### Phase 1 — Handle Begin/Update/Commit/Cancel IME events

Map OS IME events → Halcyon events.

### Phase 2 — Integrate composition shaping

ParagraphEngine merges composition span.

### Phase 3 — Cursor + selection rewrite

Composition-aware cursor logic.

### Phase 4 — Hit-testing support

Cluster mapping includes temporary composition clusters.

### Phase 5 — Render-time decoration

Underline, highlight, IME-caret.

### Phase 6 — Full TextField integration

Finish in SPEC-HAL-054.

---

## 13. Testing Strategy

* Japanese Hiragana → Kanji conversion sequences.
* Korean Hangul composition (NFC):

    * ㄱ + ㅏ → 가
    * full multi-step sequences
* Chinese Pinyin → Hanzi
* Candidate cycling without commit
* Cancel mid-sequence
* Insert composition into middle of paragraph
* RTL text with IME (Arabic shaping)

---

## 14. Performance Notes

* Composition re-layout is cheap; text size small.
* Composition updates must be near-instant.
* Shaping via rustybuzz is fast enough for real-time IME.

---

## 15. Security Considerations

* UTF-8 validation mandatory.
* Composition text must not escape visual bounds.
* IME-provided offsets must be validated (no OOB).

---

## 16. Relationships

| SPEC | Description | Status |
|-----|-------------|--------|
| SPEC-HAL-048 | Paragraph & Text Layout Engine | Implemented |
| SPEC-HAL-049 | Font Collections & Fallback Rules | Implemented |
| SPEC-HAL-050 | Unified Text Shaping Architecture | Implemented |
| SPEC-HAL-065 | Complex Text Shaping (rustybuzz) | Implemented |
| SPEC-HAL-052 | Interactive Text Editing Model | Design – depends on this SPEC for IME events |
| SPEC-HAL-054 | TextField Widget | Future – will consume this SPEC's composition model |
| SPEC-HAL-055 | Clipboard & Undo/Redo | Future |

This SPEC (053) serves as the **design foundation** for IME composition handling. It defines contracts and invariants that SPEC-054 (TextField) and future editing implementations must follow.
