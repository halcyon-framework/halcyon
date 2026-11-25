# SPEC-HAL-270: **Notification Log System**

### *Persistent In-Process Notification Log*

**Author:** JC
**Status:** Implemented
**Depends on:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-001 (Core Architecture)
- SPEC-HAL-269 / 269A (Toast Notification System — ephemeral layer)

---

# 1. Purpose

This SPEC defines the **persistent notification log** subsystem of Halcyon.

It documents:

* The distinction between ephemeral toasts (SPEC-HAL-269) and the persistent log
* The model types and ownership contract
* The ring buffer capacity and eviction policy
* The `AppContext` and `ScreenContext` API surface
* Invariants the system must uphold

---

# 2. Motivation and Scope

SPEC-HAL-269 defines toasts: ephemeral, auto-dismissing overlays for in-the-moment feedback. Toasts are not recorded. Once dismissed, they are gone.

Applications frequently need a complementary **persistent record** — a log that:

* survives toast dismissal
* is user-accessible at any time via an embedded panel
* does not require the user to have been watching the screen at the moment of the event
* carries semantic severity levels (Info, Warning, Error)
* supports per-entry actions (e.g., "Retry", "View details")

SPEC-HAL-270 defines this second layer. The two systems are **independent**: `push_toast` writes only to the ephemeral queue; `push_notification` writes only to the persistent log. Callers choose explicitly. There is no automatic mirroring between the two layers unless the caller invokes both.

---

# 3. Architecture

## 3.1 Layer Separation

| Property | Toast (SPEC-HAL-269) | Notification Log (SPEC-HAL-270) |
|---|---|---|
| Lifetime | Ephemeral — auto-dismissed | Persistent — explicit remove or eviction |
| Storage | `Vec` queue, flushed each frame | Ring buffer, fixed capacity |
| UI | Overlay widget (`ToastWidget`) | Embeddable panel (`NotificationLogWidget`) |
| Ownership | `ToastManager` in `AppContext` | `NotificationManager` in `AppContext` |
| API | `push_toast(...)` | `push_notification(...)` |

## 3.2 Ownership

`NotificationManager` is owned by `AppContext` as `Rc<RefCell<NotificationManager>>` and exposed on `ScreenContext` via `push_notification(...)` and `notification_manager()` accessor.

Ownership invariants:

**INV-NL-OWNER**: `AppContext` is the sole owner of `NotificationManager`. No widget, screen, or external module may own or store a reference to it across frames.

**INV-NL-BORROW**: All mutations (`push`, `remove`, `clear`) occur through `AppContext::push_notification` or via the `borrow_mut()` accessor during event flush (Phase 2). No mutation during layout or paint.

---

# 4. Types

## 4.1 `NotificationId`

```rust
pub struct NotificationId(pub u64);
```

Stable, monotonically increasing, unique within a process lifetime. Never reused.

## 4.2 `NotificationLevel`

```rust
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}
```

Semantic severity. Drives visual treatment in `NotificationLogWidget` (accent color, icon).

## 4.3 `NotificationEntry`

```rust
pub struct NotificationEntry {
    pub id: NotificationId,
    pub level: NotificationLevel,
    pub title: Option<String>,
    pub message: String,
    pub timestamp_ms: u64,
    pub action: Option<NotificationAction>,
}
```

Entries are cloned into the ring buffer on push. They are immutable after insertion.

## 4.4 `NotificationAction`

```rust
pub struct NotificationAction {
    pub label: String,
}
```

Optional single action per entry. The widget is responsible for routing action invocation back to the screen via FormBinder or equivalent.

## 4.5 `NotificationHandle`

```rust
pub struct NotificationHandle {
    pub id: NotificationId,
}
```

Returned from `push_notification`. Callers may retain the handle to later call `remove(id)` by id if needed.

---

# 5. NotificationManager

## 5.1 Storage

Backed by `crate::core::series::Series<NotificationEntry>`, which is a semantic wrapper around `SlidingWindow<T>` — a fixed-capacity ring buffer.

**INV-NL-CAP**: The log has a fixed capacity set at construction. Default is 256. When the buffer is full, the oldest entry is silently evicted on `push`. No error, no panic.

**INV-NL-ORDER**: `iter()` yields entries newest-first. All consumers must treat the first element as the most recent.

## 5.2 API

```rust
impl NotificationManager {
    pub fn new() -> Self
    pub fn with_capacity(capacity: usize) -> Self

    pub fn push(
        &mut self,
        level: NotificationLevel,
        message: impl Into<String>,
        title: Option<impl Into<String>>,
        action: Option<NotificationAction>,
        timestamp_ms: u64,
    ) -> NotificationHandle

    pub fn remove(&mut self, id: NotificationId)
    pub fn clear(&mut self)

    pub fn iter(&self) -> impl Iterator<Item = &NotificationEntry>
    pub fn len(&self) -> usize
    pub fn is_empty(&self) -> bool
    pub fn capacity(&self) -> usize
}
```

`remove()` rebuilds the ring buffer without the targeted entry. This is acceptable because removal is a low-frequency user-initiated action.

## 5.3 Timestamp contract

Timestamps (`timestamp_ms`) are provided by the caller using wall-clock time (`SystemTime::now()` as `u64` milliseconds since UNIX epoch). The `NotificationManager` does not own a clock — it accepts and stores whatever timestamp is given.

**INV-NL-TIMESTAMP**: Callers are responsible for providing monotonically non-decreasing timestamps within a single session. The manager imposes no ordering constraint on storage — it stores in push order.

---

# 6. AppContext Integration

## 6.1 Field

```rust
pub notification_manager: Rc<RefCell<NotificationManager>>,
```

Initialized in `AppContext::new()` with `NotificationManager::new()`.

## 6.2 AppContext method

```rust
pub fn push_notification(
    &mut self,
    level: NotificationLevel,
    message: impl Into<String>,
    title: Option<impl Into<String>>,
    action: Option<NotificationAction>,
    timestamp_ms: u64,
) -> NotificationHandle
```

## 6.3 ScreenContext methods

```rust
pub fn push_notification(
    &mut self,
    level: NotificationLevel,
    message: impl Into<String>,
    title: Option<impl Into<String>>,
    action: Option<NotificationAction>,
    timestamp_ms: u64,
) -> NotificationHandle

pub fn notification_manager(&self) -> std::cell::Ref<'_, NotificationManager>
```

`notification_manager()` provides read-only access for building notification log panels from `build()`.

---

# 7. Constitutional Compliance

### Pillar A — Retained-Mode Ownership

`NotificationManager` is owned by `AppContext`. Entries are value types (`Clone`). No widget holds a reference to the manager or any entry across frames.

### Pillar B — Phase Separation

All mutations (`push`, `remove`, `clear`) occur in Phase 2 (Deferred Mutations / flush_pending_actions). The `iter()` accessor used during `build()` is read-only and produces no side effects.

### Pillar C — Identity Through ID

Each entry is identified by `NotificationId(u64)`. Widgets reference entries by id, never by pointer or index.

### Pillar D — Pure Data In, Explicit Actions Out

`NotificationEntry` is pure data. Widget-level actions (dismiss, invoke action) are communicated back through FormBinder bindings, not direct callbacks into the manager.

### Pillar E — GPU-First Rendering

`NotificationLogWidget` is an embeddable retained-mode widget. It reads the log snapshot during `build()` and constructs a deterministic subtree. No mutation during layout or paint.

---

# 8. Invariant Summary

| ID | Statement |
|---|---|
| INV-NL-OWNER | AppContext is the sole owner of NotificationManager |
| INV-NL-BORROW | Mutations only in Phase 2; read-only during build/layout/paint |
| INV-NL-CAP | Fixed capacity; oldest entry evicted silently when full |
| INV-NL-ORDER | iter() yields newest-first |
| INV-NL-TIMESTAMP | Caller provides timestamps; manager imposes no clock |
| INV-NL-IMMUTABLE | Entries are immutable after insertion |
| INV-NL-SEPARATE | No automatic mirroring between toast layer and notification log |

---

# 9. Future Considerations

* **Unread count / badge**: Not implemented. If added, a separate `unread_count: usize` field on `NotificationManager` would be the correct approach — not derived from entry state.
* **Persistence across process restart**: Out of scope. Log is in-process only.
* **NotificationLogWidget**: Defined separately as a regular embeddable widget. See implementation task.

---

# 10. Files

| File | Role |
|---|---|
| `src/model/notification.rs` | Types and NotificationManager |
| `src/core/series.rs` | Ring buffer semantic wrapper |
| `src/core/sliding_window.rs` | Raw ring buffer |
| `src/core/app.rs` | AppContext/ScreenContext integration |
| `examples/notification_demo.rs` | Reference demo |
