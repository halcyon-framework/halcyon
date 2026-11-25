# **SPEC-HAL-000: Halcyon — Glossary, Naming Conventions & Design Principles**

**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** Establish unified terminology, naming conventions, and core design principles for the entire Halcyon codebase.
**Purpose:** Prevent naming inconsistencies, architectural confusion, and unnecessary refactoring during implementation.

---

## 1. Purpose

This SPEC defines the **common language** of Halcyon.

Before writing any code, we establish:
- What core terms mean
- How to name types, modules, and variables
- Fundamental design principles that apply across all SPECs

This is a **reference document** — concise, authoritative, permanent.

---

## 2. Core Terminology

### Widget
A logical UI component with:
- Identity (`WidgetId`)
- Internal state
- Layout behavior
- Event handling
- Paint output (emits scene nodes)

Examples: `Button`, `TextField`, `Row`, `Column`, `Panel`

**Not** a DOM element. **Not** immediate-mode. Retained, persistent.

### Scene / Scene Graph
The GPU-ready representation of the UI:
- Tree of drawable primitives (`Rect`, `Text`, `Line`, etc.)
- Emitted by widgets during paint phase
- Consumed by renderer
- Does **not** contain widget logic

### Node (Scene Node)
A single drawable primitive in the scene graph.
Examples: `RectNode`, `TextNode`, `ClipNode`

**Not** a widget. Nodes are data; widgets are logic.

### Element
**Avoided term.** Use `Widget` or `Node` explicitly to prevent DOM confusion.

### Rect / RectF
A rectangle defined by `(x, y, width, height)`.
- `Rect`: integer coordinates (rare)
- `RectF`: floating-point coordinates (default)

Origin: top-left corner.

### Box / BoxConstraints
Layout constraints passed to widgets:
```rust
struct Constraints {
    min_width: f32,
    max_width: f32,
    min_height: f32,
    max_height: f32,
}
```

**Not** a CSS box model. Simpler.

### Layout
The process of assigning sizes and positions to widgets.
- Top-down constraint passing
- Bottom-up size resolution
- Results in `Rect` per widget

### Paint
The process of emitting scene nodes from widgets.
- Occurs after layout
- Widgets output draw commands
- No layout changes allowed during paint

### Event Routing
The process of delivering input events (pointer, keyboard) to widgets.
- Three phases: Capture → Target → Bubble
- Hit-testing determines target widget
- Pointer capture for drag operations

### State Model
The data layer that widgets observe.
- Widgets are stateless views of app state
- Mutations trigger rebuilds
- No two-way binding, no magic observers

### Theme
The visual styling system:
- Color palettes
- Typography scales
- Component-specific styles
- Runtime switchable

---

## 3. Naming Conventions

### Types (Structs, Enums, Traits)
- `PascalCase`
- Examples: `Widget`, `SceneBuilder`, `EventPhase`, `ButtonStyle`

### Functions & Methods
- `snake_case`
- Examples: `build_scene()`, `handle_event()`, `set_theme()`

### Variables
- `snake_case`
- Examples: `widget_id`, `scene_graph`, `pointer_pos`

### Constants
- `SCREAMING_SNAKE_CASE`
- Examples: `MAX_WIDGETS`, `DEFAULT_THEME`, `FPS_TARGET`

### Modules (Files)
- `snake_case`
- Examples: `widget.rs`, `scene_graph.rs`, `event_routing.rs`

### Builder Pattern
Use `.with_*()` for optional properties:
```rust
Button::new()
    .with_label("Click Me")
    .with_style(ButtonStyle::Primary)
```

Use `.on_*()` for event handlers:
```rust
Button::new()
    .on_click(|app| { /* ... */ })
```

### Widget IDs
Type: `WidgetId(u64)` (not `usize`)
- Globally unique within widget tree
- Stable across frames
- Auto-generated or explicit
- `u64` ensures no rollover on long-running trading apps

---

## 4. Coordinate Systems

### Window Space
- Origin: top-left of OS window
- Units: logical pixels (after DPI scaling)
- Source of all input events

### Widget Local Space
- Origin: top-left of widget's `Rect`
- Relative to parent
- Used for layout and event handling
- **Always uses `f32` logical pixels** (physical pixel rounding handled only in renderer)

### GPU / NDC Space
- Normalized Device Coordinates: `[-1, 1]` for both axes
- Only used internally by renderer
- Widgets never see this

### DPI / Scaling
- All sizes in **logical pixels**
- Physical pixels = logical pixels × scale_factor
- Winit provides scale_factor
- Widgets work in logical space only

---

## 5. Z-Order Convention

**Higher Z = Rendered Later = Appears On Top**

Layers:
```
0:   Background
1:   Base UI
2:   Panels / Containers
3:   Widgets (buttons, text, etc.)
4:   Overlays (tooltips, modals)
5:   Drag Preview / Active Elements
```

Within a layer, widgets at same Z are drawn in tree order (depth-first).

---

## 6. Design Principles

### Principle 1: Explicit Over Implicit
- No hidden magic
- No auto-wiring
- No global state (except where unavoidable)

### Principle 2: Retained Mode
- Persistent widget tree
- Incremental updates
- Reuse structure across frames

### Principle 3: Deterministic Behavior
- Same inputs → same outputs
- No race conditions
- No non-deterministic layout

### Principle 4: Performance First
- Zero allocations in hot paths
- Batched GPU operations
- Minimal state diffing

### Principle 5: Trading-Grade Standards
- 120-240 FPS target
- Sub-1ms event latency
- Stable memory usage over hours

### Principle 6: No Web Stack Influence
- No HTML/CSS/DOM terminology
- No MVVM/React patterns
- Native desktop first

### Principle 7: Monotonic Time Source
- **Halcyon uses monotonic high-precision timer (`Instant`) as its only time source**
- Used for: animations, double-click detection, frame timing, all time-based events
- Never use system time or wall-clock time for UI timing

---

## 7. Module Organization

```
halcyon/
  src/
    lib.rs              // Public API exports

    core/
      widget.rs         // Widget trait
      scene.rs          // Scene graph types
      layout.rs         // Layout system
      event.rs          // Event types
      theme.rs          // Theme system

    widgets/
      button.rs
      textfield.rs
      row.rs
      column.rs
      panel.rs

    platform/
      winit.rs          // Winit integration
      wgpu.rs           // WGPU renderer

    utils/
      id.rs             // ID generation
      math.rs           // Rect, Size, Point
```

---

## 8. Error Handling

- Use `anyhow::Result<T>` for application-level errors
- Use `thiserror::Error` for library-level errors
- Panic only for programming bugs (violated invariants)
- No silent failures

---

## 9. Dependency Policy

### Core Dependencies (Allowed)
- `wgpu` (rendering)
- `winit` (windowing)
- `rusttype` or `fontdue` (text)
- `serde` (serialization)
- `anyhow`, `thiserror` (errors)

### Forbidden Dependencies
- `tokio`, `async-std` (no async runtime)
- `diesel`, `sqlx` (no ORM)
- Web frameworks
- Immediate-mode UI libraries
- **No GUI frameworks of any kind as inspiration or code reference**
  (Not GTK, Qt, Dear ImGui, egui, Druid, Iced, Flutter, SwiftUI, React Native, etc.)
  Their paradigms must not contaminate Halcyon's design

### External Integrations
- Trading/charting logic lives in separate crates
- Halcyon is pure UI framework

---

## 10. Testing Strategy

- Unit tests for core logic
- Integration tests for widget behavior
- Visual regression tests (manual for now)
- Benchmarks for hot paths (layout, paint, diffing)

---

## 11. Documentation Standards

### Code Comments
- `///` for public API
- `//` for implementation notes
- No obvious comments (e.g., `// increment counter`)

### SPEC Format
- Problem statement first
- Design principles
- API examples
- Implementation checklist

### Examples
Every major feature needs a runnable example in `/examples/`

---

## 12. Version Strategy

Semantic versioning:
- `0.x.y` during development
- `1.0.0` when production-ready for trading use

Breaking changes allowed before `1.0.0`.

---

## End

This SPEC establishes the **foundation language** of Halcyon.

All subsequent SPECs, code, and documentation must follow these conventions.

When in doubt, refer to SPEC-HAL-000.
