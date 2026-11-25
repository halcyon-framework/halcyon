# **SPEC-HAL-037 — Persistent Widget Identity & Lifecycle Model**

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Category:** Core / Architecture
**Target:** `core::widget`, `core::widget_tree`, `core::event_router`, `tests/`

---

## 1. Purpose

This SPEC formalizes **widget identity** and **lifecycle** in Halcyon.

We already have:

* a retained `WidgetTree`,
* a `Widget` trait,
* `WidgetId` / `NodeId` concepts in practice,
* diffing/caching plans (SPEC-HAL-015, 026, 035),
* upcoming focus & input hierarchy (SPEC-HAL-038).

What’s missing is a **clear contract**:

* what exactly is a widget’s identity?
* when is it born / alive / dead?
* how long do we keep its state?
* how does this interact with diffing, focus, caching?

This SPEC creates that contract.

Goal:

> A widget’s state, focus, and relationships stay attached to **that widget**,
> even when the UI restructures, reorders, or reflows.

---

## 2. Problems Without a Model

Without a formal identity model, we get:

1. **State teleporting**

    * reorder children → internal state jumps to wrong row/button
    * text fields suddenly swap contents

2. **Focus glitches**

    * focused widget removed/replaced → stale focus reference
    * keyboard events go to a dead widget

3. **Broken diffing**

    * SPEC-HAL-035 (Widget Diff Optimizer) can't know if:

        * a node moved
        * a node was removed & re-added
    * leads to unnecessary rebuilds and bad perf

4. **Resource leaks**

    * no clear point to release:

        * subscriptions
        * GPU resources
        * timers

5. **Testing hell**

    * impossible to reason about “this widget instance” in tests
    * flakier tests due to implicit identity semantics

---

## 3. Identity Model Overview

We distinguish between:

* **NodeId** — the **position** in the widget tree (parent/children links)
* **WidgetId** — the **logical identity** of a widget instance

### 3.1. NodeId

* Represents a **slot in the tree**
* Changes when:

    * widget is reparented
    * widget is removed / reinserted
* Used for:

    * traversal
    * layout
    * hit-testing

### 3.2. WidgetId

* `struct WidgetId(u64);`
* **Globally unique** within a `WidgetTree` lifetime
* Stable as long as the widget exists (from mount to unmount)
* Never reused during the lifetime of a `WidgetTree`
* Used for:

    * state storage keyed by widget
    * focus model (SPEC-HAL-038)
    * diffing / caching (SPEC-HAL-015, 026, 035)
    * external references (e.g. “scroll this widget into view”)

**Rule:**

> NodeId is *where*, WidgetId is *who*.

---

## 4. Lifecycle States

Each widget instance (identified by `WidgetId`) has a lifecycle:

1. **Created**

    * `Widget` object constructed (e.g. `Button::new()`)
    * Not yet part of `WidgetTree`
    * No events, no layout, no paint

2. **Mounted**

    * Node created in `WidgetTree`
    * A `WidgetId` assigned
    * `on_mount()` called once

3. **Active**

    * Participates in:

        * layout
        * paint
        * event routing
    * This is the normal state between mount and unmount

4. **Suspended (off-screen / detached)** — optional future extension

    * Not currently visible or part of layout, but **state kept**
    * For tab panes, hidden panels

5. **Unmounted**

    * Widget removed from `WidgetTree`
    * `on_unmount()` called once
    * `WidgetId` is retired (never reused)
    * All associated state (focus, caches, subscriptions) must be cleaned

For now we implement 1, 2, 3, 5 explicitly.
Suspended can be introduced later if/when docking/tab system needs it.

---

## 5. Widget Trait Contract

### 5.1. ID Access

The widget itself is usually not responsible for its own ID; the tree is.

But we expose:

```rust
trait Widget {
    // existing methods...

    /// Return this widget’s stable identity.
    fn id(&self) -> WidgetId;
}
```

The default implementation may be provided by a wrapper / base struct; individual widgets don’t manually manage IDs.

### 5.2. Lifecycle Hooks

We standardize two hooks (one already exists in the code):

```rust
trait Widget {
    // Called once when the widget enters the tree with a fresh WidgetId
    fn on_mount(&mut self, _cx: &mut MountContext) {}

    // Called once when the widget is removed from the tree
    fn on_unmount(&mut self) {}
}
```

**Rules:**

* `on_mount()` is called **after** the node (and ID) has been allocated.
* `on_unmount()` is called **before** the Widget is dropped / removed.
* Both are **at-most-once** per widget instance.

Widgets may use these for:

* subscribing/unsubscribing to external signals
* registering themselves in app state
* initializing focus
* registering hotkeys

---

## 6. WidgetTree Responsibilities

`WidgetTree` becomes the **authority** on identity and lifecycle.

### 6.1. ID Generation

* Use a `WidgetIdGenerator` in `utils::id`:

  ```rust
  struct WidgetIdGenerator {
      next: AtomicU64,
  }
  ```
* `next_id()` returns a fresh `WidgetId`.
* No reuse inside a tree’s lifetime.

### 6.2. Node Creation

On `create_root()` / `add_child()`:

1. Allocate a new `WidgetId`
2. Bind it to the node (NodeId + Widget)
3. Call `widget.on_mount(&mut MountContext { widget_id, tree, ... })`

### 6.3. Node Removal

On remove/detach:

1. Retrieve widget
2. Call `widget.on_unmount()`
3. Remove node and all child nodes (recursive)
4. Retire all involved `WidgetId`s

### 6.4. Reparent / Reorder

When a node is **moved** (not torn down):

* `WidgetId` must be preserved.
* IDs of siblings stay unchanged.
* Lifecycle hooks **must not** be called again.

> Rule:
> **Moving or reordering does not change identity or lifecycle.**
> Only creation and destruction do.

---

## 7. Interaction with Other SPECs

### 7.1. SPEC-HAL-015 (Caching & Diffing)

Widget-level diffing relies on stable identity:

* If `WidgetId` is the same:

    * diff state, reuse caches, reuse subtrees
* If `WidgetId` is new:

    * treat as new node, rebuild layout/paint caches

`WidgetId` becomes the primary key for:

* dirty flags
* layout invalidation records
* diff results

### 7.2. SPEC-HAL-038 (Focus & Input Routing)

Focus model stores a `WidgetId` as:

* currently focused widget
* focus chain (for tab traversal)
* focus scopes (modals, panels, dock areas)

On `on_unmount()`, focus system must:

* check if focused widget is being removed
* move focus to:

    * nearest ancestor
    * or next focusable sibling
    * or clear focus

### 7.3. Clipboard / DragDrop / Actions (019, 020)

Any long-lived reference to UI elements must use `WidgetId`, not `NodeId`.

Examples:

* “Scroll this widget into view”
* “Trigger action for widget X”

---

## 8. Keyed Children (Dynamic Collections)

Dynamic containers (FlowGrid, dynamic FlexColumn, virtualized lists) require **stable identity across rebuilds**.

The rule:

* Caller is responsible for providing stable **keys** for logical items.
* Halcyon maps **keys → WidgetId**.

Example API (conceptual):

```rust
FlexColumn::new_keyed(items, |item| item.key, |item| {
    TextBlock::new(item.text.clone())
})
```

Internal mapping:

* For a given key:

    * if existing `WidgetId` present → reuse
    * else → allocate new `WidgetId` and mount widget
* When keys disappear → unmount corresponding widgets

This prevents:

* list reordering from swapping internal state
* text fields swapping their content
* checkboxes “moving” their tick

---

## 9. Testing & Debugging

For tests:

* `WidgetId` can be printed/logged:

  ```rust
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
  pub struct WidgetId(pub u64);
  ```
* Test helpers:

    * “find widget by id”
    * “assert widget with id X was unmounted”
    * “expect focus on widget Y after event sequence”

Debugging hooks:

* Tree dump shows `(NodeId, WidgetId, type_name)` per node.

---

## 10. Implementation Checklist (for Claude Code)

**No code here, only steps.**

### 10.1. ID infrastructure

1. In `src/utils/id.rs`:

    * Ensure `WidgetId` is `u64`-backed, `Copy + Eq + Hash + Debug`.
    * Implement `WidgetIdGenerator` with `next_id()`.

2. In `src/core/widget.rs`:

    * Add `fn id(&self) -> WidgetId;` to `Widget` trait OR ensure a base type stores it.
    * Ensure `on_mount()` and `on_unmount()` hooks exist and match this SPEC.

### 10.2. WidgetTree wiring

3. In `src/core/widget_tree.rs`:

    * Store `WidgetId` per node, apart from `NodeId`.
    * On `create_root()` / `add_child()`:

        * allocate `WidgetId` via generator,
        * attach to node,
        * call `widget.on_mount()`.

4. On node removal:

    * Ensure `on_unmount()` is called exactly once.
    * Apply recursively to children.

5. For any “move” or “reparent” operation (if present or added later):

    * ensure it **does not** create a new `WidgetId`.
    * do **not** call lifecycle hooks again.

### 10.3. Router & Focus Integration (future-aware)

6. In `src/core/event_router.rs`:

    * Use `WidgetId` where “logical target widget” is tracked.
    * Keep `NodeId` for geometric traversal / hit-testing only.

7. (When SPEC-HAL-038 is implemented)

    * Focus registry keyed by `WidgetId`.
    * On `on_unmount()`, clear focus if it points at that widget.

### 10.4. Tests

8. Add tests in `tests/tier1_widget_tests.rs` (or new file):

    * Create a small tree, grab `WidgetId`s, reorder nodes:

        * assert IDs are stable.
    * Remove a node:

        * assert `on_unmount()` was called once.
    * Replace a widget with a new one:

        * assert `WidgetId` changes.

---

## 11. Layout Dirty Invariants (Geometry-Relevant State)

**Added**: 2025-12-23 (virtualization black gap fix)

This section formalizes when widgets MUST mark themselves layout-dirty.

### 11.1 Core Invariant

**INV-LIFECYCLE-DIRTY-1 — Mandatory Dirty Marking on Geometry State Change**

If a widget mutates internal state that affects child geometry, it MUST be
marked layout-dirty in the same frame.

This applies to:
- `List.visible_range` (virtualization)
- Any widget performing realization windows
- Any widget with geometry-relevant computed state

### 11.2 Forbidden Patterns

The following behaviors are forbidden:

- **INV-LIFECYCLE-DIRTY-F1**: Updating geometry-relevant state without triggering layout
- **INV-LIFECYCLE-DIRTY-F2**: Relying on paint-time transforms to compensate for stale layout
- **INV-LIFECYCLE-DIRTY-F3**: Caching layout results when geometry inputs changed

### 11.3 Enforcement

For virtualized widgets:
- `update_visible_range_from_scroll()` MUST return a change flag
- Caller (engine) MUST call `tree.mark_layout_dirty(widget_id)` when changed

This ensures:
- Layout runs with current geometry state
- Children are positioned correctly BEFORE paint transforms are applied
- No 1-frame lag between state change and layout update

See: SPEC-HAL-134 Section 21.2, SPEC-HAL-131A Section L.2

### 11.4 Layout Epoch Lifecycle (Added 2025-12-23)

**INV-LIFECYCLE-EPOCH-1 — Layout Epoch Stamp**

Every node that is laid out in a frame MUST have its `layout_epoch` stamped
with the current epoch value.

Implementation:
- `layout_epoch: u64` field on `WidgetNode`
- Stamped in `layout_node()` (app.rs), `layout_child()`, `layout_child_full()`,
  `relayout_child_with_size()`, `relayout_child_with_constraints()` (widget.rs)
- Also in `List::layout()` for direct child layout (list.rs)

**INV-LIFECYCLE-EPOCH-2 — Paint Requires Current Epoch**

A node may not be painted unless `node.layout_epoch == current_layout_epoch()`.

- Enforced by assertion in `paint_node()` (debug builds)
- Violation indicates paint-without-layout bug

**INV-LIFECYCLE-EPOCH-3 — Direct Layout Path**

Widgets that perform direct `child_widget.layout()` calls (instead of using
`cx.layout_child()`) MUST manually update the child's cache state:

```rust
if let Some(node) = cx.tree.node_mut(child_id) {
    node.cached_size = result.size;
    node.last_constraints = Some(constraints);
    node.dirty_layout = false;
    #[cfg(debug_assertions)]
    node.layout_epoch = current_layout_epoch();
}
```

This applies to:
- `List::layout()` — uses direct layout for custom child positioning

See: SPEC-HAL-042 Section 11.4, SPEC-HAL-134 Section 21.5

---

## 12. Summary

This SPEC locks in a **clear, mechanical contract**:

* NodeId = position
* WidgetId = identity
* on_mount/on_unmount = single-entry, single-exit of lifecycle

It underpins:

* diffing (109),
* caching (100, 015),
* focus (112),
* complex dynamic lists,
* stable trading UI behavior under heavy reconfiguration.
