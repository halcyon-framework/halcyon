# SPEC-HAL-269: Toast Notification System

**Author:** JC
**Status:** Proposed
**Depends on:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-041 (Unified Timing & Animation Clock)
- SPEC-HAL-097 (Motion & Micro-Animation Rules)
- SPEC-HAL-127 (Overlay Stack & Input Capture Contract)
- SPEC-HAL-147 (Animation as Authorized State Interpolation)
- SPEC-HAL-236 (Overlay Layer Composition & Hit-Test Authority)

---

## 1. Purpose

This SPEC defines the **Toast Notification System** for the Halcyon UI engine.

A Toast is a **non-modal, non-blocking, auto-dismissing notification** rendered
as a transient overlay at the bottom-center of the screen.

This SPEC specifies:
- The queue model governing multiple simultaneous notifications
- The lifecycle of individual toast entries (show → visible → dismiss)
- Dismiss interactions (timer expiry, click, action button)
- Entry and exit animation constraints
- Stacking visual model
- Overlay authority bounds
- Integration with existing engine systems

---

## 2. Scope and Non-Scope

### In Scope

- `ToastHandle` — public API for creating and observing a toast
- `ToastManager` — queue owner, lifecycle authority
- `ToastWidget` — self-rendering overlay widget
- Timer-driven auto-dismiss
- Click-to-dismiss
- Optional action button (single action per toast)
- Sonner-style visual stacking when N > 1 toasts
- Slide-up entry / fade+slide-down exit animation (visual-only)

### Out of Scope

- Persistent notification center or history
- System-level OS notifications
- Rich media content (images, progress bars)
- Multi-action toasts
- Toast categories or severity levels (future SPEC)

---

## 3. Foundational Constraints

Toasts operate under the following Constitutional constraints (SPEC-HAL-002):

**CONST-1 — No modal semantics.**
Toast MUST NOT use `NavCommand::OpenModal`, `ModalStack`, or `OverlayRoot` modal entries.
Toast does not capture input. Toast does not block interaction with underlying UI.

**CONST-2 — No scrim.**
Toast MUST NOT render a scrim or any overlay fill that occludes the base screen.

**CONST-3 — Overlay rendering rules apply.**
Toast fills behind text MUST use `TextBackgroundSpan`, not `push_rect`.
Toast MUST render at `LAYER_OVERLAY + 3` or higher (above Dialog at `LAYER_OVERLAY + 2`).

**CONST-4 — Animation is visual-only (SPEC-HAL-147).**
Slide and fade animation MUST NOT affect layout geometry, canonical rect, or hit-testing.
Hit-testing uses the canonical (fully visible) rect regardless of animation progress.
Animated geometry endpoints MUST be authorized layout states.

**CONST-5 — Timer authority (SPEC-HAL-041).**
All timing MUST use `cx.frame_time_ms()` from the engine frame loop.
`std::time::Instant` and `SystemTime` are forbidden for toast timer logic.

**CONST-6 — Deferred mutation (SPEC-HAL-002, Section 4.2).**
Toast removal from the queue MUST be deferred.
ToastManager MUST NOT remove entries during layout or paint.

---

## 4. Data Model

### 4.1 ToastId

```
ToastId: u64 — monotonically increasing, never reused within a session
```

### 4.2 ToastEntry

Each toast in the queue is represented as a `ToastEntry`:

| Field           | Type                  | Description                                         |
|-----------------|-----------------------|-----------------------------------------------------|
| `id`            | `ToastId`             | Stable identity                                     |
| `message`       | `String`              | Display text                                        |
| `duration_ms`   | `u64`                 | Auto-dismiss timer in milliseconds (0 = no timer)   |
| `action`        | `Option<ToastAction>` | Optional single action button                       |
| `show_at_ms`    | `Cell<Option<u64>>`   | Engine ms at first visible frame; None = not shown  |
| `dismiss_state` | `Cell<DismissState>`  | Lifecycle phase                                     |

### 4.3 DismissState

```
enum DismissState {
    Pending,      // In queue, not yet visible
    Visible,      // On screen, timer running
    Exiting,      // Exit animation in progress
    Dismissed,    // Fully removed from view
}
```

Transitions:
```
Pending → Visible    : first layout/paint pass
Visible → Exiting    : timer expiry OR user click OR action activated
Exiting → Dismissed  : exit animation complete
```

`Dismissed` entries are removed from the queue at the start of the next
**Deferred Mutations** phase (SPEC-HAL-002 Phase 2).

### 4.4 ToastAction

```
struct ToastAction {
    label:    String,
    callback: Box<dyn Fn(&mut AppContext)>,
}
```

Action callbacks follow the same rules as widget callbacks (SPEC-HAL-002 §3.7):
they MAY mutate `AppState` via `cx.update(...)` and MAY enqueue nav events.
They MUST NOT mutate the widget tree directly.

### 4.5 ToastHandle

`ToastHandle` is the public API surface returned to the caller on `push()`.
It is a cheaply cloneable reference to the entry.

```
ToastHandle {
    id:            ToastId,
    inner:         Rc<ToastEntry>,
}

impl ToastHandle {
    fn is_dismissed(&self) -> bool
    fn dismiss(&self)          // immediate: sets state to Exiting
}
```

---

## 5. ToastManager

`ToastManager` is the **sole owner of the toast queue**.
It lives on `AppRoot` alongside `ModalStack`.

```
struct ToastManager {
    queue:       Vec<ToastEntry>,
    next_id:     u64,
    pending_rm:  Vec<ToastId>,   // deferred removal list
}
```

### 5.1 API

```
fn push(&mut self, message, duration_ms, action) -> ToastHandle
fn dismiss(&mut self, id: ToastId)
fn dismiss_all(&mut self)
fn flush_dismissed(&mut self)   // called in Phase 2 only
```

### 5.2 Queue Invariants

**INV-TM-OWNER**
`ToastManager` is the sole owner of all `ToastEntry` instances.
No screen, widget, or external module may hold owned `ToastEntry`.

**INV-TM-LIFO-VISUAL**
Toasts are displayed newest-on-top (LIFO visual order).
Queue is logically FIFO (push appends), but rendered top-to-bottom newest-first.

**INV-TM-MAX**
The queue MUST NOT exceed **5 visible entries** at any time.
If a 6th is pushed while 5 are visible, the oldest visible entry is immediately
transitioned to `Exiting` before the new entry is appended.

**INV-TM-DEFERRED-RM**
`flush_dismissed()` MUST only be called during Phase 2 (Deferred Mutations).
It MUST NOT be called during layout, paint, or event routing.

---

## 6. Visual Model

### 6.1 Single Toast

A single visible toast renders as a **pill/bar** at bottom-center of the screen.

Layout constants (logical pixels):
```
TOAST_MARGIN_BOTTOM:   32.0    // gap from screen bottom edge
TOAST_PADDING_H:       20.0    // horizontal content padding
TOAST_PADDING_V:       12.0    // vertical content padding
TOAST_MAX_WIDTH:       480.0
TOAST_MIN_WIDTH:       200.0
TOAST_ITEM_GAP:        8.0     // vertical gap between stacked toasts
ACTION_GAP:            16.0    // gap between message and action label
```

The canonical rect is computed in `layout()`:
```
toast_w = clamp(message_width + padding + action_width, MIN, MAX)
toast_h = message_height + PADDING_V * 2
toast_x = (screen_w - toast_w) / 2
toast_y = screen_h - MARGIN_BOTTOM - toast_h
```

### 6.2 Stacking Model (Sonner-style)

When N toasts are visible simultaneously:

- **N = 1**: Standard single render.
- **N = 2**: Two toasts stacked vertically. Each occupies full visual presence.
  Gap of `TOAST_ITEM_GAP` between them.
- **N ≥ 3**: The bottom (newest) toast is at full size. Toasts above it are
  rendered at **reduced opacity and slightly smaller scale**, creating a
  perspective-stack illusion. Only the bottom 3 are painted; older toasts
  are hidden (but remain in queue, timers running).

Stacking scale/opacity for toasts above the primary:
```
position 0 (bottom/newest): scale 1.0, opacity 1.0
position 1:                  scale 0.95, opacity 0.85
position 2:                  scale 0.90, opacity 0.65
position 3+:                 not rendered (hidden)
```

Scale is applied as a visual-only paint-time transform (SPEC-HAL-147).
Canonical rects are never scaled. Hit-testing uses canonical rects.

### 6.3 Rendering Layers

```
TOAST_SHADOW_LAYER: LAYER_OVERLAY + 3
TOAST_BG_LAYER:     LAYER_OVERLAY + 4
TOAST_TEXT_LAYER:   LAYER_OVERLAY + 5
```

All background fills MUST use `TextBackgroundSpan` (z-ordering law).
Action button fill MUST use `TextBackgroundSpan`.
Border MUST use `push_border_rect`.
Shadow MUST use `push_shadow`.

---

## 7. Animation

### 7.1 Entry Animation

Entry animation: **slide up + fade in**.

Duration: 220ms (configurable via theme in future SPEC).
Easing: ease-out.

At `t = 0` (start): toast is offset `+TOAST_H` downward, opacity 0.0.
At `t = 1` (end): toast is at canonical rect, opacity 1.0.

The vertical offset is applied at paint time as a translation of draw commands.
The canonical rect is immediately at its final position for hit-testing.

**INV-ANIM-ENTRY-VISUAL-ONLY**
Entry offset MUST NOT be applied to the canonical layout rect.
Hit-testing MUST use the canonical rect from the moment `DismissState = Visible`.

### 7.2 Exit Animation

Exit animation: **slide down + fade out**.

Duration: 180ms.
Easing: ease-in.

At `t = 0`: toast at canonical rect, opacity 1.0.
At `t = 1`: toast offset `+TOAST_H` downward, opacity 0.0.

On exit animation complete: transition to `Dismissed`.

**INV-ANIM-EXIT-HIT**
During `Exiting`, the toast MUST NOT be hit-testable.
`OverlayHitRegion` MUST be removed at the start of `Exiting`, not at `Dismissed`.

### 7.3 Timer Mechanics

```
progress(t) = (frame_time_ms - start_ms) / duration_ms
clamped to [0.0, 1.0]
```

`needs_animation()` returns `true` while any toast is in `Pending`, `Visible`,
or `Exiting` state. Returns `false` when all toasts are `Dismissed` or queue is empty.

---

## 8. Input Handling

### 8.1 Click-to-Dismiss

`ToastWidget` registers a **non-capturing** hit region for each visible toast rect.

Click anywhere on a toast body (excluding the action button) → transition that
toast to `Exiting`.

This is NOT a modal hit region. Background interaction is fully unblocked.

**INV-INPUT-NO-CAPTURE**
Toast MUST NOT use pointer capture.
Toast MUST NOT register `OverlayHitRegion` with `blocks_background = true`.

### 8.2 Action Button

The action button occupies the right portion of the toast pill.
On click: invoke `ToastAction::callback`, then transition toast to `Exiting`.

Action button has its own sub-rect for hit detection.
It uses pressed visual state per SPEC-HAL-093A (hover/press tint).

### 8.3 No Keyboard Interaction

Toasts do not participate in focus traversal (SPEC-HAL-126).
Toasts do not handle keyboard events.
No Escape-to-dismiss.

---

## 9. ToastWidget

`ToastWidget` is a **self-rendering overlay widget** attached to the root widget
tree, not a modal screen. It is mounted as a child of the root by the hosting
`AppRoot` when the queue is non-empty.

`ToastWidget` holds an `Rc<ToastManager>` (shared with `AppRoot`).

### 9.1 Widget Trait Obligations

```
fn layout(&mut self, cx, constraints, _children) -> LayoutResult
    - Computes canonical rects for all visible entries
    - Stores rects internally (Cell/RefCell per entry)
    - Returns LayoutResult::fit(screen_size)

fn paint(&self, cx, rect, _children)
    - Reads frame_time_ms
    - For each visible entry: records show_at_ms if None
    - For each Exiting entry: checks animation complete → marks Dismissed
    - Applies animation offsets at paint time (visual only)
    - Emits TextBackgroundSpan, push_shadow, push_border_rect, push_text_with_backgrounds

fn paint_overlay(&self, cx)
    - Registers OverlayHitRegion for each Visible toast (not Exiting)

fn handle_event(&mut self, cx, event) -> bool
    - Pointer-down: hit-tests each toast rect
    - Hit on action sub-rect → invoke action, transition to Exiting
    - Hit on body → transition to Exiting
    - Returns true if consumed

fn needs_animation(&self) -> bool
    - true if any entry is Pending, Visible, or Exiting
```

### 9.2 Mounting

`AppRoot` mounts `ToastWidget` as a child of the root node when `ToastManager`
transitions from empty to non-empty. It unmounts it when the queue becomes empty
after `flush_dismissed()`.

This follows the same deferred mounting pattern as modal overlay widgets.

---

## 10. Public API Surface

```rust
// On AppContext / ScreenContext
fn toast(&mut self) -> ToastBuilder

struct ToastBuilder<'a> {
    fn message(self, msg: impl Into<String>) -> Self
    fn duration(self, ms: u64) -> Self          // default: 4000
    fn action(self, label, callback) -> Self
    fn push(self) -> ToastHandle
}

// Convenience
fn toast_message(msg: impl Into<String>) -> ToastHandle
    // duration = 4000, no action
```

---

## 11. Invariant Summary

| ID | Statement |
|----|-----------|
| INV-TM-OWNER | ToastManager is sole owner of ToastEntry instances |
| INV-TM-LIFO-VISUAL | Rendered newest-on-top |
| INV-TM-MAX | Max 5 visible; 6th push evicts oldest |
| INV-TM-DEFERRED-RM | flush_dismissed() in Phase 2 only |
| INV-ANIM-ENTRY-VISUAL-ONLY | Entry offset does not affect canonical rect |
| INV-ANIM-EXIT-HIT | Hit region removed at Exiting, not Dismissed |
| INV-ANIM-TIMER | All timing via cx.frame_time_ms() exclusively |
| INV-INPUT-NO-CAPTURE | No pointer capture, no background blocking |
| INV-LAYER | All toast draws at LAYER_OVERLAY + 3 or higher |
| INV-BG-FILL | All background fills via TextBackgroundSpan only |
| INV-NO-MODAL | No ModalStack, no NavCommand::OpenModal |
| INV-NO-SCRIM | No scrim or occluding overlay fill |
| INV-DEFERRED-MUT | Queue mutations only in Phase 2 |

---

## 12. Relationship to Existing SPECs

- **SPEC-HAL-127** — Toast does NOT push to `OverlayRoot`. Hit regions are
  non-capturing. Toast is outside the modal dismissal policy.
- **SPEC-HAL-147** — Animation offsets are visual-only projections of authorized
  layout states. No write-back to canonical geometry.
- **SPEC-HAL-041** — All timing via engine frame clock.
- **SPEC-HAL-236** — Toast respects overlay layer composition. Non-blocking by
  design.
- **SPEC-HAL-002** — All mutations deferred. No cross-phase leaks. Timer state
  stored in `Cell<>`, not mutated during traversal.

---

## 13. Open Questions (non-blocking)

1. **Theme tokens for animation duration** — hardcoded constants for now; theme
   integration deferred.
2. **Accessibility** — toast content should be announced via accessibility layer
   (SPEC-HAL-125). Deferred to that SPEC's scope.
3. **Swipe-to-dismiss on touch** — out of scope for this SPEC.
