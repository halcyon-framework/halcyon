# SPEC-HAL-281: **Input Mask Mode for TextField**

### *Pattern-Driven Formatted Text Entry as a TextField Configuration*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Amends:** SPEC-HAL-101 (TextField Widget), SPEC-HAL-054 (TextField Architecture)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-101 (TextField Widget — Single-Line Text Input)
* SPEC-HAL-054 (TextField Architecture)
* SPEC-HAL-108 (Text Editing Policy)

---

# 1. Purpose

Add an **input mask mode** to the existing TextField widget. When a mask is configured, the TextField enforces a character-level format pattern: it accepts or rejects each keystroke based on the mask, auto-inserts literal characters (separators, dashes, colons), and advances the cursor through editable positions.

This is NOT a new widget. It is a configuration mode on TextField, activated via:

```rust
TextField::new()
    .with_input_mask("(000) 000-0000")
```

Use cases: phone numbers, SSNs, IBANs, credit card numbers, postal codes, dates, times, IP addresses, license plates, serial numbers — any structured format where the pattern is fixed and the user fills in variable positions.

---

# 2. Mask Pattern Language

```
0  — Required digit (0-9)
9  — Optional digit (0-9) or space
A  — Required letter (a-z, A-Z)
a  — Optional letter or space
*  — Required alphanumeric (digit or letter)
?  — Optional alphanumeric or space
\  — Escape next character as literal
Any other character — literal (auto-inserted, not editable)
```

### Examples

| Use Case | Mask | Formatted Output |
|----------|------|-----------------|
| US Phone | `(000) 000-0000` | `(555) 123-4567` |
| SSN | `000-00-0000` | `123-45-6789` |
| Time (HH:MM) | `00:00` | `14:30` |
| Date (MM/DD/YYYY) | `00/00/0000` | `03/15/2026` |
| IP Address | `099.099.099.099` | `192.168.  1.  1` |
| IBAN (partial) | `AA00 0000 0000 0000` | `DE89 3704 0044 0532` |
| License Plate | `AAA-0000` | `ABC-1234` |
| Hex Color | `\#****** ` | `#FF8800` |

---

# 3. Mask Model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskSlot {
    RequiredDigit,
    OptionalDigit,
    RequiredLetter,
    OptionalLetter,
    RequiredAlphanumeric,
    OptionalAlphanumeric,
    Literal(char),
}

#[derive(Debug, Clone)]
pub struct InputMask {
    slots: Vec<MaskSlot>,
    placeholder_char: char,  // default '_'
}
```

### Invariants

**INV-281-M1: Mask Is Construction-Time**
The mask is set at construction via `.with_input_mask(pattern)`. It does NOT change at runtime. Changing the mask requires creating a new TextField.

**INV-281-M2: Compiled Once**
The mask pattern string is compiled into `Vec<MaskSlot>` once at construction. All runtime operations use the compiled form.

**INV-281-M3: Placeholder Display**
Unfilled editable positions show `placeholder_char` (default `_`). Literals always show. Example: mask `(000) 000-0000` with no input → `(___) ___-____`.

---

# 4. TextField Integration

### TextFieldConfig Extension

Add to `TextFieldConfig` (SPEC-HAL-101 §2):

```rust
pub struct TextFieldConfig {
    // ... existing fields ...
    pub input_mask: Option<InputMask>,
}
```

### Behavioral Changes When Mask Is Active

**INV-281-B1: Overwrite Mode**
When a mask is active, TextField operates in **overwrite mode**. Each keystroke replaces the character at the cursor position, then advances. There is no insert mode — the mask length is fixed.

**INV-281-B2: Fixed Length**
The text buffer is always exactly `mask.slots.len()` characters. It cannot grow or shrink. Unfilled positions hold `placeholder_char`.

**INV-281-B3: Character Validation**
Each keystroke is validated against the current cursor's `MaskSlot`. Accepted characters are inserted; rejected characters are silently ignored.

**INV-281-B4: Literal Skip**
After inserting a character, cursor advances. If the next position is a `Literal`, cursor automatically skips past it to the next editable position. Multiple consecutive literals are all skipped.

**INV-281-B5: No Text Selection**
Text selection is disabled in masked mode. The mask is fixed-width; selection semantics don't apply. Copy copies the formatted value. Cut is disabled.

**INV-281-B6: Backspace**
Clears the character at the current position (sets to placeholder), moves cursor left to the previous editable position (skipping literals).

**INV-281-B7: Delete**
Clears the character at the current position without moving cursor.

**INV-281-B8: Home/End**
Move to first/last editable position (skipping literals).

**INV-281-B9: Editing Substrate Bypass**
When a mask is active, the standard editing substrate (SPEC-HAL-052/105/107) cursor movement and text insertion logic is bypassed. The mask engine handles cursor positioning and character insertion directly. This is a controlled exception to SPEC-HAL-107's wiring rules — the mask is a simpler, fixed-width model that doesn't need cluster-aware editing.

---

# 5. Value Accessors

**INV-281-VA1: raw_value()**
Returns only the editable characters (no literals, no placeholders). For phone `(555) 123-4567`, raw is `5551234567`.

**INV-281-VA2: formatted_value()**
Returns the full display string including literals and placeholders.

**INV-281-VA3: is_complete()**
Returns `true` if all required positions have values. Optional positions may be empty.

**INV-281-VA4: set_raw_value(raw: &str)**
Sets editable positions from a raw string. Characters matched left-to-right against editable slots. Extra characters ignored. Missing characters leave positions empty.

---

# 6. Builder API

```rust
TextField::new()
    .with_input_mask("(000) 000-0000")      // activates mask mode
    .with_mask_placeholder('_')              // default '_'
    .with_value("5551234567")                // fills via set_raw_value
    .on_commit(|formatted| { ... })
```

---

# 7. Custom Character Classes

**INV-281-X1: Registerable Classes**
Applications can register custom character classes on the InputMask:

```rust
let mut mask = InputMask::compile("\\#HHHHHH");
mask.register_class('H', |ch| ch.is_ascii_hexdigit());

TextField::new().with_compiled_mask(mask)
```

This allows domain-specific masks without modifying the widget.

---

# 8. Rendering

**INV-281-R1: Same Pipeline as TextField**
Masked mode uses the exact same paint path as regular TextField. Background, border, focus ring, capture ring — all inherited. No rendering changes needed.

**INV-281-R2: Filled vs Placeholder Colors**
Filled positions: `typography.primary`. Placeholder positions: `typography.muted`. This is the only paint difference — the text color varies per character based on whether the position is filled.

**INV-281-R3: Cursor**
Blinking vertical bar at the current editable position. Same as standard TextField cursor.

---

# 9. Relationship to SPEC-HAL-273A (TimeInputField)

SPEC-HAL-273A defines a smart time input field that accepts flexible formats (`950`, `9am`, `0950pm`). This is MORE flexible than a strict mask — a mask enforces fixed positions, but the time parser accepts variable-length input.

TimeInputField MAY use the mask engine for display formatting (showing `__:__` placeholder), but the parsing logic is custom. The mask mode defined here is the general-purpose substrate; TimeInputField is a specialized consumer.

---

# 10. Acceptance Criteria

- `TextField::new().with_input_mask("(000) 000-0000")` works
- Typing fills editable positions, auto-skips literals
- Backspace/Delete clear positions correctly
- Left/Right/Home/End navigate editable positions (skip literals)
- `raw_value()` returns digits/letters only
- `formatted_value()` includes literals and placeholders
- `is_complete()` checks all required positions
- Placeholder char visible for unfilled positions
- No text selection in masked mode
- Custom character classes work via `register_class`
- Focus ring, capture ring, border — all inherited from TextField

---

# 11. Non-Goals

- Paste with format matching (future — paste raw value into mask)
- Dynamic mask change at runtime
- Variable-length masks
- Multi-line masked input
- Undo/redo per-character (commit-level only)
