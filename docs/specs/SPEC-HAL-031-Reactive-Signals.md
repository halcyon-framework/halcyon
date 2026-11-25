# **SPEC-HAL-031: Reactive Signals Layer**

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Scope:** State Model, Dirty Invalidation, Widget API
**Purpose:** Add a small, deterministic reactive layer to Halcyon’s state model that drives precise invalidation and redraw, without introducing global FRP or runtime magic.

---

## 1. Purpose

Halcyon already has:

* A state model (SPEC-HAL-007)
* Caching/diffing and dirty flags (SPEC-HAL-015, SPEC-HAL-026)
* A retained widget tree (SPEC-HAL-010)

What’s missing is a **clean way to connect data changes to UI invalidation** without:

* manually wiring `mark_dirty` everywhere
* full-blown FRP / streams / async graph hell
* hidden observers or global “signals” runtime

This SPEC defines a **small, local signals layer**:

* Simple value signals (`Signal<T>`)
* Widget-local subscriptions
* Immediate, synchronous invalidation hooks
* No background threads, no async, no global registry

Signals are **just a tool for targeted dirtying**. Nothing more.

---

## 2. Design Goals & Non-Goals

### 2.1 Goals

* Allow widgets to **subscribe** to specific pieces of state
* When state changes, mark **only affected widgets** as dirty
* Keep everything **synchronous and single-threaded**
* No allocation storms, no graph rewiring per frame
* Easy to debug: no hidden updates

### 2.2 Non-Goals

* No full FRP/Elm/Redux clone
* No time-traveling or event sourcing
* No async streams or futures
* No global “reactive graph” that spans the entire process

Signals are purely **local glue** from data → dirty flags.

---

## 3. Core Concepts

### 3.1 Signal<T>

Signal is a small, owned reactive cell:

```rust
pub struct Signal<T> {
    // internal
}
```

Properties:

* Owns `T`
* Provides `get()` / `set()`
* Maintains a **list of subscribers** = “who needs to be dirty if this value changes”
* Does **not** auto-propagate to other signals (no FRP graph)

Semantics:

* `set(value)` compares old vs new (via `PartialEq`)
* If equal → no change, no notifications
* If different → notify subscribers immediately (synchronous)

### 3.2 SignalHandle

Widgets never own `Signal<T>` directly. They hold a **handle**:

```rust
pub struct SignalHandle<T> {
    // pointer/id into SignalStore
}
```

Signal handles are:

* Copyable, lightweight
* Used inside widgets to subscribe in `layout()` or `update()`
* Safe to store in widget state

### 3.3 SignalStore

A central, but **scoped**, container:

```rust
pub struct SignalStore {
    // id -> Signal<T> storage
}
```

* Lives inside App / Root context, not global static
* Created once per app
* Accessible via `LayoutContext` / `EventContext` for subscription
* Provides methods:

```rust
fn create<T: 'static + Clone + PartialEq>(&mut self, initial: T) -> SignalHandle<T>;
fn get<T: 'static + Clone>(&self, handle: SignalHandle<T>) -> T;
fn set<T: 'static + Clone + PartialEq>(&mut self, handle: SignalHandle<T>, value: T);
```

Internally tracks subscribers per signal.

### 3.4 SignalSubscription

Link between a **widget node** and a **signal**:

```rust
struct SignalSubscription {
    widget_id: NodeId,
    signal_id: SignalId,
}
```

Subscriptions are stored in `SignalStore`:

* On `set()`, store finds all `widget_id`s subscribed
* Marks them dirty via widget tree API

---

## 4. API Surface

### 4.1 In App / Root

App (or similar root struct) owns the `SignalStore`:

```rust
pub struct App {
    signal_store: SignalStore,
    widget_tree: WidgetTree,
    // ...
}
```

App exposes:

```rust
impl App {
    pub fn signals(&mut self) -> &mut SignalStore {
        &mut self.signal_store
    }
}
```

### 4.2 In LayoutContext / EventContext

To allow widgets to subscribe without touching App directly:

```rust
pub struct LayoutContext<'a> {
    pub tree: &'a mut WidgetTree,
    pub text_engine: &'a mut dyn TextEngine,
    pub theme: &'a Theme,
    pub signals: &'a mut SignalStore,   // NEW
}

pub struct EventContext<'a> {
    pub tree: &'a mut WidgetTree,
    pub signals: &'a mut SignalStore,   // NEW
    // ...
}
```

### 4.3 Widget-Side Usage

Widget might store `SignalHandle<T>`:

```rust
pub struct PriceLabel {
    price_signal: SignalHandle<f64>,
    formatted: String,
}
```

In `layout()`:

```rust
fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints, _children: &[NodeId]) -> Size {
    // subscribe + fetch current value
    let current_price: f64 = cx.signals.watch(self.price_signal, cx.tree.current_node_id());

    self.formatted = format!("{:.2}", current_price);
    // measure text etc...
}
```

In `paint()`:

```rust
fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
    // just use self.formatted; no signal access here
}
```

---

## 5. SignalStore: Operations

### 5.1 Creating Signals

Typically at app startup:

```rust
let price_handle = app.signals().create(0.0_f64);
// store handle in some shared app state
```

### 5.2 Updating Signals

From any state-changing logic (e.g. market data handler):

```rust
app.signals().set(price_handle, new_price);
// this triggers dirty flags for all subscribed widgets
```

Implementation notes:

* `set()` uses `PartialEq` to avoid unnecessary invalidation
* On change, it gathers all subscribers and calls into `WidgetTree`:

```rust
widget_tree.mark_node_dirty(node_id);
```

This ties directly into SPEC-HAL-026 (dirty layout/paint invalidation).

### 5.3 Subscribing from Widgets

Add helper method on `SignalStore`:

```rust
impl SignalStore {
    pub fn watch<T: 'static + Clone>(
        &mut self,
        handle: SignalHandle<T>,
        node_id: NodeId,
    ) -> T {
        self.add_subscription(handle.id, node_id);
        self.get(handle)
    }
}
```

`LayoutContext` wires:

```rust
impl<'a> LayoutContext<'a> {
    pub fn watch<T: 'static + Clone>(&mut self, signal: SignalHandle<T>) -> T {
        let node_id = self.tree.current_node_id();
        self.signals.watch(signal, node_id)
    }
}
```

So widget in `layout()` just does:

```rust
let value = cx.watch(self.price_signal);
```

---

## 6. Integration with Dirty Flags (SPEC-HAL-026)

### 6.1 When Signal Changes

`SignalStore::set()` performs:

1. If `old == new` → return
2. Else:

    * Look up `subscribers: Vec<NodeId>`
    * For each:

        * `widget_tree.mark_node_dirty(node_id)`

Dirty marking respects existing diffing logic:

* If widget caches layout results, layout is recomputed
* If widget caches paint output, its scene nodes are refreshed
* No global repaint necessary

### 6.2 Subscription Lifetime

When widget is removed:

* `WidgetTree` emits `on_unmount` (already present)
* `SignalStore` provides a hook to remove all subscriptions for that `NodeId`

Implementation detail:

* Either maintain:

    * `signal_id → Vec<NodeId>`
    * `node_id → Vec<signal_id>`
* On unmount → remove both directions

This prevents leaks and stale subscriptions.

---

## 7. Safety & Determinism

* No async, no threads
* All notifications are **synchronous**: `set()` finishes before next event
* No re-entrancy: `set()` is only called from event/update loop, never from inside paint

Ordering rule:

* If multiple signals are updated during one tick, all produce dirties
* Layout/paint sees the **latest** values only
* No partial intermediate states

---

## 8. Examples

### 8.1 Reactive Price Label

```rust
pub struct PriceLabel {
    price: SignalHandle<f64>,
    text: String,
}

impl PriceLabel {
    pub fn new(price: SignalHandle<f64>) -> Self {
        Self {
            price,
            text: String::new(),
        }
    }
}

impl Widget for PriceLabel {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> Size {
        let value: f64 = cx.watch(self.price);
        self.text = format!("{:.2}", value);

        // measure text via TextEngine and return Size
        // ...
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // draw self.text
    }
}
```

Market data:

```rust
fn on_tick(app: &mut App, new_price: f64) {
    app.signals().set(app.state.price_signal, new_price);
    // widgets with PriceLabel mark themselves dirty via subscription
}
```

### 8.2 Toggle Button Bound to Signal<bool>

Signal holds the boolean:

```rust
let dark_mode = app.signals().create(false);

// Button inside UI
Button::new("Dark mode")
    .on_click(move |app| {
        let current = app.signals().get(dark_mode);
        app.signals().set(dark_mode, !current);
    });
```

Widgets that care about dark mode subscribe in layout and adjust their theme usage.

---

## 9. Implementation Checklist

### Phase 1 — Core Structures

* [ ] Add `SignalId`, `SignalHandle<T>`, `SignalStore`
* [ ] Implement create/get/set/watch
* [ ] Implement subscription storage (signal → widget(s), widget → signal(s))

### Phase 2 — Context Integration

* [ ] Add `signals: &mut SignalStore` to `LayoutContext` and `EventContext`
* [ ] Add `LayoutContext::watch()` helper
* [ ] Ensure `WidgetTree` exposes `current_node_id()` or similar

### Phase 3 — Dirty Integration

* [ ] Extend `WidgetTree` with `mark_node_dirty(node_id)` (if not already present)
* [ ] On `SignalStore::set()`, mark all subscribed nodes dirty
* [ ] On widget unmount → unsubscribe node from all signals

### Phase 4 — Examples & Tests

* [ ] Add `examples/signals_price_label.rs`
* [ ] Add tests for:

    * Set with equal value → no dirty
    * Set with new value → node marked dirty
    * Node unmount → subscriptions removed
    * Multiple subscribers to same signal

---

## 10. Constraints & Future Work

* Signals are **not** a global app state system; they are local reactive cells
* They are intended mainly for **UI-critical** values (prices, selections, modes)
* Full state modeling remains in SPEC-HAL-007 (structs, stores, reducers, whatever you choose there)

Future SPECs can extend this with:

* Derived signals (computed from others)
* Batched updates (coalescing multiple changes per tick)

But this SPEC must land first as a **minimal, predictable base**.
