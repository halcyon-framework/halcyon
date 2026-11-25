# SPEC-HAL-057: SecureEntry (PasswordField) Mode

Masked Rendering, Editing Isolation, Clipboard Restrictions, Undo/Redo Hygiene

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-052** (Editing Model)
* **SPEC-HAL-053** (IME Composition Model)
* **SPEC-HAL-054** (TextField Architecture)
* **SPEC-HAL-055** (Undo/Redo System)
* **SPEC-HAL-056** (Clipboard Integration)
* **SPEC-HAL-048** (Paragraph & Layout Engine)

Planned follow-ups:

* SPEC-HAL-058 (Text Commands: Word-Jump, Word-Delete, Navigation Granularity)
* SPEC-HAL-059 (Touch Keyboard / Virtual Keyboard Integration)
* SPEC-HAL-060 (Multiline RichTextField)

---

## 1. Summary

This SPEC defines **SecureEntry Mode**, Halcyon’s standardized password field behavior:

* Masking of visible glyphs (● / • / ❖ configurable)
* Exact-text preservation internally (editing operates on real text)
* Clipboard restrictions (no copy/cut)
* Undo/Redo with privacy rules (history never exposes clear text to renderer)
* IME restrictions (optional disablement)
* Cursor & selection behavior remain identical
* Hit-testing based on masked glyph widths but logical positions remain correct
* No text leakage through layout engine or renderer logs

This produces a safe, correct password field compatible with all previous text SPECs.

---

## 2. Motivation

Password fields require special handling:

* Must NOT show actual characters.
* Must NOT allow copying visible text.
* Must preserve underlying text for editing and IME commit.
* Must avoid history storing intermediate revealable states.
* Must not alter editing model (SPEC-052) semantics.

Halcyon must implement secure entry robustly without compromising the rest of the text stack.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Display mask characters instead of actual text.
* R2: Cursor movement matches underlying text, not mask glyphs.
* R3: Selection highlights width of mask glyphs but never reveals underlying text.
* R4: Copy/Cut disabled.
* R5: Paste allowed (but text never revealed in clipboard).
* R6: Undo/Redo stores real text but must never expose it visually.
* R7: IME composition allowed but masked until commit.
  *Optional future flag: disable IME in password mode.*

### 3.2 Rendering invariants

* I1: Renderer must never receive underlying glyphs or clusters.
* I2: ParagraphEngine must construct layout using masked text.
* I3: Mask characters must behave as monospace or consistent-width for predictable UX.
* I4: Underlying text and layouts must stay separate.

### 3.3 Editing invariants

* E1: EditingState always stores real text.
* E2: CompositionState stores real composition text.
* E3: Cluster boundaries based on real text.
* E4: Cursor logic based on real text clusters.
* E5: Undo/Redo operates on real text only.

### 3.4 Security invariants

* S1: No debug logs show real text.
* S2: Render tree never contains real glyphs.
* S3: Clipboard copy returns empty.
* S4: Clipboard cut returns empty.
* S5: No lingering buffers of real glyph shapes.
* S6: Password fields should avoid platform “secure text entry” warnings by default.

---

## 4. Data Model

### 4.1 TextFieldState extension

```
pub struct TextFieldState {
    pub secure: bool,
    pub mask_char: char, // default: '•'
    ...
}
```

### 4.2 MaskedParagraph

ParagraphEngine receives:

* full real text (for layout metadata)
* masked substituted text (to shape/render)

Workflow:

```
real_text → cluster map (SPEC-052)
masked_text (same number of clusters) → shaping & glyph generation
```

We maintain 1:1 mapping:

```
real cluster[i] ↔ masked cluster[i]
```

---

## 5. Masking Rules

### 5.1 Default mask character

Default: U+2022 (•)
Alternatives: U+25CF (●), U+2736 (✶), custom.

### 5.2 Masking per cluster

For each cluster:

```
render_glyph = mask_char
```

### 5.3 Width consistency

To ensure predictable cursor movement:

* mask_char must be shaped per its natural metrics
* but cursor x-position is derived from shaped masked glyphs
* cluster count remains identical

Thus user-visible caret movement corresponds 1:1 to character positions.

### 5.4 Masking IME composition

During composition:

* Underlying real pre-edit characters exist
* UpdateComposition uses masked string:

    * If IME provides composition text "ka":

        * display "••"
    * cursor inside composition moves between masked clusters

Commit: underlying real text inserted.

Cancel: revert without display.

---

## 6. Editing Behavior in Secure Mode

The editing model (SPEC-052) does not change.
Only the visual representation changes.

### 6.1 Cursor movement

Cursor moves through underlying clusters; mask text ensures exact glyph spacing.

### 6.2 Backspace/delete

Deletes real clusters — masked text updated shape accordingly.

### 6.3 Insert

Typing inserts underlying characters but only masked glyphs are shown.

### 6.4 Undo/Redo

Undo/Redo manipulates real text only.
Masked representation recomputed after each edit.

---

## 7. Clipboard Behavior

SecureEntry modifies clipboard rules (SPEC-056):

### 7.1 Copy

Always copies empty string.

### 7.2 Cut

Deletes selection but copies empty string.

### 7.3 Paste

Allowed.
Clipboard text inserted into underlying text, shown masked.

### 7.4 Selection visibility

Selection highlight shows mask-glyph width only.

---

## 8. Layout & Rendering Integration

### 8.1 ParagraphEngine modifications

ParagraphEngine must:

* Produce cluster map from underlying text
* Produce shaped glyphs from masked text
* Use underlying text only for:

    * editing cursor positions
    * cluster maps
    * logical indices
* Use masked-only text for:

    * shaping
    * glyph quads

### 8.2 Renderer guarantees

Renderer MUST NOT receive the underlying text.
Only masked glyph IDs and quads are produced.

### 8.3 Cursor rendering

Cursor x-position uses masked glyph positions.
Again: masked and real text share identical cluster counts, so cursor aligns.

### 8.4 Selection rectangles

Based on masked glyph positions.
Never expose underlying characters.

---

## 9. IME Behavior in Secure Mode

IME pre-edit text is masked until committed.

Pipeline:

1. BeginComposition
2. UpdateComposition(text="あ")

    * displayed as masked
3. CommitComposition

    * real underlying text inserted

IME cursor inside composition is determined from masked shaping.

Optional per-widget configuration:
`disable_ime_in_secure_mode: bool`.

If enabled:

* All IME events ignored
* Only direct keyboard input allowed (ASCII-only inputs)

---

## 10. Security Considerations

### 10.1 Never expose real glyphs

* ParagraphEngine must forbid resolving real glyph IDs.
* Underlying text never sent to HarfBuzz for shape display.
* Only masked text is shaped.

### 10.2 No memory leakage

* Renderer must not store persistent glyph data that exposes underlying shapes.
* Underlying strings live only in EditingState and UndoHistory.
* Optional: wipe underlying text on unmount/destruction.

### 10.3 Clipboard restrictions

* Prevents accidental password leakage.

### 10.4 Logging restrictions

* Debug logs sanitized automatically if secure=true.

### 10.5 Side-channel considerations

While not fully eliminating side-channel leaks (timing, UI animation measurements), this SPEC provides strong protection against:

* Shoulder-surfing
* Clipboard leakage
* Debug/log leakage
* Renderer leaks
* IME candidate window exposure (IME candidates show derived text, not underlying actual typed characters unless IME provides them)

Real-world secure entry will be fully addressed when porting Halcyon to mobile.

---

## 11. Migration Plan

### Phase 0 — Extend TextField with `secure: bool` and `mask_char`

Add flags without changing behavior.

### Phase 1 — MaskedParagraph support

ParagraphEngine reshaped to separate:

* cluster map from underlying text
* shaping from masked text

### Phase 2 — EditingState integration

Ensure all mutation methods operate normally.

### Phase 3 — Clipboard restrictions

Implement copy/cut disabled.

### Phase 4 — IME safe masking

Apply masked composition spans.

### Phase 5 — Renderer updates

Ensure only masked glyphs rendered.

### Phase 6 — Final integration & tests

Full secure entry ready.

---

## 12. Testing Strategy

* Type passwords (ASCII, Unicode, emoji)
* Multi-cluster characters masked as single glyph
* Backspace deletes correctly
* Paste multi-line strings
* Undo/Redo preserves real text
* IME composition masked
* Clipboard copy outputs empty
* Render tree contains no underlying text
* Stress tests (long passwords)
