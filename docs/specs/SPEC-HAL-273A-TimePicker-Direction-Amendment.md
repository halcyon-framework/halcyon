# SPEC-HAL-273A: **TimePicker Direction Amendment**

### *Smart Time Input, Scroll Picker, and DateTimePicker Composition*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Amends:** SPEC-HAL-273 (TimePicker)
**Amendment Type:** B (Extending)

---

# 1. Purpose

Amend SPEC-HAL-273 to define the full time input strategy for Halcyon. The original SPEC defined a column-based picker widget. This amendment clarifies that the column picker is a **secondary, touch-optimized** component, and defines two additional components:

1. **TimeInputField** — a smart masked text field for fast keyboard entry (primary desktop interaction)
2. **DateTimePicker** — a combined widget composing DatePicker + TimePicker/TimeInputField

---

# 2. Component Roles

| Component | Platform | Primary Input | Status |
|-----------|----------|--------------|--------|
| **TimePicker** (SPEC-273) | Mobile/Touch | Scroll columns, gesture snap | Implemented (standalone) |
| **TimeInputField** (this amendment) | Desktop | Keyboard — smart parsing | Not yet implemented |
| **DateTimePicker** (this amendment) | Both | Composes Date + Time | Not yet implemented |

---

# 3. TimeInputField — Smart Masked Input

### 3.1 Core Behavior

A single-line text field that accepts flexible time input and parses it into a `TimeValue` on commit. The field shows formatted time (e.g., `14:30` or `2:30 PM`) when not editing. On focus, the user types freely and the field parses on commit.

### 3.2 Accepted Input Formats

All of the following must parse correctly:

| Input | Parsed As |
|-------|-----------|
| `0950` | 09:50 |
| `950` | 09:50 |
| `1430` | 14:30 |
| `09:50` | 09:50 |
| `9:50` | 09:50 |
| `09:50am` | 09:50 |
| `09:50pm` | 21:50 |
| `0950pm` | 21:50 |
| `950am` | 09:50 |
| `09am` | 09:00 |
| `9am` | 09:00 |
| `9pm` | 21:00 |
| `12pm` | 12:00 |
| `12am` | 00:00 |
| `0` | 00:00 |
| `12` | 12:00 |
| `23` | 23:00 |

### INV-273A-P1: Parser Rules

1. Strip whitespace and normalize to lowercase
2. Check for trailing `am`/`pm`/`a`/`p` suffix → extract and remove
3. Remaining string is digits, optionally with one `:`
4. If `:` present: split into hour and minute parts
5. If no `:`: interpret by digit count:
    - 1 digit: hour (e.g., `9` → 09:00)
    - 2 digits: hour (e.g., `14` → 14:00, `09` → 09:00)
    - 3 digits: first digit = hour, last two = minutes (e.g., `950` → 9:50)
    - 4 digits: first two = hour, last two = minutes (e.g., `0950` → 09:50)
6. Apply AM/PM modifier if present (add 12 for PM when hour < 12, set to 0 for 12AM)
7. Validate: hour 0..23, minute 0..59. Reject invalid.

### INV-273A-P2: Commit Behavior

- Commit on Enter or blur (standard TextField commit)
- If input is invalid: revert to previous value, show error flash
- If input is valid: update value, format display

### INV-273A-P3: Display Format

When not focused/editing, the field shows the formatted value:
- 24h mode: `14:30`
- 12h mode: `2:30 PM`

When focused, the field shows the raw text the user is typing.

### 3.3 Implementation Notes

TimeInputField wraps (or extends) the existing `TextField` widget. The parsing logic is a pure function `parse_time_input(input: &str) -> Option<TimeValue>`. The field stores the working text while editing and the committed `TimeValue`.

---

# 4. DateTimePicker — Combined Widget

### 4.1 Concept

A single field showing `2025-03-15 14:30` (or locale-appropriate format). Opens a popup containing:
- Left panel: DatePicker calendar (existing widget)
- Right panel: TimeInputField (or TimePicker for touch)

Both values commit together as a combined `DateTimeValue`.

### 4.2 Value Type

```rust
pub struct DateTimeValue {
    pub date: DateValue,
    pub time: TimeValue,
}
```

### 4.3 Deferred

DateTimePicker is a composition task — it needs DatePicker + TimeInputField working independently first. It's explicitly deferred until both are solid.

---

# 5. TimePicker Amendments

### 5.1 Scroll/Gesture Support (Future)

The existing TimePicker column widget should eventually support:
- Smooth scroll with momentum and snap-to-value
- Touch gesture drag (not just click-based Up/Down)
- Haptic-style visual feedback (values scroll through with size/opacity gradients)

This is a touch/mobile interaction SPEC, not a widget structure change. The widget architecture supports it — the scroll data is just `working.hour += delta` clamped and wrapped.

### 5.2 TimePickerField Removed

The popup-triggered `TimePickerField` is removed (wrong desktop pattern). The standalone `TimePicker` remains for embedding and future mobile use.

---

# 6. Priority Order

1. Remove `TimePickerField` (cleanup task — immediate)
2. Implement `TimeInputField` with smart parser (next time-related work)
3. Add scroll/gesture to `TimePicker` (when touch support is in scope)
4. Implement `DateTimePicker` composition (after both above are solid)

---

# 7. Acceptance Criteria (TimeInputField)

- All input formats from §3.2 parse correctly
- Invalid input reverts to previous value
- 12h and 24h display modes produce correct formatted output
- Field integrates with form binding system (`BindingKey` / `ChangeWriter`)
- Keyboard-only workflow: Tab into field → type `1430` → Enter → field shows `14:30`
