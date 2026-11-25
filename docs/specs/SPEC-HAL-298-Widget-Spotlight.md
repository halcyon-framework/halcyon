# SPEC-HAL-298: **Widget Spotlight**

### *Temporary Focus Zoom for Dense Dashboard Layouts*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-020 (Action System)
* SPEC-HAL-030 (Presentation Policy)
* SPEC-HAL-081B/081C (Surface Stack / Overlays)

---

# 1. Purpose

This SPEC defines Widget Spotlight — a compositor-level feature that
temporarily promotes one or more widgets to a large overlay above
the dashboard, allowing detailed inspection without leaving the
layout.

Single press `f`: widget expands to ~85% of the viewport with the
dashboard dimmed behind it. Double press `f`: full screen. Multiple
widgets can be spotlighted simultaneously, splitting the spotlight
area. Escape or `f` again returns to normal.

This is a generic Halcyon feature. Any widget that opts in can be
spotlighted. The first consumers are Plot, ListView, and TreeView.

---

# 2. Problem

Dense dashboards pack 20+ widgets into small cells. Individual
widgets become too small for detailed interaction — axis labels
overlap, table rows are unreadable, tree nodes are cramped. The
user needs to temporarily expand a widget for inspection without
destroying the layout, losing context, or navigating to a separate
screen.

Existing solutions (modal dialogs, dedicated detail screens) break
context. The user loses sight of the dashboard. Spotlight keeps the
dashboard visible (dimmed) while promoting the widget of interest.

---

# 3. Architecture

## 3.1 Opt-In Model

Spotlight is disabled by default. Widgets opt in via a trait method:

```rust
/// Whether this widget supports spotlight zoom.
///
/// Default: false. Only widgets that benefit from temporary
/// expansion should return true. Buttons, text inputs, labels,
/// and other small controls should NOT opt in.
fn supports_spotlight(&self) -> bool { false }
```

The spotlight action is only available when the focused widget
returns `true`. It does not appear in the Action Palette for
widgets that return `false`.

## 3.2 Spotlight Modes

```rust
pub enum SpotlightMode {
    /// Widget occupies ~85% of viewport. Dashboard dimmed behind.
    Focus,
    /// Widget occupies 100% of viewport.
    FullScreen,
}
```

Single `f` toggles Focus mode. Double `f` (within 300ms) toggles
FullScreen mode. If already in Focus, second `f` promotes to
FullScreen. If already in FullScreen, `f` returns to normal.

Escape always exits spotlight entirely regardless of mode.

## 3.3 Component Layers

```
┌─────────────────────────────────────────────┐
│  SpotlightManager (on App)                  │
│  - Tracks which widget(s) are spotlighted   │
│  - Computes spotlight layout rects          │
│  - Manages double-tap detection             │
│  - Manages entry/exit transitions           │
├─────────────────────────────────────────────┤
│  Compositor Integration                     │
│  - Renders scrim overlay (dark, 50% alpha)  │
│  - Promotes spotlighted widgets to higher   │
│    z-layer above scrim                      │
│  - Applies temporary layout override        │
├─────────────────────────────────────────────┤
│  Action System Integration                  │
│  - ActionId::SPOTLIGHT_TOGGLE               │
│  - Bound to `f` at System layer             │
│  - Widget-scoped: only fires when focused   │
│    widget supports_spotlight() == true       │
└─────────────────────────────────────────────┘
```

## 3.4 Focus Mode Gating

Spotlight only activates in NAVIGATION mode — when the widget has
keyboard focus (blue border) but is NOT in capture mode (orange
border). Once a widget is captured (clicked into), `f` belongs to
the widget's domain (e.g., tool shortcut, find, text input).

This is enforced by the spotlight handler checking
`!widget.in_capture()` before activating. If the widget is
captured, the `f` keystroke passes through to the widget's own
input handling.

| Focus State | `f` Behavior |
|-------------|-------------|
| Navigation focus (blue) | Spotlight toggle |
| Capture focus (orange) | Passes to widget (tool shortcut, find, etc.) |
| No focus | No-op |

**INV-298-9:** Spotlight only fires in navigation mode. In capture
mode, `f` passes through to the widget's domain.

---

# 4. SpotlightManager

Lives on App alongside the NavigationStack and FocusManager. Manages
the set of spotlighted widgets and their mode.

```rust
pub struct SpotlightManager {
    /// Currently spotlighted widgets (0 = normal, 1+ = spotlight active).
    entries: Vec<SpotlightEntry>,
    /// Current mode.
    mode: SpotlightMode,
    /// Timestamp of last `f` press (for double-tap detection).
    last_f_press: Option<Instant>,
    /// Double-tap threshold.
    double_tap_ms: u128,
}

struct SpotlightEntry {
    widget_id: WidgetInternalId,
    /// Original layout rect (saved on entry, restored on exit).
    original_rect: RectF,
}

impl SpotlightManager {
    pub fn is_active(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn is_spotlighted(&self, id: WidgetInternalId) -> bool {
        self.entries.iter().any(|e| e.widget_id == id)
    }

    /// Toggle spotlight for a widget.
    ///
    /// If not spotlighted: add to spotlight set.
    /// If already spotlighted and sole entry: exit spotlight.
    /// If already spotlighted with others: remove from set.
    pub fn toggle(&mut self, id: WidgetInternalId, rect: RectF) { ... }

    /// Add a widget to the spotlight set (multi-select).
    pub fn add(&mut self, id: WidgetInternalId, rect: RectF) { ... }

    /// Exit spotlight entirely.
    pub fn exit(&mut self) { ... }

    /// Promote Focus → FullScreen.
    pub fn promote(&mut self) { ... }

    /// Compute layout rects for all spotlighted widgets.
    ///
    /// Divides the spotlight area equally among N widgets.
    /// Returns (widget_id, computed_rect) pairs.
    pub fn compute_rects(
        &self,
        viewport: RectF,
    ) -> Vec<(WidgetInternalId, RectF)> { ... }
}
```

## 4.1 Multi-Widget Layout

When N widgets are spotlighted, they share the spotlight area:

| N | Layout |
|---|--------|
| 1 | Single widget, centered |
| 2 | Side by side (horizontal split) |
| 3 | Top 1 + bottom 2, or 1×3 row |
| 4 | 2×2 grid |
| 5-6 | 2×3 grid |
| 7+ | Cap at 6. Additional widgets rejected. |

The layout adapts to viewport aspect ratio. Landscape prefers
horizontal arrangements. Portrait prefers vertical stacking.

## 4.2 Adding to Spotlight Set

The focused widget receives `f`. If Shift is held, the widget is
added to the existing spotlight set (multi-select). Without Shift,
the previous spotlight is replaced.

```
f                → replace: spotlight this widget only
Shift+f          → add: include this widget in spotlight set
f on spotlighted → remove from set (exit if last)
ff               → promote to full screen
Escape           → exit all spotlight
```

---

# 5. Layout Override

Spotlighted widgets receive a temporary layout rect that overrides
their normal grid position. The override is applied during the
layout phase — the widget's `layout()` method receives the spotlight
rect instead of its normal parent-assigned rect.

## 5.1 Constitutional Compliance

The layout override does NOT mutate the widget tree. The widget
stays in its SplitPane cell. The SpotlightManager provides an
alternative rect that the layout engine uses for spotlighted widgets.
When spotlight exits, the widget receives its normal rect again.

This is projection-only, analogous to transition interpolation
(SPEC-HAL-145). The authoritative layout data (SplitPane ratios,
grid positions) is unchanged. The spotlight rect is a non-authoritative
override applied at the last moment.

```
Normal:     SplitPane assigns rect → widget.layout(rect)
Spotlight:  SplitPane assigns rect → SpotlightManager overrides
            → widget.layout(spotlight_rect)
```

The widget doesn't know it's spotlighted. It receives a larger rect
and lays itself out accordingly. Plot renders a bigger texture.
ListView shows more rows. TreeView expands its nodes.

## 5.2 Original Rect Preservation

On spotlight entry, SpotlightManager saves the widget's current
layout rect. On exit, the saved rect is discarded and normal layout
resumes. The saved rect is NOT restored directly — the widget
returns to its parent's layout, which may have changed during
spotlight (e.g., window resize).

---

# 6. Scrim

When spotlight is active, a dark overlay renders between the
dashboard and the spotlighted widgets:

```
Z-layers (bottom to top):
  Dashboard widgets        (normal z-layer)
  Scrim overlay            (LAYER_SPOTLIGHT_SCRIM)
  Spotlighted widgets      (LAYER_SPOTLIGHT)
```

Scrim properties:
- Color: `[0.0, 0.0, 0.0, 0.5]` (50% black)
- Covers entire viewport
- Click on scrim exits spotlight (same as Escape)
- Scrim does NOT block keyboard events to the spotlighted widget

The scrim is rendered by the compositor, not by any widget. It's a
single full-viewport quad at the scrim z-layer.

---

# 7. Action System Integration

```rust
impl ActionId {
    /// Spotlight: toggle focus zoom on the focused widget.
    pub const SPOTLIGHT_TOGGLE: Self = Self(300);
}
```

Registration:

```rust
registry.register(Action {
    id: ActionId::SPOTLIGHT_TOGGLE,
    name: "Toggle Spotlight",
    description: "Expand focused widget for detailed inspection",
    category: "View",
    shortcut: Some(Shortcut::new(KeyCode::KeyF, Modifiers::default())),
    scope: ActionScope::Widget,
    handler: spotlight_toggle_handler,
});
```

The handler checks `focused_widget.supports_spotlight()`. If false,
the action is a no-op. If true, it delegates to SpotlightManager.

Double-tap detection lives in SpotlightManager, not in the action
system. The action fires on every `f` press. SpotlightManager
tracks timing and decides Focus vs FullScreen promotion.

---

# 8. Event Routing During Spotlight

Spotlighted widgets receive all keyboard and pointer events normally.
They are the focused widgets — focus doesn't change on spotlight
entry.

Pointer events on the scrim (outside spotlighted widgets) exit
spotlight. Pointer events on non-spotlighted dashboard widgets are
blocked by the scrim.

Tab navigation is constrained to the spotlighted widget set. Tab
does not escape to the dimmed dashboard.

---

# 9. Resize During Spotlight

If the window resizes while spotlight is active, SpotlightManager
recomputes spotlight rects based on the new viewport. The widget
receives its new spotlight rect in the next layout pass. No special
handling needed — the normal layout-on-resize path handles it.

---

# 10. First Consumers

Widgets that return `supports_spotlight() = true`:

| Widget | Rationale |
|--------|-----------|
| **Plot** | Dense dashboard charts need zoom for detail inspection. Primary use case. |
| **ListView** | Dense data tables benefit from more visible rows and wider columns. |
| **TreeView** | Deep hierarchies cramped in small panels. |

All other widgets default to `false`. The consumer can override on
custom widgets:

```rust
impl Widget for MyComplexPanel {
    fn supports_spotlight(&self) -> bool { true }
    // ...
}
```

---

# 11. Invariants

**INV-298-1:** Spotlight is opt-in via `supports_spotlight()`.
Default is `false`. The spotlight action is a no-op for widgets
that return `false`.

**INV-298-2:** Spotlight does not mutate the widget tree. No
reparenting, no insertion, no removal. The widget stays in its
original tree position. Layout override is projection-only.

**INV-298-3:** Spotlight layout override is non-authoritative.
The authoritative layout data (parent constraints, SplitPane ratios)
is unchanged. When spotlight exits, normal layout resumes immediately.

**INV-298-4:** Scrim click exits spotlight. Equivalent to Escape.

**INV-298-5:** Maximum 6 widgets in a spotlight set. Additional
`Shift+f` presses beyond 6 are rejected.

**INV-298-6:** Escape always exits spotlight entirely, regardless
of mode or number of widgets. No partial exit via Escape.

**INV-298-7:** SpotlightManager lives on App. No hidden global
state. Accessed through AppContext.

**INV-298-8:** Keyboard focus remains on the spotlighted widget.
Spotlight entry does not change focus. Spotlight exit does not
change focus.

**INV-298-9:** Spotlight only fires in navigation mode (navigation
focus, not capture focus). In capture mode, `f` passes through to
the widget's domain.

---

# 12. Acceptance Criteria

1. `f` on a Plot in a dashboard expands it to ~85% viewport
2. `ff` on a Plot expands to 100% viewport
3. Escape exits spotlight
4. Scrim click exits spotlight
5. `f` on a non-supporting widget does nothing
6. `Shift+f` on a second Plot adds it to spotlight set
7. Two spotlighted Plots share the spotlight area side by side
8. Four spotlighted Plots arrange in a 2×2 grid
9. Window resize during spotlight recomputes layout correctly
10. Plot renders correctly at spotlight size (texture resizes)
11. ListView shows more rows when spotlighted
12. Tab navigation stays within spotlighted widget set
13. `f` on an already-spotlighted widget exits spotlight
14. Spotlight action appears in Action Palette only for supporting widgets
15. Dashboard is dimmed but visible behind scrim
16. Spotlight works in both Focus and FullScreen modes
17. `f` in navigation mode activates spotlight
18. `f` in capture mode does NOT activate spotlight (passes to widget)

---

# 13. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. No tree mutation. Widget stays in its node. |
| B — Phase Separation | Preserved. Layout override applied during layout phase. Scrim rendered during paint. |
| C — ID-based Identity | Preserved. SpotlightManager tracks widgets by WidgetInternalId. |
| D — Pure Data | Preserved. SpotlightEntry is pure data. No callbacks, no references. |
| E — GPU-First | Preserved. Scrim is a GPU quad. Widget renders normally at larger size. |
| F — Sovereignty | Preserved. Spotlight is engine-owned. No OS window management involved. |

No permanent invariants weakened.

---

# 14. Non-Goals

- Animation/transitions on entry/exit (future polish, not MVP)
- Drag to reposition spotlighted widgets
- Pinch-to-zoom gesture for spotlight (touch is future work)
- Spotlight across multiple monitors
- Persistent spotlight state across screen transitions
- Spotlight for modal dialogs or popups
