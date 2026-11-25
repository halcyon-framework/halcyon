# SPEC-HAL-268: Proximity Scroll (Anchor-Displacement Cursor Scrolling)

**Status:** Active
**Supersedes:** Original edge-proximity design (v1)
**Layer:** Widget / Interaction
**Applies To:** ScrollArea
**Origin:** User request — convenience scrolling via cursor position

---

## 1. Concept

Hold a modifier combo while the cursor is inside a ScrollArea. The container scrolls automatically based on cursor proximity to container edges. Closer to an edge → faster scroll in that direction. Center = dead zone, no scroll. Release the combo → scrolling stops immediately.

No clicking, no wheel. Pure positional input. Smooth, predictable, no sudden jumps.

---

## 2. Interaction Model

### Activation

- **Modifier combo**: To be determined. Candidates:
  - `Option + Cmd` (macOS) / `Alt + Ctrl` (Windows) — unlikely to collide with system shortcuts
  - `Middle mouse button hold` — natural but not available on all input devices
  - Configurable via theme/preferences

- **Entry**: Combo held + cursor enters ScrollArea bounds
- **Exit**: Combo released OR cursor leaves ScrollArea bounds
- **Cancellation**: Any PointerDown cancels proximity scroll mode

### Dead Zone

The container interior is divided into zones:

```
┌─────────────────────────────┐
│  SCROLL UP (fast at edge)   │
│  ┌───────────────────────┐  │
│  │ SCROLL UP (slow)      │  │
│  │ ┌───────────────────┐ │  │
│  │ │                   │ │  │
│←─│ │    DEAD ZONE      │ │──→│
│  │ │    (no scroll)    │ │  │
│  │ │                   │ │  │
│  │ └───────────────────┘ │  │
│  │ SCROLL DOWN (slow)    │  │
│  └───────────────────────┘  │
│  SCROLL DOWN (fast at edge) │
└─────────────────────────────┘
```

Same pattern horizontally if horizontal scroll is enabled.

### Speed Curve

For each axis:

1. Compute normalized distance from center: `d = (cursor_pos - center) / (half_extent)`  → range [-1.0, 1.0]
2. Apply dead zone: if `|d| < dead_zone_fraction` → speed = 0
3. Outside dead zone: remap `|d|` from `[dead_zone, 1.0]` → `[0.0, 1.0]`
4. Apply easing curve (quadratic or cubic) for natural acceleration
5. Multiply by `max_scroll_speed` (pixels per second, configurable)
6. Direction: sign of `d`

```
speed(d) = sign(d) * ease((|d| - dead_zone) / (1.0 - dead_zone)) * max_speed
         where ease(t) = t² (quadratic) or t³ (cubic)
```

### Frame-Driven Application

Speed is in **pixels per second**, not pixels per event. Each frame:

```
scroll_delta = speed * frame_dt
scroll_area.scroll_by(delta)
```

This ensures consistent behavior regardless of frame rate. High refresh = same speed, smoother motion. Low frame rate = same speed, larger steps.

### Clamping

- Scroll is clamped to `[0, max_scroll]` as usual
- When scroll reaches the limit, speed naturally drops to zero (no overscroll)
- No bounce, no inertia after release

---

## 3. Suggested Constants

| Parameter | Value | Notes |
|-----------|-------|-------|
| `dead_zone_fraction` | 0.35 | 35% of half-extent from center |
| `max_scroll_speed` | 800.0 px/s | At container edge |
| `ease_power` | 2.0 | Quadratic ramp |
| `activation_delay` | 100ms | Ignore brief modifier taps |

All configurable via `ScrollAreaConfig` or theme.

---

## 4. Implementation Outline

### ScrollArea State

```rust
struct ProximityScrollState {
    active: bool,
    cursor_pos: Point,          // Last known cursor position (local coords)
    activation_time: Instant,   // When combo was first detected
}
```

### Event Handling

In `handle_event`, on `PointerEventKind::Move`:

```
if combo_held(pe.modifiers) && !self.proximity.active {
    if elapsed since first detection > activation_delay {
        self.proximity.active = true;
        // Request animation frame to drive scrolling
    }
}
if combo_held(pe.modifiers) && self.proximity.active {
    self.proximity.cursor_pos = pe.local_pos;
}
if !combo_held(pe.modifiers) && self.proximity.active {
    self.proximity.active = false;
}
```

### Frame Update

In `update_visual()` (or equivalent frame-driven hook):

```
if self.proximity.active {
    let speed = compute_proximity_speed(self.proximity.cursor_pos, container_size);
    let delta = speed * dt;
    self.state.scroll_by(delta);
    self.smooth.set_target_offset(...);
    // Request next frame
}
```

### Modifier Detection

The PointerEvent already carries `modifiers`. No new input infrastructure needed. The specific combo is checked as:

```rust
fn is_proximity_combo(modifiers: Modifiers) -> bool {
    // macOS: Option+Cmd, Windows: Alt+Ctrl
    modifiers.alt && (modifiers.meta || modifiers.ctrl)
}
```

Platform-specific combo via `cfg(target_os)` or theme configuration.

---

## 5. Visual Feedback (Optional)

While proximity scroll is active:

- Cursor could change to a scroll cursor (cross-arrows or similar)
- Subtle directional indicators at container edges showing scroll direction
- Speed indicator opacity scales with scroll speed

These are nice-to-have. The feature works without any visual feedback — the scrolling itself is the feedback.

---

## 6. Invariants

**INV-PROX-SCROLL-1**: Proximity scroll MUST be frame-driven (pixels/second × dt), never event-driven.

**INV-PROX-SCROLL-2**: Proximity scroll MUST NOT interfere with normal scroll input. If the user scrolls the wheel while proximity mode is active, both inputs compose additively.

**INV-PROX-SCROLL-3**: Proximity scroll MUST stop immediately on modifier release. No inertia, no coast.

**INV-PROX-SCROLL-4**: Proximity scroll MUST NOT activate during pointer capture (scrollbar drag, column resize, text selection). Check `cx.has_pointer_capture()`.

**INV-PROX-SCROLL-5**: Dead zone fraction and max speed MUST be configurable per ScrollArea instance or via theme.

---

## 7. Open Questions

1. **Key combo choice**: What feels natural and doesn't collide with existing shortcuts? Need to survey both macOS and Windows modifier landscapes.

2. **Horizontal-only / vertical-only containers**: If ScrollArea only has V-scroll enabled, should horizontal cursor position be ignored entirely, or should it still work if the content overflows horizontally?

3. **Nested ScrollAreas**: If a ScrollArea is inside another ScrollArea, does proximity scroll apply to the innermost one under the cursor? (Yes — same as normal scroll routing.)

4. **Touchpad**: Should proximity scroll work with touchpad modifier detection? Touchpad users already have two-finger scroll, so this may be mouse-only.

---

## 8. Priority

Low. This is a convenience feature, not a bug fix. Implement after scroll routing and SHIFT+scroll are working correctly.
