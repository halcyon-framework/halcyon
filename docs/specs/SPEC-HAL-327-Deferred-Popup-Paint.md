# RFC: Deferred Popup Paint

### *Popup Content Rendered Outside All Clip Scopes*

**Extends:** SPEC-HAL-127 (Overlay Stack), SPEC-HAL-081C (Overlay Surface)
**Affects:** ComboBox, DatePickerField, ColorPickerField, Widget trait, paint_tree

---

## 1. Problem

`paint_overlay` runs inside paint_node (line 2457 in app.rs).
It executes after THIS node's clips end but INSIDE parent
clips. When a popup widget lives inside a ScrollArea, its
popup content is clipped to the ScrollArea's viewport.

This is architecturally wrong. Popups must render above ALL
content, outside ALL clip scopes — same as drag proxy (line
2118) and tooltip (line 2136), which paint AFTER paint_node
returns.

## 2. Existing Patterns

Two systems already paint at the correct level:

### Drag Proxy (line 2118-2134)
```rust
paint_node(root, &mut cx, ...);  // main traversal

// AFTER traversal — no clip scope active
if let Some(drag) = active_drag {
    node.widget.paint_drag_proxy(&mut cx, ghost_rect, ...);
}
```

### Tooltip (line 2136-2143)
```rust
// AFTER traversal — no clip scope active
if let Some(tooltip) = tooltip_render {
    TooltipSurface::paint_standalone(&mut cx, ...);
}
```

Both paint at top level, outside all clips. Popup content
must follow the same pattern.

## 3. Design

### 3.1 New Widget Trait Method

```rust
/// Paint popup content (dropdowns, date pickers, color
/// pickers). Called in a deferred pass OUTSIDE all clip
/// scopes, after the main paint traversal completes.
///
/// Default: no-op.
///
/// Unlike paint_overlay (which runs inside parent clips),
/// paint_popup runs at the top level with no clip stack.
/// Popup content is never clipped by parent ScrollAreas.
fn paint_popup(
    &self,
    _cx: &mut PaintContext,
    _rect: RectF,
    _tree_widget_id: TreeWidgetId,
) {}
```

### 3.2 Collection During Paint Traversal

In paint_node (app.rs line 2453-2458), REPLACE the current
paint_overlay call with conditional collection:

```rust
// paint_overlay for inline overlays (scrollbars, etc.)
if let Some(node) = cx.tree.node(node_id) {
    let tree_widget_id = node.id;
    node.widget.paint_overlay(cx, abs_rect, tree_widget_id);
}

// Collect popup nodes for deferred rendering
if let Some(node) = cx.tree.node(node_id) {
    if node.widget.has_popup() {
        deferred_popup_nodes.push((node_id, abs_rect, node.id));
    }
}
```

`has_popup() -> bool` is a new Widget trait method (default:
false). ComboBox, DatePickerField, ColorPickerField return
true when their popup is open.

### 3.3 Deferred Paint Pass

In paint_tree (app.rs), AFTER paint_node returns:

```rust
paint_node(root, &mut cx, ...);

// Deferred popup paint — OUTSIDE all clip scopes
for (node_id, rect, widget_id) in deferred_popup_nodes.drain(..) {
    if let Some(node) = cx.tree.node(node_id) {
        node.widget.paint_popup(&mut cx, rect, widget_id);
    }
}

// Drag proxy (existing)
if let Some(drag) = active_drag { ... }

// Tooltip (existing)
if let Some(tooltip) = tooltip_render { ... }
```

Order: main tree → popups → drag proxy → tooltip.
Each layer paints above the previous.

### 3.4 Collection Mechanism

`deferred_popup_nodes` is a `Vec<(NodeId, RectF, TreeWidgetId)>`
created before paint_node and passed through (either as a
local variable in paint_tree or as a field on PaintContext).

If on PaintContext:
```rust
pub(crate) deferred_popup_nodes: Vec<(NodeId, RectF, TreeWidgetId)>,
```

Widgets call in paint_overlay (or a new registration method):
```rust
cx.register_deferred_popup(node_id, rect, tree_widget_id);
```

### 3.5 Widget Migration

#### ComboBox
- Move popup rendering from `paint_overlay` to `paint_popup`
- `paint_overlay` becomes empty (or keeps non-popup overlay
  content if any)
- `has_popup()` returns `self.popup_open.get()`

#### DatePickerField
- Move popup rendering from `paint_overlay` to `paint_popup`
- `has_popup()` returns `self.popup_open`

#### ColorPickerField
- Move popup rendering from `paint_overlay` to `paint_popup`
- `has_popup()` returns `self.popup_open`

All three use `compute_popup_rect` from `src/core/popup.rs`
for viewport-aware placement. The viewport dimensions come
from `cx.viewport_size()` (root rect), which is correct
at the top level.

### 3.6 paint_overlay Preserved

`paint_overlay` is NOT removed. It continues to serve
inline overlay content (ScrollArea scrollbars, column
resize handles, etc.) that SHOULD be clipped by parent
viewports.

The split:
- `paint_overlay` — inline overlays, clipped by parents
- `paint_popup` — popup overlays, NEVER clipped

## 4. Invariants

**INV-POPUP-1**: Popup content MUST be painted after
paint_node(root) returns, outside all clip scopes.

**INV-POPUP-2**: Popup content MUST use LAYER_OVERLAY
z-indices for correct rendering above all content.

**INV-POPUP-3**: `has_popup()` MUST return false when
no popup is open. Deferred collection MUST NOT include
widgets without open popups.

**INV-POPUP-4**: Popup paint order follows tree traversal
order (first registered = first painted = lowest z). If
multiple popups are open, later ones paint on top.

**INV-POPUP-5**: `paint_popup` receives the SAME `rect`
(absolute paint rect) as `paint_overlay` would. The
widget is responsible for computing popup position via
`compute_popup_rect`.

## 5. Forbidden

**FORBID-POPUP-1**: Popup content MUST NOT be rendered
inside paint_overlay. All popup rendering goes through
paint_popup (deferred pass).

**FORBID-POPUP-2**: paint_popup MUST NOT be called
inside paint_node. Only from the deferred pass in
paint_tree.

## 6. Migration Steps

Phase 1: Add `has_popup()` and `paint_popup()` to Widget
trait (both default no-op/false). Add deferred collection
to paint_node and deferred pass to paint_tree.

Phase 2: Migrate ComboBox popup rendering from paint_overlay
to paint_popup. Verify dropdown appears correctly near
window edges.

Phase 3: Migrate DatePickerField and ColorPickerField.
Verify calendar and color picker popups appear correctly
near all window edges.

Phase 4: Remove the old popup rendering code from
paint_overlay in all three widgets.
