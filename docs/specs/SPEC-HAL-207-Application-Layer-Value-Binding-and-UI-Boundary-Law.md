# SPEC-HAL-207: Application-Layer Value Binding & UI Boundary Law

**Status:** Binding
**Layer:** Application / State Integration
**Scope:** All widget → application data flows
**Ground Truth:** SPEC-HAL-188A, SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-235, SPEC-HAL-236, SPEC-HAL-246, SPEC-HAL-247, SPEC-HAL-248

---

## 0. Purpose

This SPEC defines the only legal mechanism by which application state may consume widget-produced values.

It exists to:

- eliminate implicit widget state access
- prevent deadlocks and re-entrancy bugs
- formalize UI-visible boundaries
- make value transfer explicit, deterministic, and frame-safe
- provide a user-facing API that is usable without leaking low-level mechanics

This SPEC is binding law, not a convenience API description.

---

## 1. Definitions

### 1.1 UI-Visible Boundary

A UI-visible boundary is any execution context that:

- is entered by the UI framework
- may re-enter in the same frame or next frame
- may call user code
- may traverse the widget tree
- may trigger layout, paint, or event dispatch

Examples (non-exhaustive):

- `Widget::handle_event`
- `Widget::layout`
- `Widget::paint`
- `bind`, `sync`, `take_changed`
- `flush_pending_actions`
- any closure passed to `ScreenContext`

### 1.2 Application Layer

The application layer is all code that:

- owns mutable domain state
- owns models, forms, view-models
- may use locks, async, background threads
- is not part of widget execution

---

## 2. Core Law: UI Boundary Isolation

### INV-207-A1: No Locks Across UI Boundaries

Locks must never be held when crossing a UI-visible boundary.

Forbidden:

```rust
let guard = model.lock();

cx.bind(|b| {
    b.text("email").sync(&mut guard.email);
});
```

Legal:

```rust
let snapshot = {
    let guard = model.lock();
    guard.clone_for_ui()
};

cx.bind(|b| {
    b.text("email").sync(&mut snapshot.email);
});
```

### INV-207-A2: Widgets Must Never Observe Locks

UI APIs must not accept:

- `MutexGuard`
- `RwLockGuard`
- any lock-carrying reference

Widgets are lock-agnostic by law.

---

## 3. Widget Responsibility Boundary

### INV-207-B1: Widgets Do Not Own Application State

Widgets:

- render
- accept input
- emit value changes

They do not:

- store application state
- store navigation state (owned by NavigationHost — SPEC-HAL-247)
- store authoritative focus state (owned by the interaction domain's Focus Resolver/FocusManager — SPEC-HAL-235/247)
- store authoritative surface identity (owned by Compositor; navigation lifecycle references remain with NavigationSurfaceOrchestrator — SPEC-HAL-247)
- synchronize models
- manage ownership
- coordinate async behavior

### INV-207-B2: Widgets Emit Values, Not Effects

Widgets produce value deltas, not mutations.

Example:

```rust
checkbox.take_changed() -> Option<bool>
```

No widget ever writes into a model directly.

---

## 4. Application-Layer Binding Model

### 4.1 Binding Is Application-Owned

All binding logic lives outside widgets. FormBinder owns the binding authority.

Widgets are identified by stable identifiers (string keys), not `NodeId`.

Example:

```rust
form.bind(cx, |b| {
    b.bool("accept_terms").sync(&mut model.accepted);
    b.text("email").sync(&mut model.email);
});
```

### 4.2 .sync() Semantics (Normative)

`.sync(target)` means:

> If the widget identified by this key produced a value change since the last sync, copy that value into target exactly once.

Properties:

- edge-triggered
- idempotent per frame
- no replay
- no duplication
- no mutation if unchanged

### INV-207-C1: Single-Transfer Rule

Each value change may be observed at most once per binding cycle.

### INV-207-C2: Snapshot Semantics

Bindings observe a snapshot of widget outputs.

They do not:

- hold references into widgets
- pull values reactively
- poll continuously

---

## 5. No Synthetic Coupling

### INV-207-D1: No Widget-Embedded Bindings

Widgets must never:

- store binding metadata
- store references to application values
- carry smart pointers to application state

All binding is external and isolated.

### INV-207-D2: No Event Bus for Primitive Values

Primitive widget values must not be emitted via a global event bus.

Reason:

- breaks locality
- destroys determinism
- complicates ownership
- encourages hidden coupling

---

## 6. Deadlock Prevention Law

### INV-207-E1: No Blocking in UI Execution

UI-visible code must be:

- non-blocking
- lock-free
- allocation-stable (steady state)

### INV-207-E2: Lock Scope Containment

Locks may only exist:

- entirely before UI execution
- entirely after UI execution

Never overlapping.

### INV-207-E3: No User-Controlled Locking in Bindings

Users must not be able to:

- manually lock inside bindings
- hold locks across bind / sync
- inject blocking behavior

The API must make this structurally impossible.

---

## 7. Required API Shape (Implications)

This SPEC requires the existence of:

- an application-layer binding facade
- string-keyed widget lookup
- per-frame change aggregation
- explicit sync / take semantics

It forbids:

- immediate-mode value pulling
- widget-scoped state queries
- implicit global observers

---

## 8. Invariant Supremacy

### INV-207-Z1: Supremacy Clause

No UX improvement, convenience feature, or shortcut may violate these invariants.

If new behavior is needed, a new SPEC must be written.

---

## 9. Concrete API Specification (Normative)

### 9.1 Module Location

`src/model/form_binder.rs`

Exported via `halcyon::model::form_binder`.

### 9.2 BindingKey

Interned identifier for a binding slot. Opaque to application code.

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingKey(u32);
```

- Created via `ChangeWriter::intern(key)` or internally by `BindingContext` typed accessors.
- Same string always returns the same `BindingKey`.
- Never exposed as raw `u32` to application code.
- Separate from `NodeId` — no widget tree coupling.

### 9.3 ChangeWriter

Write-side handle to the change store. Cloneable. Given to widgets for emission.

```rust
#[derive(Clone)]
pub struct ChangeWriter {
    inner: Rc<RefCell<ChangeStoreInner>>,
}

impl ChangeWriter {
    /// Intern a string key into a BindingKey.
    pub fn intern(&self, key: &str) -> BindingKey;

    /// Write a bool change. Marks slot dirty. Final value wins per frame.
    pub fn set_bool(&self, key: BindingKey, value: bool);

    /// Write a selection change (Option<u32>). Marks slot dirty.
    pub fn set_selection(&self, key: BindingKey, value: Option<u32>);

    /// Write a text change. Marks slot dirty.
    pub fn set_text(&self, key: BindingKey, value: String);
}
```

- `Rc<RefCell<_>>` — single-threaded, no Send/Sync required.
- Multiple writes per frame: final value wins, slot remains dirty.
- Writing during `bind()` closure will panic (RefCell borrow conflict). This is correct — writes and reads must not interleave.

### 9.4 FormBinder

Application-layer binding facade. Owns the shared change store.

```rust
pub struct FormBinder {
    inner: Rc<RefCell<ChangeStoreInner>>,
}

impl FormBinder {
    /// Create a new empty FormBinder.
    pub fn new() -> Self;

    /// Create a ChangeWriter handle for widget emission.
    pub fn writer(&self) -> ChangeWriter;

    /// Execute a binding pass. All sync/take operations occur inside the closure.
    ///
    /// `_cx` serves as a phase marker — proves caller has ScreenContext access.
    pub fn bind<F>(&mut self, _cx: &ScreenContext<'_>, f: F)
    where
        F: FnOnce(&mut BindingContext<'_>);

    /// Manual write: set a bool value by string key. For testing and stubbing.
    pub fn set_bool(&mut self, key: &str, value: bool);

    /// Manual write: set a selection value by string key.
    pub fn set_selection(&mut self, key: &str, value: Option<u32>);

    /// Manual write: set a text value by string key.
    pub fn set_text(&mut self, key: &str, value: String);
}
```

- Owned by `Screen` implementation as a struct field.
- `bind()` borrows the `RefCell` mutably for the closure duration, preventing concurrent writes.
- Manual `set_*` methods intern the key and write in one step. Used before widget retrofits are complete.

### 9.5 BindingContext

Scoped context passed into `bind()` closure. Lifetime prevents escape.

```rust
pub struct BindingContext<'a> {
    store: &'a mut ChangeStoreInner,
}

impl<'a> BindingContext<'a> {
    /// Access a bool binding by string key.
    pub fn bool(&mut self, key: &str) -> BoolBinding<'_>;

    /// Access a selection binding by string key.
    pub fn selection(&mut self, key: &str) -> SelectionBinding<'_>;

    /// Access a text binding by string key.
    pub fn text(&mut self, key: &str) -> TextBinding<'_>;
}
```

- `'a` is the lifetime of the `RefCell` borrow inside `bind()`.
- Typed accessors intern the key if not yet interned, then return a handle to the corresponding slot.
- Each type has its own key namespace. `b.bool("x")` and `b.text("x")` address separate slots.

### 9.6 Typed Binding Handles

Lifetime-tied to `BindingContext`. Cannot escape the `bind()` closure.

```rust
pub struct BoolBinding<'a> { /* &'a mut BoolSlot or None */ }
pub struct SelectionBinding<'a> { /* &'a mut SelectionSlot or None */ }
pub struct TextBinding<'a> { /* &'a mut TextSlot or None */ }

impl<'a> BoolBinding<'a> {
    /// If changed since last sync, copy value into target. Clears dirty flag.
    /// No-op if unchanged or key never written.
    pub fn sync(&mut self, target: &mut bool);

    /// If changed since last sync, return value and clear dirty flag.
    /// Returns None if unchanged or key never written.
    pub fn take(&mut self) -> Option<bool>;
}

impl<'a> SelectionBinding<'a> {
    pub fn sync(&mut self, target: &mut Option<u32>);
    pub fn take(&mut self) -> Option<Option<u32>>;
}

impl<'a> TextBinding<'a> {
    pub fn sync(&mut self, target: &mut String);
    pub fn take(&mut self) -> Option<String>;
}
```

### 9.7 .sync() Contract (Normative)

Given a typed binding handle `h` for key `k`:

| Condition | `.sync(&mut target)` | `.take()` |
|-----------|---------------------|-----------|
| Key never written | No-op (target unchanged) | `None` |
| Key written, slot clean (already synced) | No-op (target unchanged) | `None` |
| Key written, slot dirty | `*target = slot.value`; clear dirty | `Some(slot.value)`; clear dirty |

Additional rules:

- **Exactly-once per sync pass**: After `sync()` or `take()` clears the dirty flag, subsequent calls in the same `bind()` for the same key return no-op/`None`.
- **Multiple writes before sync**: Final value wins. Dirty flag set on every write, value overwritten.
- **Multiple changes in one frame**: Coalesced — only the final value is visible to the binder.
- **Text sync**: `target.clone_from(&slot.value)`. Text take: `Some(slot.value.clone())`.
- **Duplicate key in same bind pass**: Second accessor for same key and type returns a handle to the same slot (already clean if first handle synced).

### 9.8 Error / Invalid-Key Behavior

| Scenario | Behavior |
|----------|----------|
| Key never written to | `sync()` is no-op, `take()` returns `None` |
| Key exists in different type namespace | No conflict — separate namespaces |
| `bind()` called outside `flush_pending_actions` | Compiles only if `&ScreenContext` is available (soft phase guard) |
| Widget writes during `bind()` closure | Runtime panic (RefCell borrow conflict) — correct by design |
| Same key interned twice | Returns same `BindingKey` — idempotent |

### 9.9 Widget Integration Surface

Widgets emit into the change store via `ChangeWriter`:

```rust
// Widget configuration (application code in Screen::build):
let key = form.writer().intern("accept_terms");
checkbox.set_binding(key, form.writer());

// Widget internal emission (on state change):
if let Some((key, writer)) = &self.binding {
    writer.set_bool(*key, new_checked);
}
```

### 9.10 Integration Point

The binder is polled in `Screen::flush_pending_actions`:

```rust
fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
    self.form.bind(cx, |b| {
        b.bool("accept_terms").sync(&mut self.model.accepted);
        b.selection("color").sync(&mut self.model.selected_color);
        b.text("email").sync(&mut self.model.email);
    });
}
```

This runs after event routing, before scene build. Changes applied here are visible in the same frame's paint.

### 9.11 Compile-Time Enforcement

The following patterns must not compile:

**Storing a binding handle outside the closure:**

```rust
// MUST NOT COMPILE
let mut stolen = None;
binder.bind(cx, |b| {
    stolen = Some(b.bool("x")); // lifetime error
});
```

**Storing BindingContext beyond the closure:**

```rust
// MUST NOT COMPILE
let mut ctx_ref = None;
binder.bind(cx, |b| {
    ctx_ref = Some(b); // lifetime error
});
```

**No lock guards in public API:**

- `FormBinder` never returns `RefMut`, `MutexGuard`, or any guard type.
- `ChangeWriter` never returns a guard.
- All interior mutability is hidden behind value-copying methods.

---

## 10. Authority Rebinding (Post-Navigation v2)

Every value-binding example in this SPEC binds to exactly one authority.

| Example | Authority | Relationship |
|---|---|---|
| `b.text("email").sync(&mut model.email)` | FormBinder | Application layer **consumes** widget-emitted text value |
| `b.bool("accept_terms").sync(&mut model.accepted)` | FormBinder | Application layer **consumes** widget-emitted bool value |
| `b.selection("color").sync(&mut model.selected_color)` | FormBinder | Application layer **consumes** widget-emitted selection value |
| `checkbox.take_changed() -> Option<bool>` | Widget | Widget **owns** emission; caller **consumes** the delta |
| `writer.set_bool(key, new_checked)` | ChangeWriter | Widget **derives** key from FormBinder; **owns** emission timing |
| `form.bind(cx, \|b\| { ... })` | FormBinder | Application layer **owns** the binding pass; **consumes** all dirty slots |

No binding in this SPEC crosses an authority boundary. FormBinder is the sole intermediary between widget emission and application state consumption.

---

## 11. Binding Legality Matrix (Normative)

| Binding Pattern | Authority | Legal | Reason |
|---|---|---|---|
| Widget emits value via ChangeWriter | FormBinder | Legal | Single authority; edge-triggered |
| Application consumes value via .sync() | FormBinder | Legal | Single authority; snapshot semantics |
| Application consumes value via .take() | FormBinder | Legal | Single authority; exactly-once |
| Widget reads navigation path | NavigationHost | **Illegal** | Violates INV-207-B1; navigation state owned by NavigationHost (SPEC-HAL-247) |
| Widget reads focused surface | Focus Resolver / `InteractionDomainState::focus_manager` | **Illegal** | Violates INV-207-B1; focus state is interaction-domain authority (SPEC-HAL-235/247) |
| Widget reads compositor surface identity | Compositor | **Illegal** | Violates INV-207-B1; surface identity owned by Compositor (SPEC-HAL-247) |
| Widget reads overlay stack | OverlayDomainCoordinator plus admitted owner | **Illegal** | Violates INV-207-B1; active-stack admission and contents have explicit non-widget authorities (SPEC-HAL-236/247) |
| Binding spans two surfaces | None | **Illegal** | No single authority; violates root isolation (SPEC-HAL-237) |
| FormBinder projects navigation state | NavigationHost | **Illegal** | FormBinder owns value binding only; navigation control and runtime projections use their explicit non-FormBinder seams (SPEC-HAL-228/246/248) |
| FormBinder consumes NavigationProjection | NavigationSurfaceOrchestrator | **Illegal** | Per-context projection is consumed only by SPEC-HAL-248 runtime composition, not FormBinder |

---

## 12. Forbidden Bindings (Post-Navigation v2)

The following bindings were previously unaddressed. They are now explicitly forbidden.

- **Navigation path as a bindable value.** Navigation path is owned by NavigationHost. It is not a widget-emitted value. FormBinder must not consume it.
- **Focus state as a bindable value.** Focus-domain/target state is owned by the Focus Resolver over the interaction domain's authoritative `FocusManager`. Widgets must not observe it through FormBinder. Runtime glue reads only its opaque `FocusProjection` under SPEC-HAL-248.
- **Surface identity as a bindable value.** `CompositorSurfaceId` is allocated and owned by Compositor; lifecycle authorities may reference it without acquiring identity ownership. It is not application state. FormBinder must not intern, store, or transfer it.
- **Cross-surface value binding.** A FormBinder instance operates within a single surface context. Binding a widget in surface A to application state via a FormBinder in surface B is forbidden. Each surface owns its own binding scope.
- **Overlay state as a bindable value.** Domain-wide active-stack admission is owned by `OverlayDomainCoordinator`; stack records and surface identities remain with the admitted owner. They are not widget-emitted values.
- **Projection fields as bindable values.** `NavigationProjection`, `OverlayProjection`, `FocusProjection`, and `NavigationFrameProjection` are consumed only by SPEC-HAL-248 runtime composition. Their fields must not be re-routed through FormBinder.

---

## 13. Summary (Non-Normative)

- Widgets emit values
- Application layer consumes widget-emitted values via FormBinder
- UI execution is sacred
- Locks and UI never overlap
- Binding is explicit, external, deterministic
- Navigation, focus, overlay, and surface state are not bindable values
- Each binding names exactly one authority

This is not optional.
This is how Halcyon stays sane at scale.
