# SPEC-HAL-280: **TimeSlotPicker — Constrained Time Selection from Available Slots**

### *Discrete Time Window Selection Widget*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-136 (Transactional Selection and Explicit Commit Semantics)
* SPEC-HAL-273 (TimePicker — TimeValue type)

---

# 1. Purpose

Define a widget for selecting a time from a discrete set of available slots. Unlike the free-form TimePicker (SPEC-273) where any time can be entered, the TimeSlotPicker constrains selection to pre-defined time windows provided by the application.

Use cases: medical appointment booking, meeting room scheduling, restaurant reservations, trading session selection, service booking — any domain where available times are finite and externally determined.

---

# 2. Value Types

```rust
/// A single available time slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeSlot {
    pub start: TimeValue,
    pub end: TimeValue,
    pub label: Option<String>,
    pub enabled: bool,
    pub metadata: Option<u64>,  // opaque app-level key
}

/// Selection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeSlotSelection {
    None,
    Single(usize),              // index into slot list
}
```

### Invariants

**INV-280-V1: Slot Ordering**
Slots MUST be provided in chronological order by `start`. The widget does NOT sort internally.

**INV-280-V2: No Overlap Enforcement**
The widget does NOT validate that slots don't overlap. The application is responsible for providing valid, non-overlapping slots. The widget renders whatever it receives.

**INV-280-V3: Enabled Gating**
Disabled slots (`enabled: false`) are visible but not selectable. They render with muted styling. Keyboard navigation skips disabled slots.

---

# 3. Widget Architecture

The TimeSlotPicker is NOT a TimePicker variant. It's a **constrained list selector** — structurally closer to a ComboBox or ListView than to the column-based TimePicker.

Two display modes:

### 3.1 Inline List Mode

Renders as a vertical scrollable list of slot rows. Each row shows start–end time range and optional label. Suitable for popup dropdowns or sidebar panels.

```
┌─────────────────────────┐
│  09:00 – 09:30          │
│ ▸09:30 – 10:00  ◄ sel  │
│  10:00 – 10:30          │
│  10:30 – 11:00  (full)  │  ← disabled
│  11:00 – 11:30          │
└─────────────────────────┘
```

### 3.2 Grid Mode

Renders slots as a wrapped grid of compact buttons. Each button shows the start time. Suitable for dense appointment grids.

```
┌──────────────────────────────────┐
│ 09:00  09:30  10:00  10:30      │
│ 11:00  11:30  12:00  12:30      │
│ 14:00  14:30  15:00  15:30      │
└──────────────────────────────────┘
```

### Struct

```rust
pub struct TimeSlotPicker {
    slots: Vec<TimeSlot>,
    selected: Option<usize>,
    working_index: Option<usize>,  // keyboard cursor
    display_mode: SlotDisplayMode,
    on_select: Option<Box<dyn FnMut(usize, &TimeSlot)>>,
    focused: bool,
    in_capture: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotDisplayMode {
    #[default]
    List,
    Grid,
}
```

---

# 4. Data Source

**INV-280-DS1: External Slots**
Slots are provided by the application via `set_slots(Vec<TimeSlot>)` or builder `.with_slots(...)`. The widget does not generate slots. The application queries its backend/API for available times and feeds them in.

**INV-280-DS2: Dynamic Update**
Slots can be replaced at any time via `set_slots()`. Selection is cleared if the selected index is out of range after update.

**INV-280-DS3: Jump to First Available**
`.jump_to_first_available()` scrolls to and highlights the first enabled slot. Useful for initial display ("show me the soonest opening").

**INV-280-DS4: Jump to Nearest**
`.jump_to_nearest(time: TimeValue)` scrolls to the slot closest to the given time. Useful when combining with DatePicker ("I picked March 20, show me the nearest slot to my preferred time").

---

# 5. Interaction Invariants

**INV-280-I1: Capture Model**
Follows the same Navigation/Editing capture protocol as TimePicker (SPEC-273 §6). Navigation mode: Tab/Shift+Tab moves between widgets. Enter or click enters Capture. Editing mode: keyboard navigation active.

**INV-280-I2: Keyboard (Capture Mode)**
- Up / `k` → move cursor to previous enabled slot
- Down / `j` → move cursor to next enabled slot
- Enter / Space → select slot at cursor, commit, exit capture
- Escape → revert selection, exit capture
- Home → jump to first enabled slot
- End → jump to last enabled slot
- Page Up/Down → jump by visible page height

**INV-280-I3: Click**
Click on an enabled slot → select it immediately (commit). Click on a disabled slot → no action.

**INV-280-I4: Scroll Wheel**
Scroll moves the viewport, does not change selection.

**INV-280-I5: Disabled Skip**
Up/Down skips disabled slots. If all slots in the direction are disabled, cursor stays.

---

# 6. Commit Semantics

**INV-280-C1: Click Commits Immediately**
Unlike TimePicker where Enter is required, clicking a slot IS the commit action. Explicit enough for a discrete list selection.

**INV-280-C2: Keyboard Commit**
Enter/Space on the cursor position commits. Escape reverts.

**INV-280-C3: Callback**
`on_select(index: usize, slot: &TimeSlot)` fires on commit.

---

# 7. Composition with DatePicker

The TimeSlotPicker is designed to compose with DatePicker:

```rust
// Application flow:
// 1. User picks a date in DatePicker
// 2. App queries API for available slots on that date
// 3. App calls slot_picker.set_slots(available_slots)
// 4. User picks a slot
// 5. App has (date, time_slot) → books appointment
```

The two widgets are independent. The application wires them together. No coupling at the widget level.

---

# 8. Layout

**INV-280-L1: List Mode**
Fixed-width rows. Row height from theme. Scrollable if slots exceed visible height. Width determined by parent layout or explicit `.with_width()`.

**INV-280-L2: Grid Mode**
FlowGrid-style wrapping. Button width from theme. Rows wrap to available width. Scrollable vertically.

---

# 9. Rendering

**INV-280-R1: Pipeline Safety**
`push_rect` only for backgrounds and selection highlight. Text via `push_text_with_backgrounds`. Per SPEC-PM-006.

**INV-280-R2: Selected Slot**
`accents.primary` background with white text.

**INV-280-R3: Cursor Slot (keyboard)**
`surfaces.level2` background (same as TimePicker column highlight).

**INV-280-R4: Disabled Slot**
Muted text, no hover effect, no click response.

---

# 10. Acceptance Criteria

- Renders list of time slots from external data
- Click selects and commits immediately
- Keyboard navigation skips disabled slots
- `jump_to_first_available()` works
- `jump_to_nearest(time)` works
- Combines with DatePicker at the application level (no widget coupling)
- Capture model matches TimePicker convention
- Disabled slots are visible but not selectable

---

# 11. Non-Goals

- Generating slots (application responsibility)
- Validating slot overlap (application responsibility)
- Multi-select (single slot only for MVP)
- Duration display customization (shows start–end as formatted text)
- Recurring slot patterns (application generates the instances)
