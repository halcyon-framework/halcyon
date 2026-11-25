# SPEC-HAL-226 Amendment A: **Tooltip Wrapper — Consumer API Contract**

### *Hover Delay, Trigger Binding, and Auto-Placement*

**Author:** JC
**Status:** Draft Amendment to SPEC-HAL-226
**Amendment Type:** B (Extending) per SPEC-HAL-002A
**Depends on:**

* SPEC-HAL-226 (Tooltip, Hint, and Help Surfaces)
* SPEC-HAL-002 (Constitution & Architectural Invariants)

---

# 1. Purpose

SPEC-HAL-226 defines the low-level `TooltipSurface` — a non-interactive overlay primitive with explicit show/hide semantics. It deliberately defers hover delay, trigger binding, and auto-placement as non-goals.

This amendment fills that gap by specifying the **consumer-facing API contract** that sits above `TooltipSurface`. It defines how widgets bind tooltip content, when tooltips appear and disappear, and where they are placed.

This amendment does NOT weaken any SPEC-226 invariant. `TooltipSurface` remains informational-only, non-interactive, timer-free, and externally driven. The amendment adds a new layer above it.

---

# 2. Sections Added to SPEC-HAL-226

The following sections are appended after §5 (Acceptance Criteria).

---

## §6. Tooltip Host — Trigger Binding

### INV-226-H1: Context API Binding

Any widget can declare tooltip content via the paint context:

```rust
cx.tooltip(TooltipContent::text("Description text"));
```

This registers the widget's bounds as a tooltip trigger region. The framework manages the lifecycle — the widget does NOT manage timers, visibility, or placement.

### INV-226-H2: TooltipHost Wrapper (Alternative)

For cases where `cx.tooltip()` is insufficient (e.g., conditional content, dynamic text), a `TooltipHost` wrapper widget can be used:

```rust
TooltipHost::new(child_widget)
    .content(TooltipContent::text("Description"))
    .placement(TooltipPlacement::Below)
```

`TooltipHost` is a zero-size layout wrapper — it does not affect the child's layout or paint.

### INV-226-H3: Content Types

```rust
pub enum TooltipContent {
    Text(String),
    RichText { title: String, body: String },
    Custom(Box<dyn Widget>),
}
```

`Custom` allows arbitrary widget content inside the tooltip surface. The tooltip surface remains non-interactive regardless of content type — pointer events pass through.

---

## §7. Hover Delay — Timing Policy

### INV-226-T1: Show Delay

The tooltip appears after a configurable hover delay. The default delay is read from the theme:

```rust
pub struct TooltipTimingSpec {
    pub show_delay_ms: u32,   // Default: 500ms
    pub hide_delay_ms: u32,   // Default: 0ms (instant hide)
    pub reshow_delay_ms: u32, // Default: 100ms (quick reshow after recent dismiss)
}
```

### INV-226-T2: Timer Ownership

The hover timer is owned by the **framework tooltip manager**, NOT by the widget and NOT by the `TooltipSurface`. This preserves INV-226-B1 (no timers inside tooltip) — the timer is external.

### INV-226-T3: Timer Reset on Leave

The show timer resets when the pointer leaves the trigger region. If the pointer re-enters before the timer fires, the timer restarts.

### INV-226-T4: Instant Dismiss

The tooltip hides immediately when:
- The pointer leaves the trigger region (after `hide_delay_ms`, default 0 = instant).
- The trigger widget loses focus.
- A pointer-down event occurs anywhere.
- The Escape key is pressed.

### INV-226-T5: No Show on Click/Tap

Pointer-down on the trigger region does NOT show the tooltip. Tooltips are hover-only. Touch devices use long-press if tooltips are needed (future extension).

### INV-226-T6: Reshow Shortcut

If a tooltip was recently dismissed (within `reshow_delay_ms`) and the pointer enters a new trigger region, the tooltip for the new region shows immediately (no delay). This provides smooth tooltip browsing across adjacent controls.

---

## §8. Auto-Placement Policy

### INV-226-P1: Placement Preference

Each tooltip trigger may specify a preferred placement:

```rust
pub enum TooltipPlacement {
    Above,
    Below,
    Left,
    Right,
    Auto,
}
```

Default: `Auto`.

### INV-226-P2: Auto-Placement Algorithm

When `Auto` (or when the preferred placement would clip the viewport):

1. Try Below (most common, least disruptive).
2. Try Above.
3. Try Right.
4. Try Left.
5. Fall back to Below with viewport clamping.

The tooltip is clamped to the viewport in all cases — it never renders off-screen.

### INV-226-P3: Anchor Point

The tooltip anchors to the trigger widget's bounds rect:
- `Below`: centered horizontally on trigger, top edge at trigger bottom + gap.
- `Above`: centered horizontally on trigger, bottom edge at trigger top - gap.
- `Left`: centered vertically on trigger, right edge at trigger left - gap.
- `Right`: centered vertically on trigger, left edge at trigger right + gap.

Gap is a theme constant (`tooltip_gap: f32`, default 4px).

### INV-226-P4: No Reflow**

Tooltip placement is computed once at show time. If the trigger moves (scroll, resize), the tooltip position does NOT update — it stays at the original anchor. If the trigger scrolls out of view, the tooltip is dismissed.

---

## §9. Framework Tooltip Manager

### INV-226-M1: Singleton Manager

A single `TooltipManager` lives in the App, not in individual widgets. It tracks:
- Current hover target (widget bounds + content).
- Show timer state.
- Currently visible tooltip (if any).
- Last dismiss timestamp (for reshow shortcut).

### INV-226-M2: Phase Compliance

The tooltip manager operates during the **Deferred Mutations** phase (Phase 2). Timer checks happen during event intake. Tooltip show/hide is applied as a deferred mutation. This preserves Constitutional phase separation.

### INV-226-M3: Single Tooltip

At most one tooltip is visible at any time. Showing a new tooltip dismisses the current one instantly.

---

## §10. Updated Acceptance Criteria

In addition to SPEC-226 §5:

- `cx.tooltip()` registers trigger regions without widget-level timer code.
- Show delay defaults to 500ms, configurable in theme.
- Tooltip hides instantly on pointer-leave, focus-loss, pointer-down, or Escape.
- Auto-placement avoids viewport clipping.
- Reshow shortcut eliminates delay when browsing adjacent controls.
- No timer exists inside `TooltipSurface` (INV-226-B1 preserved).
- Tooltip manager lives in App, not in widgets.

---

## §11. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. TooltipManager is App-owned. |
| B — Phase Separation | Preserved. Timer in intake, mutations in Phase 2. |
| C — ID-based Identity | Preserved. Trigger regions identified by widget bounds, not references. |
| D — Pure Data | Preserved. TooltipContent is pure data. |
| E — GPU-First | Preserved. TooltipSurface renders via overlay pass. |
| F — Sovereignty | Preserved. No OS tooltip APIs used. |

No permanent invariants weakened. This amendment extends SPEC-226 with a consumer API layer.
