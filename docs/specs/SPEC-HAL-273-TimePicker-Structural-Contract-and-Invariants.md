# SPEC-HAL-273: **TimePicker — Structural Contract and Invariants**

### *Time-of-Day Column Selection Widget*

**Author:** JC
**Status:** Implemented (Revision 4 — Final)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-136 (Transactional Selection and Explicit Commit Semantics)
* SPEC-HAL-176 (Focus & Capture Protocol)
* SPEC-PM-006 (Rendering Pre-Flight Checklist)

**Amended by:** SPEC-HAL-273A (TimePicker Direction Amendment)

---

# 1. Purpose

Define the structural contract and invariants for a time-of-day column picker widget. The TimePicker is a **standalone embeddable component** — a monolithic widget that renders hour, minute, optional second, and optional AM/PM columns directly.

The TimePicker is the visual/touch selection component. For fast keyboard-first desktop time entry, see SPEC-HAL-273A (TimeInputField — smart masked text field).

---

# 2. Value Type

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeValue {
    pub hour: u8,
    pub minute: u8,
    pub second: Option<u8>,
}
```

### Invariants

**INV-273-V1: Hour Range**
`hour` MUST be in `0..24`. Values outside this range are clamped at construction.

**INV-273-V2: Minute Range**
`minute` MUST be in `0..60`. Values outside this range are clamped at construction.

**INV-273-V3: Second Range**
If present, `second` MUST be in `0..60`. Values outside this range are clamped at construction.

**INV-273-V4: Immutable Value Semantics**
`TimeValue` is `Copy + Clone + PartialEq + Eq`. No interior mutability.

---

# 3. Display Mode

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeDisplayMode {
    #[default]
    H24,
    H12,
}
```

**INV-273-D1: Display Mode Is Construction-Time**
Display mode is set at construction via builder. It does NOT change at runtime and does NOT affect the stored `TimeValue` — the value is always 24h internally. Display mode only affects rendering.

**INV-273-D2: AM/PM Column**
When `H12` mode is active, the widget renders an AM/PM column. The indicator is derived from `hour`: AM if `hour < 12`, PM otherwise. Toggling AM/PM flips the hour by ±12.

---

# 4. Column Model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeColumn {
    Hour,
    Minute,
    Second,
    AmPm,
}
```

**INV-273-COL1: Column Presence**

| Column | Present When |
|--------|-------------|
| Hour | Always |
| Minute | Always |
| Second | `show_seconds == true` |
| AmPm | `display_mode == H12` |

**INV-273-COL2: Column Order**
Left to right: Hour → Minute → Second (if present) → AmPm (if present).

**INV-273-COL3: Column Sequence**
The widget maintains `active_columns()` — an ordered list of active columns based on configuration. All navigation and auto-advance operations use this sequence.

---

# 5. Widget Architecture

```rust
pub struct TimePicker {
    value: TimeValue,
    working: TimeValue,
    display_mode: TimeDisplayMode,
    show_seconds: bool,
    focused_column: TimeColumn,
    focused: bool,
    in_capture: bool,
    domain: TimeDomain,
    edit_policy: TimeEditPolicy,
    on_commit: Option<Box<dyn FnMut(TimeValue)>>,
    digit_buffer: Option<u8>,
}
```

### Builders

```rust
TimePicker::new(TimeValue::new(14, 30, None))
    .with_display_mode(TimeDisplayMode::H12)
    .with_seconds(true)
    .with_domain(TimeDomain::working_hours())
    .with_step_minutes(15)
    .with_edit_policy(TimeEditPolicy::Full)
    .on_commit(|v| { ... })
```

---

# 6. Capture Model (SPEC-HAL-176)

The TimePicker uses the framework's keyboard capture protocol. Two operational modes:

## Navigation Mode (focused, NOT captured)

The widget is focused and shows its value. The focused column is NOT highlighted. No editing keys are active.

**Allowed actions in Navigation mode:**
- **Enter** → enters Capture mode (`cx.request_keyboard_capture()`)
- **Click on any column** → enters Capture mode AND sets `focused_column` to clicked column
- **Tab / Shift+Tab** → propagates to focus system (moves to next/prev widget)
- **Scroll wheel** → adjusts column under cursor (scroll is a pointer action, does not require capture)

**Blocked in Navigation mode:** Arrow keys, h/j/k/l, digit entry. These do nothing.

## Editing Mode (focused + captured)

Full editing is active. The focused column is visually highlighted.

**Allowed actions in Editing mode:**
- All keyboard editing (see §7)
- **Enter** → commit + exit capture
- **Escape** → revert + exit capture
- **Tab** → flush digit buffer, exit capture, propagate Tab (do NOT commit)
- **Click on column** → set `focused_column` to clicked column (stay in capture)

### Capture Lifecycle

**INV-273-CAP1: capture_eligible**
Returns `true` (unless `edit_policy == ReadOnly`).

**INV-273-CAP2: on_capture_enter**
Sets `in_capture = true`.

**INV-273-CAP3: on_capture_leave**
Sets `in_capture = false`. Clears digit buffer. Does NOT commit (revert to value).

**INV-273-CAP4: Click Enters Capture**
`PointerEventKind::Down` on the widget calls `cx.request_keyboard_capture()` and sets `focused_column` to the clicked column.

**INV-273-CAP5: Three-State Visual**
- Unfocused: `borders.default`, 1px border
- Focused (Navigation mode): `borders.focus`, 2px border — "I have focus, press Enter to edit"
- Captured (Editing mode): `borders.focus`, 2px border + capture ring shadow (`push_shadow` with `capture_color`) + active column highlighted with `surfaces.level2`

---

# 7. Editing Keyboard (Capture Mode Only)

**INV-273-KEY1: Column Navigation**
- Left / `h` → move to previous column (wraps last → first)
- Right / `l` → move to next column (wraps first → last)
- Tab → advance without wrapping. At last column: flush buffer, exit capture, propagate Tab.
- Shift+Tab → retreat without wrapping. At first column: flush buffer, exit capture, propagate.

**INV-273-KEY2: Value Adjustment**
- Up / `k` → increment focused column by step (see §9 for step rules)
- Down / `j` → decrement focused column by step
- Wrapping or clamping depends on domain (see §9)

**INV-273-KEY3: Digit Entry — Smart Auto-Apply**
First digit behavior depends on whether it can start a valid two-digit value:

| Column | Digit | Can start two-digit? | Action |
|--------|-------|---------------------|--------|
| Hour (24h) | 0-2 | Yes (00-23) | Wait for 2nd digit |
| Hour (24h) | 3-9 | No (30+ invalid) | Apply as 03-09, auto-advance |
| Hour (12h) | 0-1 | Yes (01-12) | Wait for 2nd digit |
| Hour (12h) | 2-9 | No | Apply as 02-09, auto-advance |
| Minute/Second | 0-5 | Yes (00-59) | Wait for 2nd digit |
| Minute/Second | 6-9 | No (60+ invalid) | Apply as 06-09, auto-advance |

Two-digit completion: `buffer * 10 + digit`. If valid for column, apply and auto-advance. If INVALID (e.g., "29" for hour in 24h), flush the first digit as a single value, advance to next column, and redirect the second digit as the first digit of the new column.

AmPm column: `a`/`A` key → AM, `p`/`P` key → PM. No digit entry.

**INV-273-KEY5: Space Commits**
Space is an alternative commit key (same as Enter). Flushes digit buffer, commits working → value, exits capture.

**INV-273-KEY4: Scroll Wheel (Both Modes)**
Scroll wheel adjusts the column **under the cursor** by step. Does NOT require capture — scroll is a pointer-initiated action. The focused column does NOT change during scroll. Scroll flushes digit buffer.

---

# 8. Editing Policy

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeEditPolicy {
    #[default]
    Full,
    AdjustOnly,
    ReadOnly,
}
```

**INV-273-EDIT1: Full**
Arrows, hjkl, digits, scroll — all work. Default.

**INV-273-EDIT2: AdjustOnly**
Arrows, hjkl, scroll work. Digit entry is silently ignored. Useful for step-constrained selection (e.g., 15-minute slots where arbitrary digit values would break the grid).

**INV-273-EDIT3: ReadOnly**
No editing. `capture_eligible()` returns `false`. Widget renders its value but cannot enter Capture mode. Focus still works (accessibility). Scroll is disabled.

---

# 9. Time Domain & Bounds

```rust
#[derive(Debug, Clone, Copy)]
pub struct TimeDomain {
    pub min: TimeValue,
    pub max: TimeValue,
    pub step_minutes: u8,
    pub step_seconds: u8,
    pub wrap: bool,
}
```

**INV-273-BOUND1: Default Domain**
```rust
TimeDomain {
    min: TimeValue::new(0, 0, None),
    max: TimeValue::new(23, 59, Some(59)),
    step_minutes: 1,
    step_seconds: 1,
    wrap: true,
}
```

**INV-273-BOUND2: Step Granularity**
- Hour step is always 1
- Minute step: `step_minutes` (e.g., 15 → 00→15→30→45→00)
- Second step: `step_seconds`

**INV-273-BOUND3: Clamping**
After any adjustment, if `wrap == false`, the working value is clamped to `[min, max]`.

**INV-273-BOUND4: Wrapping**
If `wrap == true`, values wrap around within the full range. Hour wraps 23→0 and 0→23. Minute wraps `(60 - step)→0` and `0→(60 - step)`. Wrapping respects step alignment.

**INV-273-BOUND5: Digit Validation Against Domain**
Digit-entered values are validated against the domain. If the resulting time would be outside `[min, max]`, the digit is rejected.

### Convenience Constructors

```rust
impl TimeDomain {
    pub fn full() -> Self { /* 00:00:00 - 23:59:59, step 1, wrap */ }
    pub fn working_hours() -> Self { /* 09:00 - 17:00, step 15, no wrap */ }
    pub fn custom(min: TimeValue, max: TimeValue) -> Self { ... }
}
```

---

# 10. Seconds Column

**INV-273-S1: Optional**
`show_seconds: bool` (default false). Builder: `.with_seconds(true)`.

**INV-273-S2: TimeValue Extension**
When seconds are enabled, `TimeValue.second` is `Some(u8)`. When disabled, it is `None`.

**INV-273-S3: Same Rules**
Seconds column follows all digit entry, wrapping, step, and scroll-wheel rules identical to minute. Range: 0..60. Step: `step_seconds`.

**INV-273-S4: Format**
24h with seconds: `14:30:45`. 12h with seconds: `2:30:45 PM`.

**INV-273-S5: Layout**
When seconds are visible, total width adds `separator_width + column_width`.

---

# 11. Commit Invariants

**INV-273-C1: Working Value**
All adjustments modify `working` only. Committed `value` is unchanged until explicit commit.

**INV-273-C2: Enter/Space Commits + Exits Capture**
Enter or Space in Editing mode: flush digit buffer, commit `working` → `value`, fire `on_commit` callback, call `cx.release_keyboard_capture()`.

**INV-273-C3: Escape Reverts + Exits Capture**
Escape: revert `working` to `value`, clear digit buffer, call `cx.release_keyboard_capture()`.

**INV-273-C4: Tab Exits Without Commit**
Tab: flush digit buffer (apply pending single digit to working but do NOT commit), release capture, return `false` to propagate Tab. Working value is preserved but not committed.

**INV-273-C5: Blur Reverts**
If focus leaves the widget entirely (`on_blur`), revert `working` to `value`. No implicit commit on blur.

**INV-273-C6: No Auto-Commit**
No action other than Enter or Space commits. Per SPEC-HAL-002 §4.11.

---

# 12. Layout

All geometry values from theme:

```rust
pub struct TimePickerGeometry {
    pub column_width: f32,
    pub column_height: f32,
    pub ampm_width: f32,
    pub separator_width: f32,
    pub radius: f32,
    pub field_height: f32,
    pub field_padding_x: f32,
    pub popup_padding: f32,
}
```

**INV-273-L1: Total Width**
`column_width × num_value_columns + separator_width × num_separators + (ampm_width if H12)`

---

# 13. Rendering

**INV-273-R1: Pipeline Safety (SPEC-PM-006)**
Standalone mode: `push_rect` / `push_border_rect` only.
Embedded mode: `TextBackgroundSpan` + `push_text_with_backgrounds` only.

**INV-273-R1A: Three-State Border**
Standalone mode border: `if focused { (borders.focus, 2.0) } else { (borders.default, 1.0) }`. Matches DatePicker pattern.

**INV-273-R2: Column Highlight (Capture Only)**
The focused column renders a `surfaces.level2` background ONLY when `in_capture == true`.

**INV-273-R3: Capture Ring**
When `in_capture`, render a focus ring with `capture_color` (same as DatePicker's capture ring pattern using `push_shadow` with `ShadowMode::Ring`).

**INV-273-R4: Separator**
Colon (`:`) separators between value columns. Static text, not interactive.

**INV-273-R5: Text Centering**
All column text uses `compute_control_text_baseline_y`.

---

# 14. Acceptance Criteria

- `TimeValue` round-trips without loss
- 12h and 24h display modes correct
- Optional seconds column works
- **Three-state border: unfocused (default/1px), focused (focus/2px), captured (focus/2px + ring)**
- **Navigation mode: arrows/digits do nothing. Only Enter/click enters Capture.**
- **Capture mode: Left/Right/h/l navigate columns, Up/Down/j/k adjust values, digits enter**
- Click on any column enters Capture and focuses that column
- Tab from last column in Capture exits capture + propagates
- **Enter or Space commits**, Escape reverts, no auto-commit on blur
- **Smart single-digit: typing `8` for hours → immediately applies 08, auto-advances**
- **Invalid second digit redirects: typing `29` for hours → 02, advances, 9 goes to minutes**
- Scroll wheel adjusts column under cursor in both modes
- Domain bounds clamp/wrap correctly
- Step granularity respected (e.g., step=15 on minutes → 00→15→30→45)
- Edit policy: AdjustOnly blocks digits, ReadOnly blocks all editing
- Capture ring visible in Capture mode, absent in Navigation mode
- `a`/`p` keys toggle AM/PM in H12 mode during Capture

---

# 15. Non-Goals

- Date+time combined picker (see SPEC-HAL-273A §4)
- Smart masked text input field (see SPEC-HAL-273A §3, SPEC-HAL-281)
- Time slot selection from discrete availability (see SPEC-HAL-280)
- Scroll/gesture momentum for mobile (future touch SPEC)
- Timezone, duration picker

---

# 16. Removed Components

**TimePickerField** — removed. Popup-triggered field was the wrong desktop pattern. Desktop time entry should be a masked text field (SPEC-HAL-273A).
