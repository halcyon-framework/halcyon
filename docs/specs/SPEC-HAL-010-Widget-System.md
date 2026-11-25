**SPEC-HAL-010: Halcyon — Widget System & Composition Model**

### **IMPORTANT: ScrollArea is a fixed-width container.**

It fills all available cross-axis space provided by its parent.
Children never influence ScrollArea's width.
Scrollbars always align to ScrollArea.rect.

---

**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** Widget identity, tree structure, composition patterns, lifecycle, and how all of this ties into layout, events, and scene building.
**Primary Use-Case:** Trading-grade UIs with complex composite widgets (charts, ladders, blotters, dashboards).

---

## 1. Purpose

This SPEC defines **how widgets actually exist** in Halcyon:

* how they are **identified**
* how they are **stored**
* how they **own children**
* how they participate in **layout, event routing, and painting**
* how they support **dynamic structures** like lists, tables, and order books.

Everything else (layout, event routing, state model) already assumes a tree of widgets.
This SPEC says what that tree actually is.

---

## 2. Requirements

The widget system must:

1. Be **retained-mode**:

    * Widgets are real objects that live across frames.
    * No egui-style reallocation per frame.

2. Provide **stable identity**:

    * Every widget has a stable ID used for:

        * events,
        * focus,
        * selection,
        * internal maps (e.g. by WidgetId → per-widget state).

3. Support **composition**:

    * Composite widgets that own children:

        * panels,
        * rows/columns,
        * complex controls like ladder, chart, blotter.

4. Support **dynamic children**:

    * Lists, trees, tables where rows/entries appear/disappear/reorder.
    * Must not randomly drop or corrupt per-row state (scroll position, selection, etc.).

5. Integrate with:

    * **Layout** (SPEC-HAL-008),
    * **Event Routing** (SPEC-HAL-009),
    * **Scene Graph** (SPEC-HAL-005),
    * **State Model** (SPEC-HAL-007).

6. Be **predictable** and **explicit**:

    * No hidden diff engines.
    * No magic binding graphs.
    * Developer controls where/when widgets are created/destroyed.

7. Be **fast enough** for:

    * thousands of live widgets,
    * frequently updated lists (order book, trades list),
    * real-time chart overlays.

---

## 3. Widget Identity

We distinguish three related concepts:

1. **WidgetId** — runtime identity (internal, unique per widget instance).
2. **WidgetKey** — *semantic* identity for dynamic lists.
3. **WidgetPath** — optional debugging/diagnostics path (for logging, not routing).

### 3.1 WidgetId

Already defined in skeleton:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u32);
```

**Rules:**

* Assigned by the **widget tree runtime**, not by the app.
* Unique within a single `WidgetTree` instance.
* Stable for the lifetime of the widget.
* Used for:

    * event routing,
    * focus manager,
    * pointer capture,
    * selection,
    * internal maps.

ID generation:

* Monotonic counter (`u32` or `u64`).
* No reuse until wrap-around (not a real constraint in practice).

### 3.2 WidgetKey (for lists)

Application-provided semantic identity for **logical items** in dynamic structures.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidgetKey {
    U64(u64),
    String(String),
    // could add more forms later.
}
```

**Use:**

* When building children for list-like widgets, app or widget may supply a `WidgetKey`.
* The widget tree uses the key to:

    * match existing child widgets to new children,
    * reuse widget objects (and their state) between frames,
    * avoid reset of per-item state when items reorder.

Example:

* Order book row for price `42000.5` gets key `WidgetKey::U64(price_in_ticks)`.
* When the book reorders, that row’s widget can be reused with its state preserved.

### 3.3 WidgetPath (optional diagnostics)

Optional debugging info:

```rust
pub struct WidgetPath {
    pub segments: Vec<WidgetPathSegment>,
}

pub struct WidgetPathSegment {
    pub name: &'static str,
    pub index: Option<usize>,
}
```

Used only for logs, debug, assertions, not for routing.
Not required for core functionality.

---

## 4. WidgetNode & WidgetTree

The tree itself is explicit.

### 4.1 WidgetNode

Conceptual structure:

```rust
pub struct WidgetNode {
    pub id: WidgetId,
    pub key: Option<WidgetKey>,

    pub widget: Box<dyn Widget>,

    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub next_sibling: Option<NodeId>,

    // layout result (root-space rect).
    pub rect: Rect,

    // z-order within parent (lower first => painted earlier).
    pub z_order: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

Notes:

* `NodeId` is an index into an internal Vec/arena of nodes.
* `WidgetId` is the logical identity exposed to rest of system; it may equal `NodeId` or be separate. Runtime detail.

### 4.2 WidgetTree

Central structure for runtime:

```rust
pub struct WidgetTree {
    pub root: NodeId,
    // Node storage: tight array / arena
    nodes: Vec<WidgetNode>,
    // Mapping from WidgetId → NodeId (for focus, capture, direct lookup)
    id_to_node: FxHashMap<WidgetId, NodeId>,
    next_widget_id: u32,
    next_node_id: u32,
}
```

**Responsibilities:**

* Own all WidgetNode instances.
* Allocate new nodes,
* Free nodes,
* Maintain parent/child/sibling relations,
* Provide traversal methods (for layout, painting, hit-testing, event routing).

Key methods (conceptual):

```rust
impl WidgetTree {
    pub fn new_root(widget: Box<dyn Widget>) -> Self { /* ... */ }

    pub fn add_child(
        &mut self,
        parent: NodeId,
        key: Option<WidgetKey>,
        widget: Box<dyn Widget>,
        z_order: i16,
    ) -> NodeId { /* ... */ }

    pub fn remove_node(&mut self, node: NodeId) { /* ... */ }

    pub fn node(&self, node: NodeId) -> &WidgetNode { /* ... */ }
    pub fn node_mut(&mut self, node: NodeId) -> &mut WidgetNode { /* ... */ }

    pub fn find_by_id(&self, id: WidgetId) -> Option<NodeId> { /* ... */ }

    pub fn children(&self, parent: NodeId) -> ChildIter<'_> { /* ... */ }
}
```

---

## 5. Widget Trait Refinement

Existing trait (from SPEC-002 / SPEC-004) becomes:

```rust
pub trait Widget {
    /// Called during layout pass:
    /// - constraints: bounds we must fit into.
    /// - cx: layout context (theme, text metrics, etc.).
    ///
    /// Implementation must:
    /// - set its own desired Size
    /// - perform layout on children (if any) and set their rects.
    fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints) -> Size;

    /// Called during scene building:
    /// - builder: scene builder that adds primitives.
    ///
    /// Implementation:
    /// - adds primitives for self,
    /// - recursively asks children to paint.
    fn paint(&self, builder: &mut SceneBuilder);

    /// Handle events for this widget.
    /// Return true if handled and should stop propagation.
    fn handle_event(&mut self, event: &Event, phase: EventPhase) -> bool;

    /// Optional: called once when widget node is first inserted into the tree.
    fn on_mount(&mut self) {}

    /// Optional: called once when widget node is removed from the tree.
    fn on_unmount(&mut self) {}
}
```

**Important:**

* Children are not exposed via trait.
* Composition is done by widgets using the `WidgetTree` API in their own constructors/builders.
* Layout and paint methods *assume* that children are already present in the tree, referenced via internal handles (NodeId / WidgetId) held by the composite widget struct.

---

## 6. Composition Patterns

There are two main composition patterns:

1. **Static composition** — child structure fixed at construction.
2. **Dynamic composition** — children change over time (lists, trees).

### 6.1 Static Composition

Example: `Panel` with a header and a body.

```rust
pub struct Panel {
    pub header_node: NodeId,
    pub body_node: NodeId,
}

impl Panel {
    pub fn new(tree: &mut WidgetTree, parent: NodeId, header: Box<dyn Widget>, body: Box<dyn Widget>) -> NodeId {
        let panel_node = tree.add_child(parent, None, Box::new(Self {
            header_node: NodeId(0), // temporary
            body_node: NodeId(0),
        }), 0);

        // Now add children under `panel_node`:
        let header_node = tree.add_child(panel_node, None, header, 0);
        let body_node = tree.add_child(panel_node, None, body, 1);

        // Fix up the Panel struct inside the panel_node.widget
        if let Some(panel_widget) = tree.node_mut(panel_node).widget.as_any_mut().downcast_mut::<Panel>() {
            panel_widget.header_node = header_node;
            panel_widget.body_node = body_node;
        }

        panel_node
    }
}
```

(The exact downcast/registration mechanism can be adjusted later; idea stands: composite widget knows its children via `NodeId`s.)

Panel’s `layout` implementation:

* Calls layout on `header_node` and `body_node` via tree.
* Sets their rects.
* Returns combined Size.

### 6.2 Dynamic Composition (Lists / Tables)

Lists, trees, tables need to manage children whose **existence and order** can change at runtime.

We define a helper pattern: **Keyed children**.

Conceptual helper:

```rust
pub struct DynamicChildren {
    // current children in order
    pub nodes: Vec<NodeId>,
}

impl DynamicChildren {
    /// Reconcile children based on a sequence of (key, widget_factory).
    /// - Reuse existing widgets whose keys match.
    /// - Create new for new keys.
    /// - Remove old unused keys.
    pub fn reconcile<F>(
        &mut self,
        tree: &mut WidgetTree,
        parent: NodeId,
        desired: &[(WidgetKey, F)],
    )
    where
        F: Fn() -> Box<dyn Widget>,
    {
        // Implementation detail: build map from key → existing node,
        // then create new order reusing nodes where possible.
        // Remove nodes not in desired keys.
    }
}
```

Usage in, say, `ListView`:

```rust
pub struct ListView {
    children: DynamicChildren,
}

impl ListView {
    pub fn update_items<F>(&mut self, tree: &mut WidgetTree, parent: NodeId, items: &[(WidgetKey, F)])
    where
        F: Fn() -> Box<dyn Widget>,
    {
        self.children.reconcile(tree, parent, items);
    }
}
```

This ensures:

* stable WidgetIds for reused items,
* stable per-row state (selection, scroll offsets, etc.),
* clean removal of no-longer-needed widgets.

Critical for:

* live order book levels,
* trades table,
* symbol list.

---

## 7. Lifecycle

Two main lifecycle hooks:

1. **on_mount** — widget was inserted into tree.
2. **on_unmount** — widget is about to be removed.

These can be used by widgets to:

* allocate external resources,
* register with global services,
* detach from them on removal.

Runtime calls:

```rust
impl WidgetTree {
    pub fn add_child(...) -> NodeId {
        // ...
        node.widget.on_mount();
        // ...
    }

    pub fn remove_node(&mut self, node_id: NodeId) {
        let node = &mut self.nodes[node_id.0 as usize];
        node.widget.on_unmount();
        // now remove node and children
    }
}
```

There is **no “update props” hook** at trait level; props are updated by app code or by composite widget code directly on the struct fields.

---

## 8. Layout + WidgetTree Integration

Layout pass (from SPEC-HAL-008) is refined:

* It operates **top-down** using `WidgetTree`.

Pseudo-code:

```rust
fn layout_tree(tree: &mut WidgetTree, root: NodeId, constraints: Constraints, cx: &mut LayoutContext) -> Size {
    layout_node(tree, root, constraints, cx)
}

fn layout_node(tree: &mut WidgetTree, node_id: NodeId, constraints: Constraints, cx: &mut LayoutContext) -> Size {
    let node = tree.node_mut(node_id);

    // Ask widget to lay out itself and its children (by accessing WidgetTree via cx if needed).
    let size = node.widget.layout(cx, constraints);

    // Node rect in parent space will be set by parent; for root, we can set it directly.
    node.rect.width = size.width;
    node.rect.height = size.height;

    size
}
```

Composite widgets:

* During their `layout`, they:

    * call layout on child nodes (with appropriate constraints),
    * position children by setting their rects inside the tree.

We need to give `LayoutContext` a handle to `WidgetTree`:

```rust
pub struct LayoutContext<'a> {
    pub tree: &'a mut WidgetTree,
    pub theme: &'a Theme,
    // text measurement, dpi, etc.
}
```

---

## 9. Paint + WidgetTree Integration

Painting is **depth-first with z-order**:

```rust
fn paint_tree(tree: &WidgetTree, root: NodeId, scene: &mut Scene) {
    let mut builder = SceneBuilder::new(scene);
    paint_node(tree, root, &mut builder);
}

fn paint_node(tree: &WidgetTree, node_id: NodeId, builder: &mut SceneBuilder) {
    let node = tree.node(node_id);

    // widget paints itself using its rect (for positioning),
    // and theme etc. if needed.
    node.widget.paint(builder);

    // then children, in z-order
    let mut children: Vec<NodeId> = tree.children(node_id).collect();
    children.sort_by_key(|nid| tree.node(*nid).z_order);

    for child_id in children {
        paint_node(tree, child_id, builder);
    }
}
```

Widgets themselves typically use builder with their own rect offset.

---

## 10. Event Routing + WidgetTree (Tie-In to SPEC-HAL-009)

Event routing (from SPEC-HAL-009) uses `WidgetTree`:

* Hit-test works by walking nodes and using `node.rect`.
* Routing uses parent/child links and NodeId path:

    * capture / target / bubble phases.

This SPEC adds the concrete data structure that routing operates on.

---

## 11. Trading-Specific Widgets and Composition

### 11.1 Chart Widget as a Child

In Halcyon, the chart becomes just another `Widget` implementation:

```rust
pub struct ChartWidget {
    // internally: chart renderer handle, snapshot refs, interaction state, etc.
}

impl Widget for ChartWidget {
    fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints) -> Size {
        // Typically: take all available space (tight constraints from parent)
        Size {
            width: constraints.max_width,
            height: constraints.max_height,
        }
    }

    fn paint(&self, builder: &mut SceneBuilder) {
        // Chart-specific painting via external renderer integration.
    }

    fn handle_event(&mut self, event: &Event, phase: EventPhase) -> bool {
        // Chart-specific interactions:
        // - panning
        // - zoom
        // - selection
        // - pointer capture
        // etc.
        false
    }
}
```

Composition example:

* `RootDashboard` widget has:

    * header panel,
    * left ladder,
    * central chart,
    * right blotter.

All are children in the `WidgetTree` under a root container.

### 11.2 Order Book / Ladder

A `LadderWidget` can use `DynamicChildren` for price levels:

* Each row = child widget (row struct).
* `WidgetKey` = price level.
* Layout of rows is done inside LadderWidget.
* Events map local pointer Y to row.

---

## 12. Implementation Checklist

When you/CC implement SPEC-009:

1. **Introduce `WidgetId`, `WidgetKey`, `NodeId`, `Rect`**:

    * `WidgetId` (runtime identity),
    * `WidgetKey` for keyed children,
    * `NodeId` for internal tree index,
    * `Rect` for layout position.

2. **Implement `WidgetNode` and `WidgetTree`**:

    * node creation, deletion,
    * parent/child/sibling relationships,
    * id-to-node map.

3. **Extend `LayoutContext` to carry `&mut WidgetTree`**:

    * so composite widgets can access children.

4. **Adjust `Widget` trait**:

    * ensure `layout`/`paint`/`handle_event` signatures match this model,
    * add `on_mount`/`on_unmount`.

5. **Implement static composition helpers**:

    * e.g. `Panel::new(...)` pattern,
    * or a simple builder for adding child nodes under a parent.

6. **Implement `DynamicChildren` helper for keyed lists**:

    * reuse nodes by key,
    * remove absent keys.

7. **Wire layout and paint passes to use `WidgetTree`**:

    * top-down layout, depth-first paint.

8. **Refactor event routing (SPEC-004) to use `WidgetTree`**:

    * hit-testing via node.rect,
    * path computation via parent links.

9. **Add minimal tests**:

    * static tree composition (parent, two children),
    * dynamic children diffing with keys,
    * removal triggers `on_unmount`,
    * very basic layout recursion sanity check.

---

## **ScrollArea Contract (HAL-010 Extension)**

A ScrollArea widget must:

* ScrollArea occupies 100% of the parent's inner width.
* ScrollArea must report this width unchanged to the layout engine.
* ScrollArea computes viewport + scrollbar geometry STRICTLY from scroll_area.rect.
* No wrapper widget is allowed to shrink ScrollArea horizontally. Wrapper logic must pass full width.

Inner ScrollAreas follow the same rule: full width relative to their own parent rect.
Scrollbars anchor to THEIR rect, not the outer one.
