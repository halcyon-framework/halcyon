# SPEC-HAL-087: Widget Identity Model

## Status: Stage 4 Implemented (Complete)

## Problem Statement

Widget identity was non-invariant due to two separate ID systems:

1. **Tree-managed IDs** (`Node.id`) - allocated by `alloc_widget_id()` in `add_child()`
2. **Widget-internal IDs** (`widget.id()`) - allocated by `WidgetInternalId::new()` in widget constructors

The `restore_widget` function previously mutated `node.id` to match `widget.id()`, corrupting the `id_to_node` mapping. Scrollbar SceneNodes were emitting widget-internal IDs, which couldn't be resolved back through `id_to_node`.

**Result**: Hit-test produced IDs that could not be resolved, event paths were empty, scrolling/scrollbars broke.

## Target Model

### Invariants

1. **TreeWidgetId is the only ID used by engine core systems**:
   - Event routing (`EventRouter`)
   - Hit-testing (`WidgetTree::hit_test`)
   - Scene graph overlay nodes (`SceneNode::ScrollbarV/H`)
   - Pointer capture (`PointerCapture`)
   - Focus management (`FocusManager`)
   - `id_to_node` mapping

2. **Widget-internal IDs are opaque/local**:
   - They **must not** be used to look up nodes in the tree
   - They **must not** be used in SceneNodes that need resolution via `id_to_node`
   - They exist only for widget-internal bookkeeping

3. **`Node.id` is stable for the lifetime of the node**:
   - Set once in `add_child()` / `create_root()`
   - Never modified afterward
   - `restore_widget` replaces widget + rect, **never** changes `Node.id`

4. **Any API producing an engine-level TreeWidgetId gets it from the tree, not from the widget**:
   - `paint_overlay` receives `tree_widget_id` from the paint traversal
   - Scene nodes store the tree-managed ID, not `widget.id()`

### Scoped access and concrete mounts — 2026-10-29

The source visibility tightening authorized by
HAL-G1-CONCRETE-MOUNT-AUTHORITY-RESOLUTION-01 keeps WidgetTree as the owner of
WidgetNode's Box, tree ID and structural membership. Public `widget()` and
`widget_mut()` return scoped borrows for existing widget state/properties;
`id()`, relationship getters and `WidgetTree::root()` expose copied observations.
Concrete-instance substitution goes through `replace_widget_only`, not a Box,
node-field or downcast whole-value assignment.

`take_widget` and `restore_widget` are crate-internal layout ownership operations.
Restoring the exact leased Box preserves both the node's TreeWidgetId and its
concrete mount lineage. A genuine tree-owned installation/replacement gets a new
private nonwrapping mount generation qualified by tree lifetime; this is the
HAL-354 G1 mount qualification, not a second routing ID or a widget-local ID.
These source/API facts do not establish G1 executable or native conformance.

## Implementation

### Stage 1 (Implemented)

#### 1. `restore_widget` Semantics (widget_tree.rs)
```rust
pub(crate) fn restore_widget(&mut self, node_id: NodeId, widget: Box<dyn Widget>, rect: RectF) {
    if let Some(node) = self.nodes.get_mut(node_id.0).and_then(|n| n.as_mut()) {
        // SPEC-HAL-087: widget.id() often differs from node.id - this is expected
        // The tree-managed ID (node.id) is the source of truth
        node.widget = widget;
        node.rect = rect;
        // node.id is NOT modified
    }
}
```

#### 2. `paint_overlay` Receives Tree-Managed ID (widget.rs, app.rs)
```rust
// Widget trait
fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId);

// app.rs paint traversal
if let Some(node) = cx.tree.node(node_id) {
    let tree_widget_id = node.id;  // Tree-managed ID
    node.widget.paint_overlay(cx, abs_rect, tree_widget_id);
}
```

#### 3. Scrollbar Scene Nodes Use Tree IDs (scroll_area.rs)
```rust
fn paint_scrollbars(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
    // Uses tree_widget_id, NOT self.id
    cx.scene.push_scrollbar_v(abs_hit, rect, tree_widget_id);
    cx.scene.push_scrollbar_h(abs_hit, rect, tree_widget_id);
}
```

#### 4. Debug Assertions in Event Routing (event_router.rs)
```rust
// After hit_test, verify ID is resolvable
#[cfg(debug_assertions)]
if tree.find_node_by_widget_id(target_id).is_none() {
    tracing::error!(
        "SPEC-HAL-087 VIOLATION: hit_test returned TreeWidgetId({:?}) not in id_to_node!",
        target_id.0
    );
}
```

### Files Modified

| File | Changes |
|------|---------|
| `src/core/widget_tree.rs` | `restore_widget` documentation + debug assertion |
| `src/core/widget.rs` | `paint_overlay` signature with `tree_widget_id` |
| `src/core/app.rs` | Pass `node.id` to `paint_overlay` |
| `src/core/event_router.rs` | Debug assertions for resolvable IDs |
| `src/widgets/scroll_area.rs` | Use `tree_widget_id` for scene nodes |

### Stage 2 (Implemented): Enforcement Hooks

Active runtime verification that prevents incorrect TreeWidgetId usage.

#### 1. `verify_tree_widget_id()` - Single ID Verification
```rust
pub fn verify_tree_widget_id(&self, widget_id: TreeWidgetId) -> bool {
    // Checks: ID in id_to_node, node exists, node.id == widget_id
    // On failure: logs error, optionally panics in strict mode
}
```

#### 2. `verify_all_nodes_consistent()` - Full Tree Verification
```rust
pub fn verify_all_nodes_consistent(&self) {
    // Validates: unique IDs, id_to_node matches node count, no stale IDs
    // Called after build_scene() in debug builds
}
```

#### 3. Guard Injection Points
- `create_root()` / `add_child()` - after creating node
- `restore_widget()` - after replacement
- `hit_test()` - before returning HitTestResult (grid + DFS paths)
- `hit_test_in_subtree()` - before returning result
- `set_overlay_hit_regions_from_scene()` - verifies all overlay IDs
- `route_pointer_event()` / `route_scroll_event()` - before build_event_path

#### 4. Strict Mode
```bash
HALCYON_STRICT_IDENTITY=1 cargo run --example flow_grid_megatest
```
Any identity violation triggers immediate panic instead of just logging.

### Files Modified (Stage 2)

| File | Changes |
|------|---------|
| `src/core/widget_tree.rs` | `verify_tree_widget_id()`, `verify_all_nodes_consistent()`, guards |
| `src/core/event_router.rs` | Updated guards to use `verify_tree_widget_id()` |
| `src/core/app.rs` | Call `verify_all_nodes_consistent()` after `build_scene()` |

## ID System Summary (Pre-Stage 3)

| System | Source | Used By |
|--------|--------|---------|
| Tree-managed | `alloc_widget_id()` | `id_to_node`, event routing, `Node.id`, scene overlays |
| Widget-internal | `WidgetInternalId::new()` | `widget.id()`, internal widget logic only |

### Stage 3 (Implemented): Type-Safe Widget Identity

Compile-time separation of engine and widget ID types prevents accidental leakage.

#### 1. New Type Definitions (utils/id.rs)
```rust
/// Engine-managed widget identifier (used by tree, events, hit-test, scene)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeWidgetId(pub u64);

/// Widget-internal identifier (used only inside widget implementations)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetInternalId(pub u64);
```

#### 2. Widget Trait Change
```rust
// Widget::id() now returns WidgetInternalId, NOT TreeWidgetId
fn id(&self) -> WidgetInternalId;

// paint_overlay receives TreeWidgetId from the tree traversal
fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId);
```

#### 3. Engine APIs Use TreeWidgetId
- `EventContext::widget_id: TreeWidgetId`
- `EventRouter::dispatch_to_widget(widget_id: TreeWidgetId, ...)`
- `HitTestResult::widget_id: TreeWidgetId`
- `ScrollbarVNode::widget_id: TreeWidgetId`
- `SceneNode` overlay types use TreeWidgetId

#### 4. Widget Implementations Use WidgetInternalId
```rust
struct Button {
    id: WidgetInternalId,  // Widget-internal ID
    // ...
}

impl Widget for Button {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
}
```

#### 5. Compile-Time Enforcement

`TreeWidgetId` and `WidgetInternalId` are distinct Rust types. Engine APIs that use `TreeWidgetId` reject `WidgetInternalId` at compile time.

### Files Modified (Stage 3)

| File | Changes |
|------|---------|
| `src/utils/id.rs` | New `TreeWidgetId`, `WidgetInternalId` types |
| `src/core/widget/mod.rs` | `Widget::id()` returns `WidgetInternalId`, `EventContext` uses `TreeWidgetId` |
| `src/core/widget_tree.rs` | Import and use `TreeWidgetId` throughout |
| `src/core/event/mod.rs` | Use `TreeWidgetId` for capture/focus |
| `src/core/event_router.rs` | Use `TreeWidgetId` in event dispatch |
| `src/core/scene/mod.rs` | Use `TreeWidgetId` in scene nodes |
| `src/widgets/*.rs` | All widgets use `WidgetInternalId` for internal ID |

### Stage 4 (Implemented): Cleanup and Consolidation

Removal of deprecated alias and final cleanup.

#### Changes
- Removed deprecated `pub type WidgetId = TreeWidgetId` alias
- Updated all comments from "WidgetId" to explicit type names
- No more ambiguity: `TreeWidgetId` for engine, `WidgetInternalId` for widgets

#### Files Modified (Stage 4)

| File | Changes |
|------|---------|
| `src/utils/id.rs` | Removed deprecated alias, updated header to Stage 4 |
| `src/core/widget_tree.rs` | Updated comments to use TreeWidgetId |
| `src/core/event_router.rs` | Updated comments to use TreeWidgetId |
| `docs/SPECs/SPEC-HAL-087-Widget-Identity-Model.md` | Updated to Stage 4 complete |

## Final ID System

| Type | Source | Used By |
|------|--------|---------|
| `TreeWidgetId` | `alloc_widget_id()` in tree | `id_to_node`, event routing, `Node.id`, scene overlays, hit-test |
| `WidgetInternalId` | `WidgetInternalId::new()` | `widget.id()`, internal widget logic only |

Types are intentionally incompatible to catch misuse at compile time.
No deprecated aliases remain - the identity model is complete.
