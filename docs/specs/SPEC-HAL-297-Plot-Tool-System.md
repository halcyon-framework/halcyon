# SPEC-HAL-297: **Plot Tool System**

### *Extensible Gesture Interpretation and Toolbar Overlay for Plot Widgets*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-296 (Plot — Coordinate-Authority Rendering System)

---

# 1. Purpose

This SPEC defines the tool system for Halcyon's Plot widget — the
mechanism by which pointer gestures are interpreted within a chart.

The active tool determines what a click, drag, or Shift+drag means.
The default Pointer tool provides the current behavior (pan, zoom,
select). Alternative tools provide measurement, drawing, annotation
placement, and consumer-defined interactions.

The toolbar is an overlay widget for switching tools. The consumer
registers built-in and custom tools. Keyboard shortcuts provide
direct tool activation.

---

# 2. Problem

Shift+drag is overloaded: range selection, measurement, drawing —
different users expect different behavior from the same gesture.
Hardcoding modifier keys to specific functions is brittle and
unextensible. Professional charting tools use a tool palette where
the active tool defines what gestures mean.

SPEC-HAL-296 §7 defines an `InteractionMode` enum (Navigate,
SelectSingle, SelectBox, Annotate) as a placeholder. This SPEC
replaces that placeholder with a formal, trait-based, consumer-
extensible tool system.

---

# 3. Architecture

Three layers:

1. **PlotTool trait** — defines gesture interpretation for one mode
2. **PlotToolbar** — overlay widget for switching between tools
3. **Built-in tools** — Pointer, Measure, drawing primitives

## 3.1 Gesture Routing

The active tool sits between raw pointer events and Plot's default
gesture handler. It receives events first. If the tool handles the
gesture, Plot's default behavior (pan/zoom/select) is suppressed.
If the tool passes through, the default handler runs.

```
PointerEvent
    ↓
active_tool.handle_gesture(event, transform, modifiers)
    ↓
GestureResult::Handled          →  Event consumed, no default handling
GestureResult::Output(output)   →  Event consumed, output queued for consumer
GestureResult::Passthrough      →  Plot default handler (pan/zoom/select)
```

Scroll events always pass through to zoom regardless of active tool.
The tool system governs click and drag only.

## 3.2 Separation of Concerns

```
┌──────────────────────────────────────────────┐
│  Consumer                                     │
│  - Registers tools (built-in + custom)        │
│  - Receives ToolOutput                        │
│  - Owns annotation storage and rendering      │
│  - Decides what "place order" or "draw line"  │
│    actually means                             │
├──────────────────────────────────────────────┤
│  Plot (Widget)                                │
│  - Routes gestures through active tool        │
│  - Renders tool overlay vertices              │
│  - Hosts PlotToolbar as child widget          │
│  - Queues ToolOutput for consumer polling     │
├──────────────────────────────────────────────┤
│  PlotTool (trait)                             │
│  - Interprets Down/Move/Up in its mode        │
│  - Produces overlay vertices during gesture   │
│  - Produces ToolOutput on gesture completion  │
│  - Stateful per gesture, stateless between    │
└──────────────────────────────────────────────┘
```

---

# 4. PlotTool Trait

```rust
/// A tool defines how pointer gestures are interpreted within a Plot.
///
/// The active tool receives all pointer events before the default
/// handler. It can consume them (Handled) or pass them through
/// (Passthrough) to the default pan/zoom/select behavior.
///
/// INV-297-1: Tools do not mutate ViewState, Series data, or
/// selection state. They produce ToolOutput only.
pub trait PlotTool: Send + Sync {
    /// Stable identifier for serialization and toolbar state.
    fn id(&self) -> &str;

    /// Display name for tooltip.
    fn name(&self) -> &str;

    /// Icon identifier (resolved by the consumer's icon system).
    fn icon(&self) -> &str;

    /// Handle a gesture event. Called on Down, Move, Up, Cancel.
    ///
    /// Receives screen-space coordinates, the current Transform
    /// for world-space conversion, and modifier key state.
    fn handle_gesture(
        &mut self,
        phase: GesturePhase,
        screen_x: f32,
        screen_y: f32,
        transform: &Transform,
        modifiers: Modifiers,
    ) -> GestureResult;

    /// Called when the tool is deactivated (another tool selected,
    /// or Escape pressed). Cancel any in-progress gesture, clear
    /// transient state.
    fn deactivate(&mut self) {}

    /// Overlay vertices to render while the tool gesture is in
    /// progress. Rendered in the offscreen texture after series,
    /// before crosshair. Same vertex format as the overlay pipeline
    /// (position [f32; 2] + color [f32; 4]).
    ///
    /// Empty = no overlay. Called every frame while tool is active.
    fn overlay_vertices(
        &self,
        transform: &Transform,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Vec<OverlayVertex> {
        let _ = (transform, viewport_w, viewport_h);
        Vec::new()
    }
}
```

## 4.1 Gesture Lifecycle

```rust
/// Phase of a pointer gesture within a tool.
pub enum GesturePhase {
    /// Pointer pressed. Tool decides whether to claim this gesture.
    Down,
    /// Pointer moved while pressed. Only sent if Down returned
    /// Handled or Output.
    Move,
    /// Pointer released. Gesture complete.
    Up,
    /// Gesture cancelled (Escape pressed, tool deactivated, focus lost).
    Cancel,
}
```

## 4.2 Gesture Result

```rust
/// Result of tool gesture handling.
pub enum GestureResult {
    /// Tool consumed the event. Plot skips default handling.
    Handled,
    /// Tool consumed the event and produced output for the consumer.
    Output(ToolOutput),
    /// Tool did not handle this event. Fall through to Plot's
    /// default handler (pan/zoom/select).
    Passthrough,
}
```

## 4.3 Tool Output

```rust
/// Output produced by a completed tool gesture.
///
/// The consumer receives this via Plot::take_tool_outputs() and
/// decides what to do: store an annotation, display a measurement,
/// place an order, or anything else.
pub enum ToolOutput {
    /// Measurement between two world-space points.
    Measurement(MeasureResult),
    /// Annotation geometry to be rendered persistently.
    Annotation(AnnotationData),
    /// Consumer-defined output from a custom tool.
    Custom(Box<dyn std::any::Any + Send>),
}
```

---

# 5. Built-In Tools

## 5.1 Pointer (Default)

The current Plot behavior. Click selects, drag pans, Shift+drag
range-selects, scroll zooms. This is Plot's default handler — not
a PlotTool implementation. When Pointer is active, all events pass
through.

```rust
pub struct PointerTool;

impl PlotTool for PointerTool {
    fn id(&self) -> &str { "pointer" }
    fn name(&self) -> &str { "Pointer" }
    fn icon(&self) -> &str { "cursor-arrow" }

    fn handle_gesture(&mut self, ..) -> GestureResult {
        GestureResult::Passthrough
    }
}
```

## 5.2 Measure

Drag between two points. Shows live measurement overlay while
dragging: a line connecting start and end, with world-coordinate
distance, percentage change, and X-span.

```rust
pub struct MeasureTool {
    /// World-coordinate anchor (set on Down).
    start: Option<(f64, f64)>,
    /// Current world-coordinate endpoint (updated on Move).
    end: Option<(f64, f64)>,
}

/// Result of a completed measurement gesture.
pub struct MeasureResult {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    /// Absolute Y distance.
    pub y_distance: f64,
    /// Percentage change: (end_y - start_y) / start_y.
    pub y_percent: f64,
    /// X distance in world units.
    pub x_distance: f64,
}
```

On Down: records start in world coords. Returns Handled.
On Move: updates end. overlay_vertices() draws a dashed line +
floating label showing distance/percentage. Returns Handled.
On Up: emits Output(Measurement(result)). Clears state.

The measurement label is rendered as text in the overlay — NOT as
a Halcyon text widget. This keeps it inside the offscreen texture
so it pans/zooms correctly with the chart content. The text is
rendered as GPU quads using the same technique as axis labels but
positioned at the midpoint of the measurement line.

## 5.3 CrosshairLock

Click to lock a persistent crosshair at a world position. Emits
an annotation. Click again at the same position to remove. Multiple
locks allowed.

## 5.4 DrawHorizontalLine

Click at a Y level → emits AnnotationData::HorizontalLine. The
consumer creates a persistent horizontal line annotation spanning
the viewport. Used for support/resistance, price alerts, order levels.

```rust
pub struct HorizontalLineTool;

impl PlotTool for HorizontalLineTool {
    fn handle_gesture(&mut self, phase, sx, sy, transform, ..) -> GestureResult {
        match phase {
            GesturePhase::Down => GestureResult::Handled,
            GesturePhase::Move => GestureResult::Handled,
            GesturePhase::Up => {
                let world_y = transform.px_to_y(sy);
                GestureResult::Output(ToolOutput::Annotation(
                    AnnotationData::HorizontalLine { y: world_y }
                ))
            }
            GesturePhase::Cancel => GestureResult::Handled,
        }
    }
}
```

## 5.5 DrawRect

Drag to define a rectangle in world coordinates. On Down: anchor.
On Move: preview rectangle overlay. On Up: emit AnnotationData::Rectangle.
Used for supply/demand zones, session ranges, ML training windows.

## 5.6 DrawTrendLine

Click start, click end → emits AnnotationData::TrendLine between
two world-coordinate points. On first Down: anchor start. On first
Up: anchor end point, draw preview line. On second Down/Up: confirm.
Or: Down starts, drag draws preview, Up confirms.

## 5.7 DrawFibonacci

Click top, drag to bottom → emits AnnotationData::FibonacciRetracement.
Standard levels: 0%, 23.6%, 38.2%, 50%, 61.8%, 78.6%, 100%.
overlay_vertices() draws the level lines during drag.

---

# 6. Annotation Data

Tools produce annotation data. Plot does not store, render, or
manage annotations. The consumer receives AnnotationData, stores it
in its own model, and renders it — either as an overlay Series
(e.g., a LineSeries for horizontal levels) or as world-anchored
widgets (SPEC-HAL-296 §8).

```rust
/// Geometry describing an annotation produced by a tool.
///
/// INV-297-5: Plot does not store annotations. The consumer
/// owns persistence, rendering, and lifecycle.
pub enum AnnotationData {
    HorizontalLine { y: f64 },
    VerticalLine { x: f64 },
    Rectangle {
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
    },
    TrendLine {
        start: (f64, f64),
        end: (f64, f64),
    },
    FibonacciRetracement {
        top: f64,
        bottom: f64,
        x_start: f64,
        x_end: f64,
    },
    Text {
        position: (f64, f64),
        content: String,
    },
    /// Consumer-defined annotation type.
    Custom(Box<dyn std::any::Any + Send>),
}
```

---

# 7. PlotToolbar

An overlay widget anchored to the Plot. Contains icon buttons for
each registered tool. The toolbar is a standard Halcyon widget — it
renders above the offscreen texture in the scene graph, gets standard
hover animations, tooltips, and keyboard navigation.

```rust
/// Toolbar overlay for switching tools.
pub struct PlotToolbar {
    entries: Vec<ToolEntry>,
    active_index: usize,
    orientation: ToolbarOrientation,
    visible: bool,
}

pub struct ToolEntry {
    pub tool: Box<dyn PlotTool>,
    /// ActionId for this tool. Toolbar reads current binding from ActionRegistry.
    pub action_id: ActionId,
}

pub enum ToolbarOrientation {
    /// Stacked column (default, left edge of Plot).
    Vertical,
    /// Row (top edge of Plot).
    Horizontal,
}
```

## 7.1 Consumer API

```rust
// With visible toolbar:
let toolbar = PlotToolbar::new()
    .add(PointerTool, ActionId(2000))
    .add(MeasureTool::new(), ActionId(2001))
    .add(HorizontalLineTool, ActionId(2002))
    .add(DrawRectTool::new(), ActionId(2003))
    .add(FibonacciTool::new(), ActionId(2005));

let plot = Plot::new()
    .with_toolbar(toolbar)
    // ...

// Without visible toolbar (action bindings still work):
let plot = Plot::new()
    .with_tool(MeasureTool::new(), ActionId(2001))
    .with_tool(HorizontalLineTool, ActionId(2002))
    // ...

// Toggle toolbar visibility:
plot.set_toolbar_visible(true);

// Programmatic tool activation:
plot.activate_tool("measure");
```

## 7.2 Toolbar Positioning

The toolbar is positioned relative to the Plot's layout rect. Default
position: top-left corner with a small margin. The consumer can
override:

```rust
toolbar.with_position(ToolbarPosition::TopLeft)    // default
toolbar.with_position(ToolbarPosition::TopRight)
toolbar.with_position(ToolbarPosition::BottomLeft)
toolbar.with_position(ToolbarPosition::Custom(x, y))
```

The toolbar is NOT inside the offscreen texture. It floats above
the chart. This means:
- Standard Halcyon text rendering for labels/tooltips
- Standard hover/press animations on buttons
- Standard focus/keyboard navigation
- No interference with chart rendering or GPU pipeline

---

# 8. Tool Output API

The consumer receives tool outputs through the same polling pattern
as selection events (SPEC-HAL-296 §7.3):

```rust
impl Plot {
    /// Get pending tool outputs since last call. Drains the queue.
    pub fn take_tool_outputs(&mut self) -> Vec<ToolOutput>;
}
```

Or via callback (builder pattern):

```rust
Plot::new()
    .on_tool_output(|output: &ToolOutput| {
        match output {
            ToolOutput::Measurement(m) => {
                println!("{:.2} → {:.2} ({:+.1}%)",
                    m.start_y, m.end_y, m.y_percent * 100.0);
            }
            ToolOutput::Annotation(ann) => {
                annotation_store.push(ann.clone());
            }
            ToolOutput::Custom(data) => {
                if let Some(order) = data.downcast_ref::<OrderRequest>() {
                    // Place order at the specified price
                }
            }
        }
    })
```

---

# 9. Gesture Routing in Plot::handle_event

When a tool is active (anything other than Pointer), the event
routing changes:

```rust
// In Plot::handle_event, on PointerDown:
if let Some(tool) = &mut self.active_tool {
    match tool.handle_gesture(GesturePhase::Down, lx, ly, &transform, pe.modifiers) {
        GestureResult::Handled => {
            self.tool_gesture_active = true;
            return true;
        }
        GestureResult::Output(output) => {
            self.pending_tool_outputs.push(output);
            return true;
        }
        GestureResult::Passthrough => {
            // Fall through to default handler below
        }
    }
}
// ... existing default handler (pan/zoom/select) ...
```

On Move and Up, if `tool_gesture_active` is true, route to tool
instead of default handler. On Up or Cancel, clear `tool_gesture_active`.

## 9.1 Overlay Integration

The tool's `overlay_vertices()` output is included in RenderParams
alongside selection rects. The worker renders tool overlay vertices
in the same pass, between series and crosshair.

Add to RenderParams:

```rust
pub struct RenderParams {
    // ... existing fields ...

    /// Tool overlay vertices (measurement line, rect preview, etc.)
    pub tool_overlay_vertices: Vec<OverlayVertex>,
}
```

Plot computes these on the main thread before sending to the pool:

```rust
let tool_overlay = if let Some(ref tool) = self.active_tool {
    tool.overlay_vertices(&phys_transform, w, h)
} else {
    Vec::new()
};
```

---

# 10. Action System Integration

Plot tools are activated via the Action System (SPEC-HAL-020). Each
tool registers a Widget-scoped Action in the ActionRegistry. Shortcuts
are bound through the BindingRegistry, not owned by the tool or toolbar.

## 10.1 Plot Action IDs

Reserved range: 2000–2099.

| ActionId | Name | Default Binding | Description |
|----------|------|-----------------|-------------|
| 2000 | Plot: Pointer | 1 | Switch to Pointer tool (default) |
| 2001 | Plot: Measure | M | Switch to Measure tool |
| 2002 | Plot: Horizontal Line | H | Switch to Horizontal Line tool |
| 2003 | Plot: Rectangle | R | Switch to Rectangle tool |
| 2004 | Plot: Trend Line | T | Switch to Trend Line tool |
| 2005 | Plot: Fibonacci | F | Switch to Fibonacci tool |
| 2010 | Plot: Clear Selection | Escape | Clear selection / deactivate tool |
| 2011 | Plot: Reset View | Home | Reset to initial viewport + re-enable follow |
| 2012 | Plot: Zoom In | = | Zoom in at center |
| 2013 | Plot: Zoom Out | - | Zoom out at center |
| 2014 | Plot: Toggle Toolbar | / | Show/hide tool toolbar |
| 2020–2049 | Reserved | — | Consumer-defined tool actions |

## 10.2 Action Registration

Plot registers its actions when initialized:

```rust
pub fn register_plot_actions(registry: &mut ActionRegistry) {
    use crate::runtime::actions::*;
    use crate::core::event::KeyCode;

    registry.add(Action {
        id: ActionId(2010),
        name: "Plot: Clear Selection",
        description: "Clear selection and deactivate tool",
        category: "Chart",
        shortcut: Some(Shortcut::key(KeyCode::Escape)),
        scope: ActionScope::Widget,
        handler: |_cx| {},
    });
    // ... etc for all plot actions
}
```

## 10.3 Key Event Routing

Plot's handle_event for KeyDown builds a Shortcut from the key event,
looks up the ActionId via the BindingRegistry, and dispatches:

```rust
Event::Key(ke) if ke.is_down && !ke.is_repeat => {
    let action_id = registry.lookup(shortcut);
    match action_id {
        ActionId(2010) => self.handle_escape_action(),
        ActionId(2011) => self.handle_reset_view_action(),
        // ...
    }
}
```

No raw KeyCode matching. All bindings come from the registry.
Users can rebind. All Plot actions appear in the Action Palette.

## 10.4 Escape Precedence (via Action)

ActionId 2010 follows a precedence chain:
1. Cancel in-progress tool gesture
2. Deactivate non-Pointer tool → return to Pointer
3. Clear object selection

## 10.5 Discoverability

All Plot actions are registered in the ActionRegistry:
- Appear in the Action Palette (Cmd+Shift+P)
- Users can search by name
- Users can see and rebind shortcuts
- Category "Chart" groups them

---

# 11. Custom Tools

The consumer extends the tool system by implementing PlotTool:

```rust
/// Example: Order placement tool for a trading application.
struct PlaceOrderTool {
    order_type: OrderType,
}

impl PlotTool for PlaceOrderTool {
    fn id(&self) -> &str { "place_order" }
    fn name(&self) -> &str { "Place Order" }
    fn icon(&self) -> &str { "order-add" }

    fn handle_gesture(
        &mut self, phase, sx, sy, transform, modifiers,
    ) -> GestureResult {
        match phase {
            GesturePhase::Down => GestureResult::Handled,
            GesturePhase::Move => GestureResult::Handled, // show price level preview
            GesturePhase::Up => {
                let price = transform.px_to_y(sy);
                GestureResult::Output(ToolOutput::Custom(
                    Box::new(OrderRequest {
                        price,
                        order_type: self.order_type,
                    })
                ))
            }
            GesturePhase::Cancel => GestureResult::Handled,
        }
    }

    fn overlay_vertices(&self, transform, w, h) -> Vec<OverlayVertex> {
        // Draw horizontal dashed line at current cursor Y
        // with color indicating buy (green) or sell (red)
        // ...
    }
}
```

**INV-297-7:** The consumer can register custom tools via the
PlotTool trait. Plot does not hard-code any tool behavior beyond
the Pointer default. Plot never interprets what a custom tool's
output means.

---

# 12. Invariants

**INV-297-1:** Tools do not mutate ViewState, Series data, or
selection state. They produce ToolOutput. The consumer owns all
downstream mutations.

**INV-297-2:** The active tool is stored on Plot (main thread).
Tool gesture handling runs synchronously in handle_event. No
cross-thread tool state.

**INV-297-3:** Tool overlay vertices are pre-computed on the main
thread and included in RenderParams. The worker renders them
without knowing which tool produced them.

**INV-297-4:** PlotToolbar is a standard Halcyon widget. It renders
above the offscreen texture, not inside it. It uses standard layout,
paint, and event routing.

**INV-297-5:** Plot does not store annotations. The consumer
receives AnnotationData via ToolOutput and owns persistence,
rendering, and lifecycle.

**INV-297-6:** Escape action (ActionId 2010) follows the precedence
chain: tool gesture cancel → tool deactivate → selection clear.
Bound to Escape via BindingRegistry (rebindable).

**INV-297-7:** Custom tools via PlotTool trait. Plot never
hard-codes tool behavior beyond the Pointer default.

**INV-297-8:** Scroll events always pass through to zoom regardless
of active tool. Tools govern click and drag only.

**INV-297-9:** All Plot keyboard interactions go through the Action
System. No raw KeyCode matching in Plot::handle_event for any
action-mappable behavior.

**INV-297-10:** Plot actions use Widget scope (ActionScope::Widget).
They fire only when a Plot has focus. They appear in the Action
Palette grouped under "Chart".

---

# 13. Acceptance Criteria

1. PointerTool passes all events through — existing behavior unchanged
2. MeasureTool shows live distance/percentage line during drag
3. MeasureTool emits MeasureResult on drag completion
4. HorizontalLineTool emits AnnotationData on click
5. DrawRectTool emits AnnotationData::Rectangle on drag completion
6. PlotToolbar renders icon buttons above the Plot
7. Toolbar tool switching works via click and keyboard shortcut
8. Escape returns to Pointer mode from any tool
9. Active tool suppresses pan/zoom when handling gestures
10. Tool overlay renders correctly in offscreen texture during gesture
11. Tool overlay disappears after gesture completion
12. Consumer-defined custom tool works via PlotTool trait
13. Tool outputs received via take_tool_outputs() polling
14. Pan/zoom still work when Pointer tool is active
15. Scroll zoom works regardless of active tool
16. Toolbar visibility toggle works
17. Multiple tools registered via builder API

---

# 14. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. PlotToolbar is a Widget. Tools owned by Plot. |
| B — Phase Separation | Preserved. Tool handling runs in handle_event (mutation phase). Overlay vertices computed before render. |
| C — ID-based Identity | Preserved. Tools identified by string ID. No cross-widget references. |
| D — Pure Data | Preserved. ToolOutput is pure data. Consumer owns interpretation. |
| E — GPU-First | Preserved. Tool overlays rendered as GPU vertices in offscreen texture. |
| F — Sovereignty | Preserved. Tool system is engine-owned. No OS-level tool state. |

No permanent invariants weakened.

---

# 15. Relationship to SPEC-HAL-296

This SPEC extends SPEC-HAL-296 in the following ways:

- **§7 InteractionMode** — superseded. The `InteractionMode` enum
  in §7 was a placeholder. SPEC-297's tool system is the formal,
  extensible replacement.

- **§7.1–7.3 Selection** — unchanged. Selection is the Pointer
  tool's behavior. When a non-Pointer tool is active, selection
  gestures are suppressed (the tool handles them instead).

- **§8 Annotations** — complemented. SPEC-296 §8 defines how
  annotations are rendered (world-anchored widgets). SPEC-297 defines
  how annotations are created (tool gestures produce AnnotationData).

- **§22 Threaded Rendering** — compatible. Tool overlay vertices
  are added to RenderParams and rendered by the worker. No threading
  changes needed.

---

# 16. Non-Goals

- Undo/redo for tool actions (consumer responsibility)
- Annotation persistence to disk (consumer responsibility)
- Annotation editing after creation (separate SPEC if needed)
- Multi-point tools beyond two points (e.g., Gann fan — future extension)
- Touch gestures (desktop-first; touch tool activation is future work)
- Tool presets/profiles (consumer responsibility)
