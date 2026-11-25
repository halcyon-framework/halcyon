# **SPEC-HAL-007: Halcyon State Model, Data Wiring, and View-Model Discipline**

**Author:** JC
**Date:** 2025-11-16
**Status:** Draft
**Scope:** State architecture, data flow, mutation rules, view building rules, widget state persistence, wiring patterns.
**Purpose:** Anchor Halcyon in a *predictable, deterministic, zero-magic* state model suitable for heavy, real-time trading systems.

---

# **1. Design Principle**

Halcyon must *not* become:

* egui (immediate mode spaghetti)
* React (useState diff hell)
* Elm (global immutable message passing)
* MVVM/XAML (binding graphs, implicit observers)
* Redux (global reducer soup)

It must remain:

**A: Retained widgets**
**B: Explicit state mutation**
**C: Declarative rebuilds**
**D: Predictable identity and lifetime**
**E: Zero hidden magic**

This SPEC defines the exact model.

---

# **2. Two State Layers**

Halcyon’s state consists of two *and only two* layers:

## **Layer 1 — AppState (Global, Persistent, Business Logic)**

A single plain Rust struct owned by the app:

```rust
struct AppState {
    username: String,
    balance: f64,
    selected_account: Option<AccountId>,
    // trading configs, user profile, etc.
}
```

**Rules:**

* Owned by the App.
* Mutated only from event handlers.
* Passed as immutable props to widgets.
* Not stored by widgets.
* Serializable.
* Where domain logic lives.

AppState = **your trading system’s truth.**

---

## **Layer 2 — WidgetState (Local, Ephemeral UI Behavior)**

Each widget instance maintains internal state:

```rust
struct TextFieldState {
    cursor: usize,
    selection: Option<(usize, usize)>,
    scroll_x: f32,
    focused: bool,
}
```

WidgetState contains:

* cursor positions
* hover
* scroll offset
* view transform
* toggles
* ephemeral view-only info

WidgetState NEVER contains:

* user profile
* account IDs
* chart data
* backend config
* anything domain specific

WidgetState = **what the widget needs to function (not data)**.

---

# **3. How Data Flows (One Way Only)**

### **DATA → PROPS → WIDGET → SCENE → GPU**

Event flow:

### **USER EVENT → WIDGET STATE → APP STATE → REBUILD → RENDER**

There is *no* backwards binding, no two-way sync magic.

---

# **4. The Halcyon “View-Model” Rule**

This is the core discipline:

### **Widgets may read AppState (via props).

Widgets may NOT mutate AppState directly.
Widgets request mutations via callbacks.**

Example:

```rust
Button::new()
    .label("Buy")
    .on_click(|app: &mut AppState| {
        app.place_order(OrderType::Buy);
    })
```

The callback receives **&mut AppState**, and that is the only entry point for data mutation.

Widgets do not own the data.
Widgets do not store the data.
Widgets do not sync the data.
Widgets do not hold references to AppState.

This eliminates React-style state poisoning.

---

# **5. Widget Identity (Critical for Trading UI)**

A stable widget ID must exist for:

* selection
* focus
* scroll position
* text cursor
* drag-and-drop
* hot path widgets like order ladders / chart modules

Halcyon uses:

```rust
pub struct WidgetId(pub usize);
```

Later we can extend to:

```
struct WidgetId { index: u32, generation: u32 }
```

Guaranteed unique *within a single frame cycle*.

Important:

**Widget identity is tied to build order, not runtime object mutation.**

This gives you stable identity choreography without magic.

---

# **6. App → UI Wiring**

Every frame:

```
AppState is stable
↓
root_widget(&AppState) → Widget
↓
layout pass
↓
paint pass → SceneGraph
↓
renderer_wgpu → GPU
```

AppState never changes unless a user event or system event triggers it.

Widgets rebuild props each frame, but their internal WidgetState persists.

This gives you:

* predictable rebuild cost
* zero hidden mutation
* no stutter during trading events
* complete determinism

---

# **7. Event Handling Discipline**

Events go through these stages:

1. **Hit-test resolves target WidgetId**
2. **Capture → Target → Bubble routing**
3. **WidgetState mutates**
4. **Widget requests AppState mutation via callback**
5. **AppState updates**
6. **UI rebuilds with new props**

No widget directly mutates AppState without going through App callback.

No global observers.

No broadcasting.

No reflection.

---

# **8. “Rebuild Every Frame” but NOT Immediate Mode**

We rebuild the *widget tree*, not the *widget state*.

Widget tree rebuild means:

* new props assigned
* internal WidgetState persists
* structural diffs are tiny
* layout + paint run every frame

Immediate mode (egui) reinitializes everything — Halcyon does not.

Retained mode = stable identity + persistent state.

---

# **9. Asynchronous Data Sources (Trading Specific)**

Trading systems have:

* tick streams
* candle aggregators
* orderbook feeds
* latency-critical data paths

Halcyon handles async data via:

### **AppState + message queue + frame pump**

1. Feed updates push messages into a channel.
2. AppRunner pulls messages at frame boundary.
3. AppState is mutated.
4. UI rebuilds.

This ensures:

* no lock contention
* no UI thread blocking
* no data races
* deterministic UI ticks

---

# **10. View-Model for Complex Tools**

More complex screens (chart view, order blotter, ladder) follow:

### **AppState contains domain data

WidgetState contains view transforms
Callbacks mutate AppState**

Example for a chart widget:

```rust
ChartWidget::new()
    .view_transform(&self.chart_view)         // WidgetState
    .candles(&app_state.candles)              // AppState (immutable)
    .on_select(|sel, app| app.selected_range = sel)
```

Chart zoom, pan, selection → WidgetState.
Selected region → AppState.
No duplication.
No two-way sync.

---

# **11. Multi-Panel / Docking Safety**

Future docking system must follow the same model:

* every Panel has WidgetState (dock_pos, collapsed, size)
* AppState owns the app semantics (what data each panel shows)
* no hidden mutation when panels move

This is how we keep Halcyon stable even when complex.

---

# **12. Serialization / Persistence Layer**

AppState is trivially serializable with Serde.

WidgetState is NOT persisted except:

* window positions
* panel splits
* some scroll offsets (optional)

This keeps save files clean.

---

# **13. Success Criteria**

A correct implementation of this SPEC must deliver:

### **A deterministic state model**

No flicker, no weird loops, no hidden sync.

### **Perfect event → state → rebuild discipline**

No part of UI affects another part unless specified.

### **UI remains smooth under real trading load**

1000+ UI updates/sec safely handled.

### **No observer graph, no binding hell**

Callbacks only.

### **WidgetState never contaminates AppState**

Guaranteed separation.

### **Charting integration remains trivial**

Because chart widget receives:

* AppState candles
* WidgetState view transforms
* callbacks

No modification to the engine.
