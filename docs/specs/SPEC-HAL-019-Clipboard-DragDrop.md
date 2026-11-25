**SPEC-HAL-019: Halcyon — Clipboard, Drag & Drop, and OS Integration**
**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** System clipboard, copy/paste, OS cursor control, drag & drop (internal + external), and integration with winit/OS.
**Primary Use-Case:** Trading UIs requiring copy/paste (price, symbol), precise cursor feedback, file import/export, and draggable UI elements.

---

# 1. Purpose

Halcyon needs a minimal but complete bridge to the operating system.

Trading workflows depend on:

* Copy/paste for symbols, prices, IDs.
* Precise OS cursor shape changes (hand, text, resize).
* Drag & drop inside the app (panels, widgets, lists).
* Drag & drop from OS (files into a chart or workspace).

This SPEC defines the required features and how Halcyon interacts with the OS through `winit`.

No bloat. No “desktop app” framework nonsense. Just what is required to function as a serious trading workstation.

---

# 2. Clipboard API

We expose a **very small, explicit** clipboard interface:

```rust
pub trait ClipboardProvider {
    fn get(&mut self) -> Option<String>;
    fn set(&mut self, text: &str);
}
```

Halcyon provides a default implementation using:

* `winit::clipboard::Clipboard` on Windows
* Platform clipboard on macOS/X11/Wayland

Halcyon keeps a runtime handle:

```rust
pub struct HalClipboard {
    provider: Box<dyn ClipboardProvider>,
}

impl HalClipboard {
    pub fn get(&mut self) -> Option<String> { self.provider.get() }
    pub fn set(&mut self, text: &str) { self.provider.set(text) }
}
```

**Widgets or screens access via ScreenContext:**

```rust
cx.clipboard.set("BTCUSDT");
```

---

# 3. Copy & Paste Integration with Widgets

### TextField

* `Ctrl+C` → copy selection
* `Ctrl+X` → cut selection
* `Ctrl+V` → paste clipboard contents
* Implemented fully in SPEC-HAL-013 (TextField) using this clipboard API.

### Other widgets

Any widget may opt to copy/paste:

* Table rows (copy order ID)
* Chart values (copy price)
* Ladder levels (copy price level)
* Context menu actions (coming later)

The main API:

```rust
fn handle_event(&mut self, event: &Event, phase: EventPhase, cx: &mut ScreenContext) -> bool {
    if let Event::Key(k) = event {
        if k.is_down && k.key_code == KeyCode::C && k.modifiers.ctrl {
            cx.clipboard.set(self.to_copy_string());
            return true;
        }
    }
    false
}
```

---

# 4. OS Cursor Control

Trading widgets require exact cursor modes:

* Arrow
* I-beam (text cursor)
* Hand (panning a chart)
* Resize (splitters/panels)
* Crosshair (chart inspection)

Halcyon exposes:

```rust
pub enum CursorIcon {
    Default,
    Text,
    Pointer,
    Grab,
    Grabbing,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    Crosshair,
}
```

A widget signals cursor changes via:

```rust
cx.platform.set_cursor(CursorIcon::Crosshair);
```

This call simply forwards the request to `winit` on each frame.

**Rule:** requests accumulate, last widget under pointer wins.

---

# 5. Drag & Drop (Internal)

Internal drag & drop = drag within Halcyon:

* Dragging a panel
* Dragging a widget in a list
* Dragging a file tab
* Dragging a selection box
* Dragging markers/annotations

**We reuse HAL-009 Pointer Capture:**

1. Widget receives `PointerDown` → captures pointer.
2. Widget receives all `PointerMove`s → updates drag state.
3. Upon `PointerUp` → completes drop logic, releases capture.

To support cross-widget drops:

### Drag Payload

```rust
pub enum DragData {
    Text(String),
    ItemId(u64),
    Price(f64),
    Symbol(String),
    Custom(Box<dyn Any>),
}
```

### Drag State

Global shared struct:

```rust
pub struct DragState {
    pub active: bool,
    pub origin_widget: Option<WidgetId>,
    pub payload: Option<DragData>,
    pub position: (f32, f32),
}
```

Access through:

```rust
cx.drag.start(payload)
cx.drag.update(...)
cx.drag.end()
```

Widgets can implement:

```rust
fn on_drag_enter(...)
fn on_drag_over(...)
fn on_drag_leave(...)
fn on_drop(...)
```

But these are optional — trading apps often implement drop at a higher level.

---

# 6. Drag & Drop (External / OS)

This includes:

* dropping files onto the app
* copying text from outside into TextField
* using OS drag onto the chart (dragging a CSV or trading log)

`winit` supports:

* `WindowEvent::DroppedFile(path)`
* `WindowEvent::HoveredFile(path)`
* `WindowEvent::HoveredFileCancelled`

Halcyon maps these to high-level events:

```rust
pub enum FileDropEvent {
    HoverEnter(String),
    HoverLeave,
    Drop(String),
}
```

Then dispatches to the **current screen** via:

```rust
fn handle_app_event(&mut self, event: &AppEvent, cx: &mut ScreenContext);
```

Trading example:

```rust
fn handle_app_event(&mut self, evt: &AppEvent, cx: &mut ScreenContext) {
    if let AppEvent::FileDrop(FileDropEvent::Drop(path)) = evt {
        self.import_file(path, cx);
    }
}
```

---

# 7. OS Integration Details

### 7.1. DPI / Scaling

Already covered in HAL-005; clipboard and drag/drop use window-space coordinates.

### 7.2. Window Title / Icon

Halcyon exposes minimal functionality:

```rust
cx.platform.set_window_title("Halcyon Workspace");
cx.platform.set_window_icon(path);
```

Trading apps may dynamically set the title (e.g., “BINANCE – LIVE – LATENCY 23ms”).

### 7.3. File Picker (optional future)

We *can* expose OS file dialogs, but not required yet.

---

# 8. Performance and Safety

Clipboard operations are:

* synchronous
* low cost
* safe to call

Drag & drop:

* uses pointer capture (already high-perf)
* requires **no heap allocations** during move events

Cursor updates:

* winit blocks duplicate calls
* Halcyon should track “last cursor” to avoid repeats

---

# 9. Implementation Checklist (for Claude Code)

### Clipboard

* [ ] Introduce `ClipboardProvider` trait
* [ ] Implement platform clipboard via winit
* [ ] Add `clipboard` field to `ScreenContext`
* [ ] Support TextField copy/paste
* [ ] Add minimal tests (copy text, paste text)

### Cursor Control

* [ ] Define `CursorIcon` enum
* [ ] Add `set_cursor()` on `PlatformContext`
* [ ] Modify event loop to update cursor per-frame
* [ ] Test cursor switching on buttons/textfields

### Internal Drag & Drop

* [ ] Add `DragState`
* [ ] Add `DragData`
* [ ] Expose start/update/end API through ScreenContext
* [ ] Ensure integration with pointer capture
* [ ] Add simple example (drag rectangles)

### External Drag & Drop

* [ ] Map winit dropped files to `AppEvent::FileDrop`
* [ ] Wire to screen handler
* [ ] Add example: drop CSV on app → logs path

---

# 10. Result

With SPEC-HAL-018 implemented:

* Text fields finally behave like real editors (copy/paste).
* Charting widget can copy price/time ranges.
* Ladder/trading widgets can copy symbol/price.
* Dragging selection boxes, panels, or custom items works natively.
* Charts can import files by dropping them from OS.
* You get proper cursor feedback (crucial for trading precision).
* Everything is fully deterministic and under your control — zero magic.

Halcyon becomes OS-aware without pulling in any bloated subsystems.
