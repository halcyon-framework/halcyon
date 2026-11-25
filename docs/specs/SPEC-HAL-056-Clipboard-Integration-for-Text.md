# SPEC-HAL-056: Clipboard Integration for Text Editing

Copy, Cut, Paste, Selection Mapping, IME Safety, Multi-Line & Multi-Widget Support

Status: Draft
Author: JC
Depends on:

* **SPEC-HAL-052** (Interactive Editing Model)
* **SPEC-HAL-053** (IME Composition)
* **SPEC-HAL-054** (TextField Architecture)
* **SPEC-HAL-055** (Undo/Redo System)
* **SPEC-HAL-048** (Paragraph & Text Layout Engine)
* **SPEC-HAL-009** (Event Routing)

Planned follow-ups:

* SPEC-HAL-057 (SecureEntry / PasswordField Mode)
* SPEC-HAL-058 (Text Commands / Word-Jump / Word-Delete)
* SPEC-HAL-059 (TextField Virtual Keyboard Model for Touch Devices)

---

## 1. Summary

This SPEC defines Halcyon’s **clipboard integration**:

* Copy (Ctrl+C)
* Cut (Ctrl+X)
* Paste (Ctrl+V)
* UTF-8–safe clipboard handling
* Multi-line support
* Proper integration with Undo/Redo
* Respect composition boundaries
* Infrastructure for multiple TextFields/widgets
* Integration with platform-native clipboards (Windows, macOS, Linux/X11, Wayland)

This SPEC ensures that clipboard operations behave exactly like modern text editors—cluster-safe, fully transactional, and IME-correct.

---

## 2. Motivation

Without clipboard support, Halcyon cannot support:

* Basic text editing
* Code editors
* Chat inputs
* Any standard input box

Clipboard operations must be:

* Fast
* UTF-8 valid
* Integrated with undo
* Consistent across all platforms
* Secure (no leak of secure-entry text)

This SPEC formalizes how Halcyon performs these operations.

---

## 3. Requirements & Invariants

### 3.1 Functional requirements

* R1: Copy copies selected text to clipboard as UTF-8.
* R2: Cut deletes selection after copying.
* R3: Paste inserts clipboard text at cursor.
* R4: Paste must be a single atomic undo transaction.
* R5: IME composition must be cancelled before paste.
* R6: Clipboard operations must not break cluster boundaries.
* R7: Clipboard text must be sanitized prior to insertion.
* R8: Clipboard system must support multi-line text.

### 3.2 Platform invariants

* Windows: Win32 `OpenClipboard`, `GetClipboardData`, `SetClipboardData`
* macOS: NSPasteboard / UIPasteboard (iOS for future)
* Linux:

    * X11 selection & clipboard atoms
    * Wayland `zwlr_data_control_manager_v1` or compositor APIs

Halcyon abstracts all platform differences behind `ClipboardBackend`.

### 3.3 Editing invariants

* I1: Clipboard text must always become valid UTF-8 before insertion.
* I2: Cut/Copy must operate on exact cluster-aligned selection ranges.
* I3: Paste must follow same pathway as typing (SPEC-052).
* I4: Clipboard cannot override IME pre-edit text; IME must be finalized/cancelled first.

---

## 4. Data Model

### 4.1 ClipboardBackend

```
pub trait ClipboardBackend {
    fn get_text(&self) -> Option<String>;
    fn set_text(&self, text: &str);
}
```

### 4.2 ClipboardManager

```
pub struct ClipboardManager {
    backend: Box<dyn ClipboardBackend>,
}
```

TextField interacts only with ClipboardManager.

---

## 5. Clipboard Operation Pipeline

### 5.1 Copy (Ctrl+C)

Steps:

1. If selection exists:

    * Extract selected UTF-8 substring
    * Write to clipboard via backend
2. If no selection:

    * Do nothing (consistent with most editors)

### 5.2 Cut (Ctrl+X)

Pipeline:

1. If selection empty → do nothing
2. Copy selected text → clipboard
3. Begin undo transaction
4. Delete selected text (cluster-aware deletion)
5. End transaction
6. Cursor moves to start of deleted region

Cut always produces an undo entry.

### 5.3 Paste (Ctrl+V)

Pipeline:

1. Cancel IME composition if active
2. Retrieve text from clipboard
3. Sanitize UTF-8
4. Normalize line breaks to `\n`
5. Begin undo transaction
6. Delete existing selection (if any)
7. Insert sanitized clipboard text
8. End undo transaction
9. Cursor moves to end of inserted text

### 5.4 Paste without selection

Cases:

* Mid-word: insert clusters normally
* At end: simple append
* At start: insert at beginning
* Multi-line paste: ParagraphEngine handles new lines

### 5.5 Password/Secure Text Fields (SPEC-057)

Clipboard ops disabled by default.
Cut/Copy return empty.
Paste allowed but never retains text.

---

## 6. UTF-8 Sanitization Rules

Clipboard text might contain invalid sequences.

Sanitization pipeline:

1. Attempt `String::from_utf8`.
2. On error:

    * Replace invalid sequences with Unicode Replacement Character (`\u{FFFD}`)
3. Strip control characters except:

    * `\n`
    * `\t`
4. Normalize CRLF → LF
5. Optionally trim non-character codepoints (U+FDD0..U+FDEF).

---

## 7. Cluster Safety Rules

Clipboard operations rely on EditingState cluster awareness (SPEC-052):

* Copy extracts exactly selection.byte_range
* Cut deletes cluster-aligned selection
* Paste inserts at cluster boundary cursor index
* Cursor moves only to cluster boundaries after paste

If clipboard text contains sequences producing multi-codepoint clusters:

* HarfBuzz cluster model defines boundaries when shaped
* EditingState stores text only; cluster mapping recomputed after layout
* Cursor after paste aligns to end of last inserted cluster

---

## 8. Integration with Undo/Redo (SPEC-055)

Clipboard operations create transactions:

| Operation | Transaction                                 |
| --------- | ------------------------------------------- |
| Copy      | No history entry                            |
| Cut       | One transaction (delete)                    |
| Paste     | One transaction (delete selection + insert) |

Undo restores text exactly before paste.
Redo reapplies the inserted clipboard text.

---

## 9. Interaction with IME (SPEC-053)

IME composition rules:

* Copy during IME: copies underlying selected text only
* Cut during IME: cut underlying text (not composition text) → cancel composition
* Paste during IME:

    * composition.cancel()
    * then paste logic
* Commit of IME text is atomic and undoable

IME never modifies clipboard.

---

## 10. Interaction with Multi-Line TextField (SPEC-054)

Copy extracts possibly multi-line selection:

```
hello
world
```

Paste inserts with preserved newlines.

Line-breaking respects ParagraphEngine constraints.

Scrolling adjusts after paste automatically.

---

## 11. Multiple TextFields / Focus Model

Clipboard operations apply to the **currently focused** TextField only:

* FocusOut disables clipboard ops
* FocusIn activates TextField as clipboard target

If no widget is focused → clipboard commands do nothing.

---

## 12. Security Considerations

* Password fields disable cut/copy automatically.
* Clipboard data must not outlive TextField unless intended (user explicitly copies).
* Avoid exposing clipboard content via logs or debug prints.
* Clipboard access must follow security restrictions on mobile/web (future).

---

## 13. Migration Plan

### Phase 0 — Implement ClipboardBackend for all platforms

* Windows: Win32 clipboard
* macOS: NSPasteboard
* Linux:

    * X11 selection
    * Wayland data control
* Halcyon wrapper unifies API

### Phase 1 — Add ClipboardManager exposed to TextField

Clipboard instance gets injected into app context.

### Phase 2 — Implement copy/cut/paste functions

Using EditingState operations, cluster-safe.

### Phase 3 — Integrate into TextField::on_key_event

* Ctrl+C
* Ctrl+X
* Ctrl+V
* Cmd+C/X/V on macOS

### Phase 4 — Testing on real platforms

IME, multi-line, UTF-8 corruption tests.

---

## 14. Testing Strategy

* Copy partial selections
* Cut multi-line block
* Paste long text
* Paste emojis, ZWJ sequences
* Unicode sanitization
* RTL clipboard interactions
* Undo/Redo around cut/paste
* IME + clipboard interactions
* Clipboard across multiple widgets
