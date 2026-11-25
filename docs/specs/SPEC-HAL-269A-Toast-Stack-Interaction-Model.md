# SPEC-HAL-269A: Toast Stack Interaction Model

**Author:** JC
**Status:** Proposed
**Amends:** SPEC-HAL-269 (Toast Notification System)
**Amendment Type:** B — Extending (adds new invariants, new state)
**Depends on:**
- SPEC-HAL-269 (base toast system)
- SPEC-HAL-147 (Animation as Authorized State Interpolation)
- SPEC-HAL-097 (Motion & Micro-Animation Rules)

---

## 1. Purpose

SPEC-HAL-269 defines the toast queue, lifecycle, and flat stacking visual model.
This amendment replaces Section 6.2 (Stacking Model) with a **collapsed/expanded
state machine** and defines the full interaction contract for the toast stack.

---

## 2. What Is Being Replaced

SPEC-HAL-269 §6.2 is superseded in its entirety by this document.

The flat opacity/scale table (position 0/1/2 static values) is removed.
The `needs_animation` / hover machinery described below replaces it.

All other sections of SPEC-HAL-269 remain in force.

---

## 3. Stack State Machine

The toast widget operates in one of four stack states:

```
enum StackState {
    Collapsed,    // Default. One toast visible, others peek as depth cues.
    Expanding,    // Animating collapsed → expanded.
    Expanded,     // All toasts fully visible. Hover active.
    Collapsing,   // Animating expanded → collapsed.
}
```

Transitions:

```
Collapsed  → Expanding  : pointer enters stack bounding rect
Expanding  → Expanded   : expansion animation complete
Expanded   → Collapsing : pointer leaves stack bounding rect
Collapsing → Collapsed  : collapse animation complete

(any state) → Collapsed : queue drops to 0 or 1 active entries
```

**INV-SS-AUTHORITY**
`StackState` is owned exclusively by `ToastWidget`.
No external component reads or writes it.

**INV-SS-SINGLE-ENTRY**
If the active queue has exactly 1 entry, `StackState` is always `Collapsed`.
Transitions to `Expanding` only occur when N ≥ 2 active entries.

---

## 4. Collapsed Visual Model

In `Collapsed` (and during `Collapsing`):

- **Position 0** (newest / bottom): rendered at full size, full opacity.
  This is the primary visible toast.
- **Position 1** (2nd newest): rendered behind position 0, shifted up by
  `STACK_PEEK_OFFSET` (8px), scaled to `STACK_SCALE_1` (0.95),
  opacity `STACK_OPACITY_1` (0.7). Clipped to a thin strip above position 0.
- **Position 2** (3rd newest): rendered behind position 1, shifted up by
  `STACK_PEEK_OFFSET * 2`, scaled to `STACK_SCALE_2` (0.90),
  opacity `STACK_OPACITY_2` (0.45). Clipped to a thin strip.
- **Position 3+**: not rendered.

A **count badge** is rendered on position 0 if N ≥ 3, showing `+N` where N is
the number of toasts NOT rendered (hidden). Badge position: top-right corner of
position 0 toast, inside padding. Badge is painted at `TOAST_TEXT_LAYER`.

The stack bounding rect in `Collapsed` is:
```
stack_rect = union of position 0 rect + peek strip rects above it
```
This rect is used for hover detection.

**INV-COLLAPSED-HIT**
In `Collapsed`, only position 0 registers a click-to-dismiss hit region.
Position 1 and 2 peek strips are visual only — they do not consume pointer events.

---

## 5. Expanded Visual Model

In `Expanded` (and during `Expanding`):

- All active entries are rendered at **full size**, full opacity, stacked
  vertically with `TOAST_ITEM_GAP` between them.
- Newest toast at bottom (position 0). Older toasts above.
- No scale reduction. No opacity reduction.
- Each toast shows a **dismiss button** (×) at its right edge, inside padding.
  Dismiss button sub-rect: `DISMISS_BTN_SIZE × DISMISS_BTN_SIZE` (20×20px),
  right-aligned, vertically centered within the toast.
- A **"Clear all"** label/button appears above the topmost toast, right-aligned
  with the toast column. Gap of `CLEAR_ALL_GAP` (6px) above topmost toast.
  Clear all triggers `dismiss_all()` on the manager.

**INV-EXPANDED-HIT**
In `Expanded`, each toast body (excluding dismiss button) is a click-to-dismiss
region. The dismiss button is its own sub-hit-region.
Clear all is its own hit region above the stack.

---

## 6. Transition Animation

### 6.1 Collapsed → Expanded (Expanding)

Duration: `EXPAND_MS` = 260ms. Easing: ease-out.

Each toast animates from its collapsed visual position to its final expanded
position. At `t = 0`, toasts are at collapsed geometry. At `t = 1`, at expanded
geometry.

Position 1 and 2 animate from their peek-strip positions to their full expanded
rects. Position 3+ animate from off-screen (from below position 0) to their
expanded positions.

Dismiss buttons and "Clear all" fade in during the last 40% of the expansion
(`t > 0.6 → opacity = (t - 0.6) / 0.4`).

### 6.2 Expanded → Collapsed (Collapsing)

Duration: `COLLAPSE_MS` = 200ms. Easing: ease-in.

Reverse of expansion. Dismiss buttons and "Clear all" fade out during first 40%.

### 6.3 Gap Close (in Expanded mode)

When a toast is dismissed while `StackState == Expanded`:

- The dismissed entry runs its exit animation (slide-down + fade, `ANIM_EXIT_MS`).
- Simultaneously, all entries above the dismissed entry animate their canonical
  positions downward to close the gap.
- Gap close duration: `GAP_CLOSE_MS` = 200ms. Easing: ease-out.

**INV-GAP-CLOSE-VISUAL**
Gap close offset is applied at paint time only.
Canonical rects are updated immediately to their new positions in the same layout
pass where the dismissed entry is removed from the paint set.
Hit-testing uses updated canonical rects from the start of gap close, not the
animated positions.

### 6.4 Animation Authority

All transition animation is visual-only per SPEC-HAL-147:
- Canonical rects always reflect final layout state.
- Hit-testing uses canonical rects.
- Animation offset applied at paint time only.
- No write-back to layout state.

---

## 7. Pointer Tracking

`ToastWidget` receives pointer events to drive stack state transitions.

On `PointerMove` with position inside `stack_rect`:
- If `StackState == Collapsed` and N ≥ 2 → transition to `Expanding`.

On `PointerMove` with position outside `stack_rect` (or `PointerLeave`):
- If `StackState == Expanded` or `Expanding` → transition to `Collapsing`.

**INV-HOVER-NO-CAPTURE**
Toast stack MUST NOT use pointer capture to track hover.
Hover is determined from `PointerMove` events in the normal (non-capturing)
routing pass.

**INV-HOVER-STACK-RECT**
`stack_rect` must be recomputed every layout pass.
In `Collapsed`: union of visible peek strips + position 0 rect.
In `Expanded`: union of all visible toast rects + "Clear all" rect.
In transition: use the expanded-target rect (prevents flicker on fast hover).

---

## 8. New Constants

```
STACK_PEEK_OFFSET:  f32 = 8.0    // px shift per level in collapsed stack
STACK_SCALE_1:      f32 = 0.95
STACK_SCALE_2:      f32 = 0.90
STACK_OPACITY_1:    f32 = 0.70
STACK_OPACITY_2:    f32 = 0.45
EXPAND_MS:          f32 = 260.0
COLLAPSE_MS:        f32 = 200.0
GAP_CLOSE_MS:       f32 = 200.0
DISMISS_BTN_SIZE:   f32 = 20.0
CLEAR_ALL_GAP:      f32 = 6.0
```

---

## 9. Updated Invariant Table (additions to SPEC-HAL-269 §13)

| ID | Statement |
|----|-----------|
| INV-SS-AUTHORITY | StackState owned exclusively by ToastWidget |
| INV-SS-SINGLE-ENTRY | StackState is always Collapsed when N ≤ 1 |
| INV-COLLAPSED-HIT | Only position 0 receives click events in Collapsed |
| INV-EXPANDED-HIT | Each toast body and × button are independent hit regions in Expanded |
| INV-GAP-CLOSE-VISUAL | Gap close is visual-only; canonical rects update immediately |
| INV-HOVER-NO-CAPTURE | No pointer capture for hover tracking |
| INV-HOVER-STACK-RECT | stack_rect recomputed every layout pass |

---

## 10. Relationship to SPEC-HAL-269

All invariants in SPEC-HAL-269 remain in force. This amendment:
- Replaces §6.2 only.
- Adds `StackState` to the data model (`ToastWidget` internal state).
- Adds `stack_rect: Cell<RectF>`, `stack_state: Cell<StackState>`,
  `expand_start_ms: Cell<Option<u64>>`, `gap_close_state: RefCell<Vec<GapCloseEntry>>`
  to `ToastWidget`.
- Extends `handle_event` to process hover/pointer tracking.
- Extends `layout` to compute `stack_rect`.
- Extends `paint_overlay` to register expanded hit regions.
