# SPEC-HAL-238: **DatePicker — Structural Contract and Invariants**

### *Full Rewrite — March 10, 2026*

**Author:** JC
**Status:** Binding
**Supersedes:** SPEC-HAL-238 (Date and Time Picker Primitive Invariants — Draft)
**Layer:** Widget Primitives / Value Controls
**Scope:** DatePicker only. TimePicker and DateTimePicker are deferred to separate SPECs.

**Depends on:**
- SPEC-HAL-002 (Constitution & Architectural Invariants) — §3.11 No Interpretive Intent Resolution; §4.11 Explicit Commit Authority
- SPEC-HAL-001 (Core Architecture)
- SPEC-HAL-199 (Popup, ComboBox, and Anchored Selection Controls) — ComboBox pattern reference
- SPEC-HAL-207 (Application-Layer Value Binding & UI Boundary Law)
- SPEC-HAL-136 (Transactional Selection and Explicit Commit Semantics)
- SPEC-HAL-093A (Interactive Element States & Micro-Feedback)
- SPEC-HAL-096B (Geometry Tokenization and Theme Authority)
- SPEC-HAL-236 (Overlay Layer Composition and Hit-Test Authority)

---

# 1. Purpose

This SPEC defines the **full structural contract** for the DatePicker widget in Halcyon.

It specifies:

- The canonical date model — `DateValue`, validation rules, ordering
- The widget state machine — calendar view state, selection state, keyboard cursor
- The interaction model — pointer, keyboard, constraint enforcement
- The commit model — what constitutes a selection commit, and what does not
- Inline and popup rendering modes — layer authority, anchor model, dismissal
- Phase compliance — which state is mutated in which phase
- Value binding — FormBinder integration, emission timing
- Visual state contract — per SPEC-HAL-093A cell state definitions
- All invariants in enumerated, enforceable form

**TimePicker and DateTimePicker are explicitly out of scope.**

---

# 2. Scope Boundaries

## 2.1 In Scope

- Gregorian calendar date selection (year, month, day)
- Inline mode: calendar embedded directly in parent layout
- Popup mode: calendar as an overlay anchored to a trigger widget, using the SPEC-HAL-236 overlay mechanism
- Min/max date constraints (disabled days)
- Month and year navigation within the calendar view
- Full keyboard navigation within the calendar grid
- FormBinder value binding via text emission
- Predicate-based date annotation and disable policy (DatePolicy)
- Static date marks with Dot and Tint visual kinds (DateMark)
- Embedded mode for use inside popup field composite (DatePickerField)
- YYYYMM keyboard jump navigation
- Vim-style navigation keys in Capture mode

## 2.2 Explicitly Out of Scope

- Time selection (hours, minutes, seconds) — separate SPEC
- Combined date+time picker — separate SPEC
- Date range selection — separate SPEC
- Week picker, fiscal calendars, non-Gregorian systems
- Time zone display or conversion
- Locale-aware day/month formatting — the widget uses raw ordinals; format display is a caller responsibility
- Natural language parsing

---

# 3. Model Layer

## 3.1 `DateValue`

`DateValue` is the canonical, timezone-neutral date representation.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateValue {
    pub year: i32,   // Full year, e.g. 2026
    pub month: u8,   // 1–12
    pub day: u8,     // 1–N where N is the valid day count for that month/year
}
```

### INV-238-DV-1: Validity at Construction

A `DateValue` must represent a valid Gregorian calendar date. Invalid states (month 0, month 13, day 0, day 32, February 30, etc.) must not be constructible through the public API. The constructor validates and returns `Option<DateValue>` or `Result`.

### INV-238-DV-2: Immutability

`DateValue` is a value type. All operations on it return new values. No field-level mutation is exposed.

### INV-238-DV-3: Chronological Ordering

`DateValue` derives `Ord`. `a < b` means `a` is chronologically earlier than `b`. This ordering governs all constraint evaluation.

### INV-238-DV-4: Leap Year Correctness

Day counts per month must account for leap years. Construction validation must use the correct Gregorian leap year rule (divisible by 4, except centuries not divisible by 400).

## 3.2 `DatePickerState`

Owns all mutable runtime state for a DatePicker instance. Lives inside the widget, not in `AppState`.

```rust
pub struct DatePickerState {
    /// The committed selected date. None until a selection is made.
    selected: Option<DateValue>,
    /// The calendar view position. Independent of selected.
    view_year: i32,
    view_month: u8,
    /// Keyboard navigation cursor. Distinct from selected.
    /// Moves freely during keyboard navigation. None when widget is not keyboard-focused.
    kbd_cursor: Option<DateValue>,
    /// Min selectable date. Days before this are disabled.
    min_date: Option<DateValue>,
    /// Max selectable date. Days after this are disabled.
    max_date: Option<DateValue>,
    /// Dirty flag: true when selected changed since last take_changed().
    dirty: bool,
}
```

### INV-238-STATE-1: View/Selection Independence

`view_year`/`view_month` and `selected` are independent. Navigating months does not alter `selected`. Committing a selection does not alter the view unless the caller explicitly synchronizes them.

### INV-238-STATE-2: Cursor/Selection Independence

`kbd_cursor` is a navigation position — it is not `selected`. Moving the keyboard cursor does not set the dirty flag. The dirty flag is set only when `selected` changes value via an explicit commit.

### INV-238-STATE-3: Widget Ownership

`DatePickerState` is owned exclusively by the widget struct. It is not shared, not wrapped in `Rc`, not accessible from outside the widget implementation.

### INV-238-STATE-4: Partial State Validity

`selected = None` is a fully valid state. The calendar renders with no day highlighted. The dirty flag is false when no selection has ever been committed.

## 3.3 Operations

```rust
impl DatePickerState {
    pub fn new(initial: Option<DateValue>, min: Option<DateValue>, max: Option<DateValue>) -> Self;

    // View navigation — does not affect selected or dirty
    pub fn prev_month(&mut self);
    pub fn next_month(&mut self);
    pub fn prev_year(&mut self);
    pub fn next_year(&mut self);

    // Keyboard cursor — does not affect selected or dirty
    pub fn move_kbd_cursor(&mut self, delta_days: i32);

    // Commit — sets selected, sets dirty
    pub fn commit_kbd_cursor(&mut self);   // commits kbd_cursor if Some and enabled
    pub fn commit_day(&mut self, day: u8); // commits pointer-selected day in view month

    // Constraint query
    pub fn is_enabled(&self, date: DateValue) -> bool;
    pub fn is_dirty(&self) -> bool;
    pub fn take_changed(&mut self) -> Option<Option<DateValue>>;

    // Accessors
    pub fn selected(&self) -> Option<DateValue>;
    pub fn view_year(&self) -> i32;
    pub fn view_month(&self) -> u8;
    pub fn kbd_cursor(&self) -> Option<DateValue>;
}
```

### INV-238-STATE-5: Dirty Flag Semantics

`dirty` is set to `true` only when `commit_day()` or `commit_kbd_cursor()` changes the value of `selected`. Navigation, cursor movement, and popup open/close do not set dirty. `take_changed()` clears dirty and returns the current `selected`. Subsequent calls in the same frame return `None`.

### INV-238-STATE-6: Commit of Disabled Date

`commit_day()` and `commit_kbd_cursor()` are no-ops if the target date fails `is_enabled()`. The dirty flag is not set. No error is emitted.

---

# 4. Calendar Grid Structure

## 4.1 Fixed Grid

The calendar grid is always 7 columns × 6 rows:

- **Row 0:** Day-of-week headers (column labels — e.g. "Su", "Mo", …)
- **Rows 1–6:** Day cells. Fixed 42 cells per view.

Day cells that fall outside the current month are rendered in an out-of-month visual state. They are never selectable.

### INV-238-GRID-1: Fixed Dimensions

The calendar grid is always 7 × 6. Month length does not alter the grid shape. Short months pad with out-of-month cells.

### INV-238-GRID-2: First-Day-of-Week

First-day-of-week (Sunday-first or Monday-first) is configured at construction and is immutable for the lifetime of the widget instance. It is never derived from locale automatically.

### INV-238-GRID-3: Geometry Authority

The widget owns all calendar geometry: cell size, cell padding, header height, navigation arrow positions, total preferred size. No external geometry authority exists for these values. All sizing uses SPEC-HAL-096B geometry tokens.

### INV-238-GRID-4: Preferred Size Stability

The widget's preferred size is constant for a given theme and font size. It does not vary by month, by selected date, or by constraint configuration.

---

# 5. Interaction Model

## 5.1 Pointer Interaction

- **Click on enabled day cell** → `commit_day(day)` → dirty flag set → FormBinder emission on next `flush_pending_actions`
- **Click on prev/next month arrow** → `prev_month()` / `next_month()` → no dirty, no emission
- **Click on prev/next year arrow** → `prev_year()` / `next_year()` → no dirty, no emission
- **Click on disabled day cell** → no-op
- **Click on out-of-month cell** → no-op
- **Click outside calendar (popup mode only)** → popup dismisses, `selected` unchanged, dirty unchanged

### INV-238-INT-1: No AppState Writes in Callbacks

Widget callbacks — pointer handlers, keyboard handlers — mutate `DatePickerState` only. They never mutate `AppState` directly. Value transfer to application state occurs via FormBinder in `flush_pending_actions`.

### INV-238-INT-2: Navigation Is Not Commit

Month/year navigation via arrow click or Page Up/Down does not constitute a commit and does not set the dirty flag.

## 5.2 Keyboard Navigation

### INV-238-KBD-1: Keyboard Scope

Keyboard navigation is fully scoped to the DatePicker widget. Arrow keys do not propagate to parent focus handlers while the DatePicker holds focus.

### INV-238-KBD-2: Cursor Independence

Arrow key navigation moves `kbd_cursor` only. It does not move `selected`. This is the Transactional Selection pattern per SPEC-HAL-136 §B Principle 1: "Selection can be a working set, not an action."

### INV-238-KBD-3: Commit Is Explicit

Selection commits only on Enter or Space. Per SPEC-HAL-002 §4.11 and SPEC-HAL-136 §F, commits must be explicit and user-driven. No implicit commit occurs on focus loss, blur, or popup dismissal.

| Key | Action |
|---|---|
| Arrow Left | `kbd_cursor` −1 day |
| Arrow Right | `kbd_cursor` +1 day |
| Arrow Up | `kbd_cursor` −7 days |
| Arrow Down | `kbd_cursor` +7 days |
| Page Up | `prev_month()` — cursor clamps to last valid day if needed |
| Page Down | `next_month()` — cursor clamps to last valid day if needed |
| Enter / Space | `commit_kbd_cursor()` if cursor is on an enabled date |
| Escape | Popup mode: dismiss without commit. Inline mode: release focus. |

### INV-238-KBD-4: Cursor Clamping on Navigation

When Page Up/Down moves the view month and the cursor's day would be invalid in the new month (e.g. cursor on day 31, new month has 30 days), the cursor clamps to the last valid day of the new month.

### INV-238-KBD-5: Constraint-Aware Cursor

The keyboard cursor never lands on a disabled date. If an arrow key move would place it on a disabled date, the move snaps to the nearest enabled date in the movement direction. If no enabled date exists in that direction within the view, the move is a no-op.

## 5.3 Constraint Enforcement

### INV-238-CON-1: Disabled Day Unselectable

Days outside the `[min_date, max_date]` range are rendered disabled and are not selectable by pointer or keyboard.

### INV-238-CON-2: Constraints Do Not Clamp View

Min/max constraints restrict which days are selectable, not which months are navigable. The user may freely navigate to months where all days are disabled. The calendar renders them all as disabled.

### INV-238-CON-3: No Interpretive Defaults

The widget must not silently select the nearest valid date when the user interacts with a disabled day. The interaction is a no-op. Per SPEC-HAL-002 §3.11, the widget must not infer intent.

## 5.4 Date Policy

`DatePolicy` is a predicate-based annotation and disable system. It supplements `min_date`/`max_date` with arbitrary rule logic.

```rust
pub struct DatePolicy {
    /// Returns a visual highlight for the given date, or None.
    pub highlight: Option<Box<dyn Fn(DateValue) -> Option<PolicyHighlight> + Send + Sync>>,
    /// Returns true if the given date should be treated as disabled.
    /// OR'd with min_date/max_date constraints.
    pub disabled: Option<Box<dyn Fn(DateValue) -> bool + Send + Sync>>,
}

pub enum PolicyHighlight {
    Dot,            // Small dot below day number
    Tint,           // Subtle accent tint behind cell
    Custom(Color),  // Caller-specified background tint
}
```

### INV-238-POLICY-1: Policy Is Additive

`DatePolicy.disabled` is OR'd with the existing `min_date`/`max_date` constraint. A date disabled by policy is as disabled as one outside the date range.

### INV-238-POLICY-2: Policy Closures Are Pure

Policy closures must not mutate external state. They are called per-cell during paint. They receive a `DateValue` and return a value type. They must not capture mutable references.

### INV-238-POLICY-3: Policy Does Not Override Commit

A date disabled by `DatePolicy.disabled` cannot be committed, consistent with INV-238-STATE-6.

## 5.5 Date Marks

`DateMark` is a static annotation for specific dates. Unlike `DatePolicy`, marks are a `Vec` provided at construction — suitable for known event dates (holidays, deadlines, etc.).

```rust
pub struct DateMark {
    pub date: DateValue,
    pub kind: DateMarkKind,
}

pub enum DateMarkKind {
    Dot,   // Small dot below day number
    Tint,  // Subtle background tint
}
```

Marks from `DateMark` and highlights from `DatePolicy` are merged additively. A date may have both.

## 5.6 Extended Keyboard Navigation (Capture Mode)

In Capture mode (active after `i`, `Enter`, or pointer click when focused), the following additional keys apply:

| Key / Text | Action |
|---|---|
| `i` | Open popup (field) or enter Capture mode (inline) |
| `h` | `kbd_cursor` −1 day (vim left) |
| `j` | `kbd_cursor` +7 days (vim down) |
| `k` | `kbd_cursor` −7 days (vim up) |
| `l` | `kbd_cursor` +1 day (vim right) |
| `t` | Jump cursor to today |
| `g` + 4–6 digits | YYYYMM jump — type `g` then 4-digit year (optional 2-digit month) to navigate view |
| Ctrl+H / Ctrl+← | `prev_month()` |
| Ctrl+L / Ctrl+→ | `next_month()` |
| `0`–`9` (one or two digits, within 600ms) | Jump cursor to day number in current month |

### INV-238-KBD-6: Vim Keys Are Text Events in Capture Mode

Vim navigation keys (`h`, `j`, `k`, `l`, `t`, `g`) are processed as `Event::Text` events, not `Event::Key` events. They are only active in Capture mode. They do not conflict with arrow key navigation.

### INV-238-KBD-7: YYYYMM Jump Is Progressive

The `g` key initiates a year-jump buffer. Subsequent digit characters fill the buffer. After 4 digits the year is captured; after 6 digits (YYYYMM) the view jumps immediately. Enter commits a partial 4-digit buffer (year only). Any non-digit key cancels the buffer without navigating.

### INV-238-KBD-8: Digit Navigation Uses Two-Digit Window

Single-digit input sets the cursor to that day (if valid) and starts a 600ms window. A second digit within that window combines into a two-digit day number. If the combined number is valid, it is committed as the cursor position and the buffer is cleared.

---

# 6. Commit Model

The DatePicker commit model is governed by SPEC-HAL-002 §4.11 (Explicit Commit Authority) and SPEC-HAL-136 (Transactional Selection).

### INV-238-COMMIT-1: Two Explicit Commit Sources

Exactly two events constitute a date selection commit:

1. **Pointer activation on an enabled day cell** — click (pointer down + up on same cell)
2. **Keyboard activation on an enabled cursor date** — Enter or Space while `kbd_cursor` is Some and enabled

No other event constitutes a commit.

### INV-238-COMMIT-2: No Implicit Commit

The following events must never constitute a commit:

- Focus gain or loss
- Popup open or close
- Month/year navigation
- Keyboard cursor movement (arrow keys)
- Blur or escape (popup dismissal)
- Programmatic view changes

### INV-238-COMMIT-3: Commit Is Atomic

A commit atomically updates `selected` and sets `dirty`. There is no intermediate state where `dirty` is true but `selected` has not yet been updated.

### INV-238-COMMIT-4: Commit Idempotency

If the user commits the same date that is already `selected`, `dirty` is not set. The value has not changed.

---

# 7. Rendering

## 7.1 Inline Mode

The calendar widget is a standard retained-mode widget placed in the widget tree by the parent screen. It participates in normal layout flow.

### INV-238-INLINE-1: Fixed Preferred Size

Inline DatePicker reports a fixed preferred size from `layout()`. It does not fill or stretch. A parent may constrain it, but the widget clips rather than reflows its internal grid.

### INV-238-INLINE-2: No Layout Mutation

The calendar does not shift or reflow surrounding layout. It is a leaf-sized widget occupying a fixed rect.

## 7.2 Popup Mode — DatePickerField

Popup mode is implemented by `DatePickerField`, a composite widget that owns an embedded `DatePickerWidget` and manages a trigger surface + anchored popup overlay.

The popup mechanism uses `paint_overlay` + `OverlayHitRegion` per SPEC-HAL-236 (Overlay Layer Composition and Hit-Test Authority), not the SPEC-HAL-199 Popup primitive.

### INV-238-POPUP-1: SPEC-236 Overlay Mechanism

`DatePickerField` registers two `OverlayHitRegion` entries in `paint_overlay`:
1. A full-window backdrop (registered first, hit-tested last) — catches outside clicks for dismissal.
2. The popup calendar rect (registered second, hit-tested first) — routes pointer and move events to the field.

### INV-238-POPUP-2: Anchor Sources

The popup anchor is the trigger widget's layout rect. Default placement is below the trigger. If screen overflow exists below and space exists above, placement flips to above. Auto placement is viewport-aware via the `PopupPlacement::Auto` variant, which requires `with_window_height()` to be set.

```rust
pub enum PopupPlacement { Below, Above, Auto }
```

### INV-238-POPUP-3: Dismissal Without Commit

Outside-click dismissal and Escape dismissal close the popup without modifying `selected` or setting `dirty`. Per INV-238-COMMIT-2, popup close is not a commit.

### INV-238-POPUP-4: Not a Modal Screen

Popup mode does not use `nav_open_modal` or any navigation stack push. The calendar popup is an overlay widget, not a screen.

### INV-238-POPUP-5: Focus and Capture

`DatePickerField` calls `cx.request_keyboard_capture()` when the popup opens. Keyboard events are forwarded to the embedded `DatePickerWidget` while the popup is open. `on_capture_leave` closes the popup.

### INV-238-POPUP-6: Popup Open State Is Widget-Internal

`popup_open` lives in `DatePickerField`. It is not in `DatePickerState` or `AppState`. The calling screen does not observe popup visibility. The screen observes only committed values via FormBinder.

### INV-238-POPUP-7: Embedded Mode

The owned `DatePickerWidget` instance is created with `.with_embedded(true)`. In embedded mode:
- The calendar background gradient is suppressed (the field provides it via background blocker).
- All cell fills and text renders use `TextBackgroundSpan` at `LAYER_OVERLAY + 2` (text pass), above the background blocker at `LAYER_OVERLAY + 1`.
- The border and focus ring are suppressed; the popup border is rendered by `DatePickerField::paint_overlay`.

## 7.3 Paint Pipeline Compliance

### INV-238-PAINT-1: Background Fill via TextBackgroundSpan

All filled regions — calendar background, day cell highlights, header area, selected day fill — use `push_text_with_backgrounds` + `TextBackgroundSpan`. Direct `push_rect` at overlay z-levels occludes underlying text. This is required by INV-BG-FILL.

### INV-238-PAINT-2: Paint Purity

`paint()` and `paint_overlay()` are pure. No `DatePickerState` mutation occurs in these methods. Animation `Cell` updates are permitted per SPEC-HAL-146.

### INV-238-PAINT-3: Theme Compliance

No hardcoded color literals. All colors sourced from `cx.theme.colors.*` semantic tokens per SPEC-HAL-096B.

| Element | Token |
|---|---|
| Calendar background | `surfaces.level2` |
| Day cell (normal) | `surfaces.level2` bg, `typography.primary` text |
| Day cell (hovered) | `list.row_hover` bg |
| Day cell (selected) | `accents.primary` bg, `typography.inverse` text |
| Day cell (today, unselected) | Normal bg + `borders.focus` outline ring |
| Day cell (today, selected) | Selected treatment only |
| Day cell (disabled) | `typography.muted` text; no hover response |
| Day cell (out-of-month) | `typography.muted` at reduced opacity; transparent bg |
| Keyboard cursor (not selected) | `accents.secondary` bg or `borders.focus` outline |
| Navigation arrows (idle) | `typography.secondary` |
| Navigation arrows (hovered) | `typography.primary` |
| Month/year header text | `typography.primary`, label weight |

### INV-238-PAINT-4: Gradient on Calendar Background

The outer calendar background uses the gradient pipeline (`paint_vertical_gradient`) for visual depth. Flat solid fills are not used on the outer surface. Individual day cell backgrounds use flat fills.

### INV-238-PAINT-5: Geometry Computed in Layout, Read in Paint

Grid cell rects are computed in `layout()` and cached into a fixed-size array on the widget. `paint()` reads these cached rects. No geometry recomputation occurs in `paint()`.

## 7.4 Cell Visual State Priority

When multiple states apply to a single cell, priority order is (highest first):

1. Selected
2. Keyboard cursor (if not selected)
3. Hovered (if not cursor or selected)
4. Today ring (overlay drawn on top of any of the above)
5. Disabled (suppresses hover, blocks interaction)
6. Out-of-month (transparent, non-interactive; never overlaps valid days)

---

# 8. Phase Compliance

| Phase | DatePicker behavior |
|---|---|
| Phase 2 — Deferred Mutations | FormBinder value transfer applied |
| Phase 3 — Rebuild | Widget rebuilt from model state snapshot |
| Phase 4 — Layout | Grid cell rects computed, cached. Preferred size returned. |
| Phase 5 — Scene Build | Calendar painted from cached rects; no state mutation |
| Phase 6 — GPU Render | No tree access |

### INV-238-PHASE-1: No Mutation in Layout or Paint

`commit_day()`, `prev_month()`, `next_month()`, `move_kbd_cursor()` — all state-mutating operations — are called only from `handle_event()`. They are never called from `layout()` or `paint()`.

### INV-238-PHASE-2: Popup Open/Close Is Immediate

Popup open and close are immediate widget state mutations in `handle_event`. They are not deferred to Phase 2. `popup_open` is toggled directly in pointer and keyboard handlers.

---

# 9. Value Binding Contract

## 9.1 FormBinder Integration

DatePicker emits its committed value via a `ChangeWriter` text binding in ISO 8601 format: `"YYYY-MM-DD"`. A cleared selection emits `""` (empty string).

The calling screen consumes the value in `flush_pending_actions`:

```rust
fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
    self.form.bind(cx, |b| {
        b.text("appointment_date").sync(&mut self.model.date_str);
    });
    // Caller parses model.date_str if non-empty.
}
```

### INV-238-BIND-1: Emit Exactly Once Per Commit

The widget emits a value change exactly once per selection commit. It does not emit on navigation, cursor movement, or popup open/close.

### INV-238-BIND-2: External Consumption

The calling screen owns value extraction. The widget does not write into `AppState` or any screen model field directly.

### INV-238-BIND-3: Unidirectional Binding

FormBinder binding is widget → application. The application may initialize `selected` via `DatePickerState::new(Some(date), ...)` at widget construction. After construction, the application does not push values back into the widget via FormBinder. This follows SPEC-HAL-207 §3 (Widgets Emit Values, Not Effects).

### INV-238-BIND-4: FormBinder Not Required

FormBinder is the canonical pattern, but not mandatory. A calling screen may poll `state.take_changed()` directly in `flush_pending_actions` if it manages widget state explicitly. Both paths are legal.

---

# 10. Invariant Index

| ID | Statement |
|---|---|
| INV-238-DV-1 | DateValue is valid at construction or not constructible |
| INV-238-DV-2 | DateValue is a value type; no in-place mutation |
| INV-238-DV-3 | DateValue ordering is chronological |
| INV-238-DV-4 | Leap year computation is Gregorian-correct |
| INV-238-STATE-1 | View navigation and selection are independent state |
| INV-238-STATE-2 | Keyboard cursor and selection are independent state |
| INV-238-STATE-3 | DatePickerState is widget-owned exclusively |
| INV-238-STATE-4 | selected=None is valid and complete |
| INV-238-STATE-5 | Dirty flag set only by commit; cleared only by take_changed() |
| INV-238-STATE-6 | Commit of disabled date is a no-op |
| INV-238-GRID-1 | Calendar grid is always 7 × 6 |
| INV-238-GRID-2 | First-day-of-week is set at construction, immutable |
| INV-238-GRID-3 | Widget owns all calendar geometry |
| INV-238-GRID-4 | Preferred size is constant for a given theme/font |
| INV-238-INT-1 | Callbacks mutate DatePickerState only; never AppState directly |
| INV-238-INT-2 | Navigation is not a commit |
| INV-238-KBD-1 | Keyboard navigation is scoped to the widget |
| INV-238-KBD-2 | Arrow key movement moves kbd_cursor, not selected |
| INV-238-KBD-3 | Commit requires Enter or Space |
| INV-238-KBD-4 | Cursor clamps to valid day on month navigation |
| INV-238-KBD-5 | Cursor never lands on a disabled date |
| INV-238-CON-1 | Disabled days are unselectable by pointer and keyboard |
| INV-238-CON-2 | Constraints restrict selectability, not view navigation |
| INV-238-CON-3 | No silent nearest-valid substitution on disabled interaction |
| INV-238-COMMIT-1 | Exactly two explicit commit sources exist |
| INV-238-COMMIT-2 | Focus loss, popup close, navigation, cursor move are not commits |
| INV-238-COMMIT-3 | Commit atomically updates selected and dirty |
| INV-238-COMMIT-4 | Committing the already-selected date does not set dirty |
| INV-238-INLINE-1 | Inline mode has fixed preferred size |
| INV-238-INLINE-2 | Inline mode does not shift surrounding layout |
| INV-238-POPUP-1 | Popup mode uses SPEC-HAL-236 overlay mechanism |
| INV-238-POPUP-2 | Popup anchors to trigger widget rect; flips on screen overflow |
| INV-238-POPUP-3 | Outside-click and Escape dismiss without commit |
| INV-238-POPUP-4 | Popup mode is not a nav_open_modal screen |
| INV-238-POPUP-5 | Keyboard capture forwards events to embedded picker; on_capture_leave closes popup |
| INV-238-POPUP-6 | Popup open state is widget-internal; not in AppState |
| INV-238-POPUP-7 | Embedded picker mode suppresses gradient, border, focus ring |
| INV-238-PAINT-1 | All fills use TextBackgroundSpan (INV-BG-FILL compliance) |
| INV-238-PAINT-2 | paint() and paint_overlay() are pure |
| INV-238-PAINT-3 | No hardcoded colors; all from theme tokens |
| INV-238-PAINT-4 | Calendar background uses gradient pipeline |
| INV-238-PAINT-5 | Cell rects computed in layout(), read in paint() |
| INV-238-PHASE-1 | No state mutation in layout() or paint() |
| INV-238-PHASE-2 | Popup open/close is immediate in handle_event; not a deferred mutation |
| INV-238-BIND-1 | Value emitted exactly once per commit |
| INV-238-BIND-2 | Application consumes value in flush_pending_actions |
| INV-238-BIND-3 | Binding is widget → application only |
| INV-238-BIND-4 | FormBinder is canonical but not required |
| INV-238-POLICY-1 | DatePolicy.disabled is OR'd with min/max constraints |
| INV-238-POLICY-2 | Policy closures are pure; no external state mutation |
| INV-238-POLICY-3 | Policy-disabled dates cannot be committed |
| INV-238-KBD-6 | Vim keys are Text events in Capture mode only |
| INV-238-KBD-7 | YYYYMM jump is progressive; Enter commits partial year |
| INV-238-KBD-8 | Digit navigation uses two-digit 600ms window |

---

# 11. Explicit Prohibitions

The following are permanently forbidden. No SPEC-238 amendment may lift these without a constitutional amendment to SPEC-HAL-002.

- Widget-side date normalization or "nearest valid date" substitution on commit (SPEC-HAL-002 §3.11)
- Implicit commit on blur, focus loss, popup dismiss, or navigation (SPEC-HAL-002 §4.11)
- Direct write to `AppState` from widget event handlers (SPEC-HAL-002 §3.7)
- Locale-automatic formatting inside the widget rendering layer
- Hardcoded color values in `paint()` (INV-238-PAINT-3)
- Geometry recomputation inside `paint()` (INV-238-PAINT-5)
- Popup open/close mutating `selected` (INV-238-POPUP-3)
- OS-level modal popup behavior (INV-238-POPUP-4)
- Sharing `DatePickerState` across widget instances (INV-238-STATE-3)
- Popup open/close treated as a deferred Phase-2 mutation — popup state is immediate widget state

---

# 12. Files

| File | Role |
|---|---|
| `src/model/date_value.rs` | `DateValue` — value type, validation, ordering |
| `src/widgets/date_picker.rs` | `DatePickerWidget` (inline + embedded) + `DatePickerState` + `DatePolicy` + `DateMark` |
| `src/widgets/date_picker_field.rs` | `DatePickerField` — trigger + anchored popup overlay |
| `src/widgets/mod.rs` | `pub mod date_picker`, `pub mod date_picker_field` |
| `examples/date_picker_demo.rs` | Reference demo — inline and popup (field) modes |

---

# 13. Out of Scope (Deferred)

- **TimePicker** — separate SPEC
- **DateTimePicker** — separate SPEC
- **Date range selection** — separate SPEC
- **Week picker** — separate SPEC
- **Locale-aware day/month name formatting** — caller responsibility; format adapter system to be defined separately if needed
