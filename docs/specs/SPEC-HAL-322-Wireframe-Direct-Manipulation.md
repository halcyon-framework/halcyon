# SPEC-HAL-322: Wireframe Direct Manipulation

### *Drag-and-Drop Widget Rearrangement in Wireframe Mode*

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-HAL-319 (Wireframe), SPEC-HAL-321 (Assembly), SPEC-HAL-002 (Constitution)

---

## 1. Purpose

This SPEC defines the drag-and-drop direct manipulation system for
wireframe mode. It enables visual rearrangement of composed widgets:
moving widgets between containers, reordering siblings, forming rows
from vertical stacks, dissolving empty containers, and deleting
widgets — all through direct mouse interaction on the live GPU-rendered
wireframe surface.

This transforms the wireframe from a passive display surface into an
interactive design tool.

---

## 2. Design Principles

### 2.1 Modifier-Gated Activation

Drag-and-drop manipulation MUST NOT activate on plain mouse drag.
Plain drag is reserved for normal widget interaction (slider thumbs,
scroll, text selection, column resize). Wireframe manipulation
activates ONLY with a modifier key held.

**Activation**: `Alt + Drag` (Option + Drag on macOS)

Alt is chosen because:
- Ctrl is used for multi-select and keyboard shortcuts
- Shift is used for range selection and constrained resize
- Alt is the least overloaded modifier
- Alt+Drag is the standard "move object" gesture in design tools
  (Figma, Sketch, Adobe XD)

### 2.2 Non-Destructive

All operations go through the composition system (MoveWidget,
RemoveWidget). The widget tree is never mutated directly by the
drag handler. Constitution §3.7 is preserved.

### 2.3 Wireframe Mode Only

Direct manipulation is ONLY active in wireframe/playground mode.
Normal application UIs never expose this behavior. This is enforced
by a flag on the App or Screen level, not by per-widget logic.

---

## 3. Interactions

### 3.1 Drag to Reorder (Within Same Parent)

**Trigger**: Alt+Drag a widget within its parent Flex container.

**Behavior**:
1. Widget lifts visually (elevated shadow, slight scale, reduced opacity)
2. Sibling gaps highlight as drop zones
3. Dropping between siblings reorders via MoveWidget with new index
4. Dropping back at original position is a no-op

**Composition action**: `MoveWidget { widget_id, new_parent_id: same, index: new_position }`

### 3.2 Drag to Reparent (Between Containers)

**Trigger**: Alt+Drag a widget out of its parent and over a different
Flex container or GroupBox.

**Behavior**:
1. The source container's slot collapses
2. The target container highlights and shows insertion indicators
3. Dropping inserts the widget as a child of the target at the
   indicated position
4. The widget retains its full subtree (children move with it)

**Composition action**: `MoveWidget { widget_id, new_parent_id: target, index }`

### 3.3 Drag to Form Row

**Trigger**: Alt+Drag a widget horizontally and drop it beside a
sibling in a vertical Flex container.

**Behavior**:
1. When a vertical Flex detects a horizontal drop (widget dragged
   left/right of a sibling rather than above/below), it interprets
   this as "make a row"
2. The engine wraps the drop target and the dragged widget in a
   new horizontal Flex row
3. The new Flex row takes the position of the original widget in
   the parent

**Composition actions** (sequence):
1. `AddWidget { parent_id: parent, widget_type: "Flex", config: {"direction":"row","spacing":8} }` → new_row_id
2. `MoveWidget { widget_id: target_sibling, new_parent_id: new_row_id, index: 0 }`
3. `MoveWidget { widget_id: dragged_widget, new_parent_id: new_row_id, index: 1 }`

### 3.4 Drag Out of Row (Row Dissolution)

**Trigger**: Alt+Drag the last remaining child out of a Flex row.

**Behavior**:
1. When a Flex container has zero children after a MoveWidget,
   the empty Flex is automatically removed
2. This prevents orphaned empty containers cluttering the tree

**Composition action**: `RemoveWidget { widget_id: empty_flex_id }`

This check runs AFTER every MoveWidget that removes a child from
a Flex. It is NOT triggered for GroupBox (empty GroupBox is valid
as a placeholder section).

### 3.5 Delete Widget

**Trigger**: Select a widget (click), then press `Delete` or `Backspace`.

**Alternative trigger**: Alt+Drag to a trash zone (bottom edge of
the wireframe window, rendered as a subtle red strip during drag).

**Behavior**:
1. Widget and its entire subtree are removed
2. Parent Flex recalculates layout
3. If parent Flex is now empty AND is not a root container,
   apply row dissolution (§3.4)

**Composition action**: `RemoveWidget { widget_id }`

### 3.6 Widget Selection

**Trigger**: Click on a widget (no modifier needed).

**Behavior**:
1. Selected widget gets a visible selection indicator
   (blue outline, 2px, outside the widget bounds)
2. Only one widget selected at a time (single selection)
3. Selection is a wireframe-mode concept — it does NOT use
   the focus system (focus is for keyboard input routing)
4. Clicking empty space deselects

Selection enables:
- Delete key removal (§3.5)
- Future: property panel showing widget config
- Future: resize handles
- Future: copy/paste

---

## 4. Visual Feedback

### 4.1 Drag Proxy

During Alt+Drag, the dragged widget renders as a semi-transparent
proxy at the pointer position:
- Opacity: 0.6
- Scale: 1.02 (slight enlargement)
- Drop shadow: elevation 4 (SPEC-HAL-030)
- Original position: shows a dashed outline placeholder

### 4.2 Insertion Indicators

When hovering over a valid drop zone during drag:

**Vertical Flex**: horizontal blue line (2px) between children,
spanning the container width. Indicates "insert here."

**Horizontal Flex**: vertical blue line (2px) between children,
spanning the container height.

**Empty container**: full container interior highlights with a
subtle blue tint.

Indicator color: `theme.accent` at 60% opacity.

### 4.3 Invalid Drop Zone

When hovering over a widget that cannot accept children (e.g.,
Label, TextField, Checkbox):
- Cursor changes to "not allowed" (🚫)
- No insertion indicator shown

### 4.4 Trash Zone

During any Alt+Drag, the bottom 40px of the wireframe window
renders as a trash zone:
- Background: subtle red gradient (10% opacity normally)
- When hovered during drag: 30% opacity + "Drop to delete" text
- Cursor: standard (not "not allowed" — deletion is intentional)

### 4.5 Selection Indicator

When a widget is selected (§3.6):
- Blue outline: 2px, `theme.accent` color, outside widget bounds
- Rendered as an overlay (does not affect layout)
- No corner handles (future: resize)

---

## 5. Drop Zone Detection

### 5.1 Hit-Test Model

During Alt+Drag, the engine runs a special drop-zone hit-test
that differs from normal widget hit-testing:

1. Only Flex containers and GroupBox containers are valid drop targets
2. Hit-test returns the **gap index** within the container, not
   the child widget
3. Gap detection uses midpoint thresholds: if the pointer is in
   the top half of child N, the gap is before N; bottom half →
   after N

### 5.2 Gap Geometry

For a vertical Flex with children at positions y0, y1, y2:
- Gap 0: y < (y0.top + y0.bottom) / 2 → before child 0
- Gap 1: between child 0 midpoint and child 1 midpoint
- Gap 2: between child 1 midpoint and child 2 midpoint
- Gap 3: y > (y2.top + y2.bottom) / 2 → after child 2

For a horizontal Flex: same logic on x axis.

### 5.3 Row Formation Detection

In a vertical Flex, if the pointer is horizontally offset from
a child (within the child's y-range but to the left or right
of the child's x-bounds), this signals row formation intent (§3.3).

Threshold: pointer x is > 50px away from the child's horizontal
center → row formation mode. Visual indicator changes from
horizontal insertion line to a vertical line beside the child.

---

## 6. Integration with Composition System

All mutations go through the existing composition action pipeline:

| Gesture | Composition Action |
|---------|--------------------|
| Reorder | MoveWidget (same parent, new index) |
| Reparent | MoveWidget (new parent, index) |
| Form row | AddWidget (Flex row) + 2x MoveWidget |
| Dissolve row | RemoveWidget (empty Flex) |
| Delete | RemoveWidget |

The wireframe drag handler emits these actions through the agent
action channel. They are processed in the deferred mutation phase
(Constitution §4.2). The drag handler NEVER mutates the tree
directly.

---

## 7. Invariants

**INV-322-1**: Drag manipulation is ONLY active when
`wireframe_mode == true`. Normal applications never expose
drag rearrangement.

**INV-322-2**: Alt modifier MUST be held throughout the entire
drag. Releasing Alt mid-drag cancels the operation and returns
the widget to its original position.

**INV-322-3**: All mutations go through the composition action
pipeline. The drag handler never calls `tree.add_child()`,
`tree.remove_node()`, or `tree.move_node()` directly.

**INV-322-4**: Drop zone hit-testing only targets Flex and GroupBox
containers. Leaf widgets (Label, TextField, Checkbox, etc.) are
never valid drop targets.

**INV-322-5**: Row dissolution only applies to Flex containers
that become empty after a child is moved out. GroupBox and other
containers are not auto-dissolved.

**INV-322-6**: The dragged widget's subtree moves intact. Children
are never separated from their parent during drag.

---

## 8. Forbidden Behaviors

**FORBID-322-1**: Plain drag (without Alt) MUST NOT trigger
wireframe manipulation. It must pass through to normal widget
interaction.

**FORBID-322-2**: Drag manipulation MUST NOT bypass the composition
pipeline. No direct tree mutation.

**FORBID-322-3**: Dropping on an invalid target (leaf widget) MUST
NOT silently reparent to the leaf's parent. It must cancel.

**FORBID-322-4**: Drag manipulation MUST NOT be available in
non-wireframe applications. No accidental exposure.

---

## 9. Implementation Phases

### Phase 1: Selection + Delete
- Widget selection on click (blue outline)
- Delete/Backspace removes selected widget
- Foundation for all subsequent phases

### Phase 2: Drag to Reorder
- Alt+Drag within same parent Flex
- Insertion indicators (horizontal/vertical lines)
- Drag proxy rendering
- MoveWidget on drop

### Phase 3: Drag to Reparent
- Alt+Drag between different containers
- Container highlight on hover
- Cross-container MoveWidget

### Phase 4: Row Formation + Dissolution
- Horizontal drop detection for row creation
- Automatic Flex row wrapping
- Empty Flex auto-removal
- Multi-action composition sequences

### Phase 5: Trash Zone + Polish
- Bottom-edge trash zone rendering
- Drop-to-delete
- Cancel on Alt release
- Undo support (future)

---

## 10. Future Extensions

- **Multi-select**: Shift+Click to select multiple widgets,
  drag them all as a group
- **Copy/Paste**: Ctrl+C/V to duplicate widget subtrees
- **Resize handles**: drag edges to set explicit width/height
- **Property panel**: side panel showing selected widget's
  config, editable in real-time
- **Alignment guides**: snap lines showing alignment with
  siblings (like Figma's smart guides)
- **Undo/Redo**: composition action stack with Ctrl+Z/Y
