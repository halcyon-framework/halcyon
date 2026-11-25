# SPEC-HAL-326: Tree-Structured Focus Navigation

### *Deterministic, Explainable Keyboard Navigation*

**Replaces:** Geometric nearest-neighbor in navigate_directional
**Extends:** SPEC-HAL-038 (Focus), SPEC-HAL-176 (Keyboard Routing)

---

## 1. Problem

The current directional focus navigation uses pixel-distance
nearest-neighbor across a flat set of focusable widgets. This
is fragile, unexplainable, and produces chaotic results:

- PERP_TOLERANCE acts as a dead zone that filters real adjacent
  widgets when spacing is tight
- Perpendicular vs travel distance priority produces unintuitive
  jumps
- No concept of containment — a widget in a GroupBox can jump
  to an unrelated widget in a different GroupBox
- Users cannot predict where focus will go

**The test:** if you can't explain the logic to a user in one
sentence, the logic is wrong.

## 2. Design Principle

Focus navigation must follow the **visual structure** of the
UI, which is defined by the **widget tree**, not pixel positions.

**One-sentence rule:** Up/Down navigates between sibling rows.
Left/Right navigates between siblings within a row. When a
container boundary is reached, navigation moves to the parent's
next sibling.

## 3. Navigation Model

### 3.1 Layout Axis Detection

Every container (Flex, FlowGrid, etc.) has a **primary axis**:

- `Flex::row()` → primary axis = Horizontal
- `Flex::column()` → primary axis = Vertical
- `FlowGrid` → primary axis = Horizontal (wrapping)

The primary axis determines how children are ordered.

### 3.2 Navigation Rules

**Within a horizontal container (Flex row):**
- Left → previous sibling
- Right → next sibling
- Up → exit container, navigate parent's previous sibling
- Down → exit container, navigate parent's next sibling

**Within a vertical container (Flex column):**
- Up → previous sibling
- Down → next sibling
- Left → exit container, navigate parent's previous sibling
- Right → exit container, navigate parent's next sibling

**Entering a container:**
When navigating INTO a container (because it's the next
sibling), focus goes to the container's **first focusable
descendant** (for Right/Down) or **last focusable descendant**
(for Left/Up).

**Exiting a container:**
When the focused widget is the first/last child and navigation
would go before/after it, focus exits the container and
continues at the parent level.

### 3.3 Non-Focusable Containers

Most containers (Flex, GroupBox, ScrollArea) are not focusable.
Navigation passes THROUGH them to their focusable descendants.
When Down is pressed from the last widget in GroupBox A, the
algorithm:

1. Exits GroupBox A
2. Finds the next sibling of GroupBox A in the parent
3. If that sibling is GroupBox B (non-focusable), enters it
4. Focuses GroupBox B's first focusable descendant

### 3.4 Scroll Integration

When focus moves to a widget inside a ScrollArea, the
existing `post_layout_reveal_focused_widget` mechanism
scrolls it into view. No viewport filtering needed — the
tree structure determines navigation targets, scroll-reveal
makes them visible.

### 3.5 Sticky Cursor (Cross-Axis Position Memory)

When navigating cross-axis (Down/Up from a horizontal row,
Left/Right from a vertical column), pure tree navigation
enters the next row at its FIRST child. This ignores the
user's horizontal position.

**Example:** Checkboxes [Enable logging] [Verbose output]
[Auto-restart] above switches [Dark mode] [Notifications].
Down from Auto-restart (rightmost) should land on
Notifications (nearest below), not Dark mode (first child).

**Rule:** When crossing from one row to the next via Up/Down,
the algorithm remembers the current widget's **x-position**
and picks the child in the target row whose x-position is
closest.

```text
navigate_cross_axis(current_widget, target_row):
  1. ref_x = current_widget.paint_rect.x  (left edge)
  2. candidates = focusable descendants of target_row
  3. pick candidate with min |candidate.paint_rect.x - ref_x|
```

When crossing columns via Left/Right (rare), the same
applies with y-position.

This is the "sticky cursor" pattern from text editors: the
cursor maintains its column when moving between lines.

### 3.6 Active Area Reference Point

Widgets report their **active area** — the visually
meaningful region — not their full layout allocation.

A checkbox stretched to 300px wide has an active area of
~120px (indicator + label). A full-width TextField has an
active area equal to its layout rect.

For navigation distance calculations, the reference point
is the **left edge** (`paint_rect.x`) of the widget, not
its geometric center. This matches human perception: users
perceive a widget's position as "where it starts," not
"where its center is."

If a widget provides a `focus_anchor_x()` method, that
value is used instead of `paint_rect.x`. Default:
`paint_rect.x`.

## 4. Implementation Sketch

### 4.1 Widget Trait Addition

```rust
/// The layout axis of this container's children.
/// Returns None for leaf widgets or widgets that don't
/// define a spatial ordering for their children.
fn layout_axis(&self) -> Option<Axis> {
    None
}
```

`Flex::row()` returns `Some(Axis::Horizontal)`.
`Flex::column()` returns `Some(Axis::Vertical)`.

### 4.2 Navigation Algorithm

```rust
fn navigate_tree(
    tree: &WidgetTree,
    focused_id: TreeWidgetId,
    direction: Direction,
) -> Option<TreeWidgetId> {
    let focused_node = tree.find_node_by_widget_id(focused_id)?;

    // Walk up to find the nearest container with a layout axis
    let (container_id, axis) = find_layout_container(tree, focused_node)?;

    // Determine if direction is along or across the container axis
    let along = match (axis, direction) {
        (Axis::Horizontal, Direction::Left | Direction::Right) => true,
        (Axis::Vertical, Direction::Up | Direction::Down) => true,
        _ => false,
    };

    if along {
        // Navigate within container: previous/next sibling
        let forward = matches!(direction, Direction::Right | Direction::Down);
        navigate_sibling(tree, container_id, focused_node, forward)
    } else {
        // Navigate across container: exit to parent
        navigate_parent(tree, container_id, direction)
    }
}
```

### 4.3 Sibling Navigation

```rust
fn navigate_sibling(
    tree: &WidgetTree,
    container_id: NodeId,
    current: NodeId,
    forward: bool,
) -> Option<TreeWidgetId> {
    let siblings = tree.children(container_id);
    let current_idx = siblings.iter().position(|&id| id == current)?;

    let next_idx = if forward {
        current_idx + 1
    } else {
        current_idx.checked_sub(1)?
    };

    let next_sibling = *siblings.get(next_idx)?;

    // If next sibling is focusable, return it
    // If it's a container, return its first/last focusable descendant
    first_focusable_in_subtree(tree, next_sibling, forward)
}
```

### 4.4 Parent Navigation

```rust
fn navigate_parent(
    tree: &WidgetTree,
    container_id: NodeId,
    direction: Direction,
) -> Option<TreeWidgetId> {
    // Find the container's parent
    let parent_id = tree.parent(container_id)?;
    // Navigate from container_id as if it were the focused widget
    // in the parent's context
    navigate_tree_from_node(tree, parent_id, container_id, direction)
}
```

## 5. Geometric Fallback

Some layouts don't have clear axis semantics (FlowGrid with
wrapping, absolute positioning). For these, a geometric
fallback (travel-first nearest-neighbor) is acceptable.
The fallback is ONLY used when `layout_axis()` returns None
for all ancestors.

## 6. Invariants

**INV-FNAV-1**: Navigation always follows tree structure when
a layout axis is defined. Geometry is never used as primary
sort when tree structure is available.

**INV-FNAV-2**: Focus can always reach every focusable widget
via repeated navigation. No widget is inaccessible.

**INV-FNAV-3**: Navigation is reversible within a row:
Right then Left returns to the same widget. Down then Up
returns to the same widget when the sticky cursor
x-position matches. If x-positions differ across rows,
Down then Up may land on a different widget in the
original row (the one closest to the sticky x).

**INV-FNAV-4**: Navigation crosses container boundaries
transparently. The user doesn't need to know about GroupBoxes
or Flex containers — they just press Down and focus moves to
the next logical widget.

**INV-FNAV-5**: Scroll-to-reveal fires on every focus change.
Navigation targets are never filtered by viewport visibility.

**INV-FNAV-6**: Cross-axis navigation uses sticky cursor.
The target widget in the destination row is the one whose
left edge is closest to the source widget's left edge.
First-child bias only applies when all candidates are
equidistant.

## 7. Forbidden

**FORBID-FNAV-1**: No pixel-distance heuristics when tree
structure is available.

**FORBID-FNAV-2**: No viewport filtering of navigation
candidates. All focusable widgets in the tree are valid
targets.

**FORBID-FNAV-3**: No PERP_TOLERANCE or geometric dead zones
in tree-based navigation.

## 8. Migration

Phase 1: Add `layout_axis()` to Widget trait. Implement in
Flex (row/column), FlowGrid. Default None for all others.

Phase 2: Implement `navigate_tree()` algorithm. Use alongside
existing geometric fallback.

Phase 3: Replace `navigate_directional()` with `navigate_tree()`
as primary. Geometric fallback only when no ancestor has
layout_axis.

Phase 4: Remove PERP_TOLERANCE, viewport filtering, and
geometric sort from the primary navigation path.
