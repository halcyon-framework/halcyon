# SPEC-HAL-253: Tree Expansion Event Contract & Action Routing Model

**Status:** Draft (Binding upon adoption)
**Layer:** Interaction / Action System / Projection Authority
**Depends On:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-079 (Unified Event Flow v3)
- SPEC-HAL-080 (Halcyon Interaction Engine)
- SPEC-HAL-128 (Pointer Capture & Gesture Arbitration)

---

## 1. Purpose

Define:
1. The expansion/collapse action model for hierarchical data.
2. The routing path from input → TreeView → Action → TreeSubstrate.
3. The authority boundaries between:
   - Input system
   - TreeView (UI composite)
   - TreeSubstrate (state + projection)
   - Adapter (topology authority)

Expansion is state mutation. Therefore:

**TreeSubstrate is the sole authority over expansion state.**

TreeView is not.

---

## 2. Authority Model

### 2.1 TreeSubstrate Authority

TreeSubstrate owns:
- `expanded: HashSet<TreeNodeId>`
- Projection of visible rows
- Depth metadata per projected row
- Expandability (via adapter.has_children)

TreeSubstrate MUST expose mutation API:

```rust
pub enum TreeExpansionOp {
    Expand(TreeNodeId),
    Collapse(TreeNodeId),
    Toggle(TreeNodeId),
    ExpandAll(TreeNodeId),
    CollapseAll(TreeNodeId),
}
```

Mutation entrypoint:

```rust
pub fn apply_expansion(&mut self, op: TreeExpansionOp);
```

TreeSubstrate is responsible for:
- Updating expansion set
- Recomputing projection
- Invalidating virtualization range

---

### 2.2 TreeView Authority

TreeView is responsible for:
- Rendering indentation
- Rendering expand/collapse affordance (chevron, triangle, etc.)
- Hit-testing affordance region
- Mapping user input to TreeExpansionOp
- Emitting action through unified action system

TreeView MUST NOT:
- Mutate expansion state directly
- Store expansion state
- Infer topology

---

## 3. Action Model

### 3.1 TreeViewAction

Add a new action type:

```rust
pub enum TreeViewAction {
    Expansion(TreeExpansionOp),
}
```

This action is routed via the existing action system.

It must NOT bypass it.

---

### 3.2 Mapping Input → Action

#### 3.2.1 Pointer

PointerDown inside expand affordance region:
- If node has children:
  → emit `TreeExpansionOp::Toggle(node_id)`

PointerDown outside affordance:
→ normal row selection logic (unchanged)

Expansion gesture MUST NOT interfere with:
- SHIFT range selection
- CTRL multi-selection
- Drag-reorder
- Resize

Gesture arbitration must follow SPEC-HAL-128.

Expand hit region must be disjoint from drag-reorder region.

---

#### 3.2.2 Keyboard

When row is focused:
- Right Arrow:
  - If collapsed and expandable → Expand
  - Else → move focus to first child (if expanded)
- Left Arrow:
  - If expanded → Collapse
  - Else → move focus to parent

This behavior is standard tree navigation.

Action emitted must still be TreeExpansionOp.

TreeSubstrate does not interpret keyboard semantics; it only applies operations.

---

## 4. Routing Flow

Pointer example:

```
PointerEvent
→ EventRouter (SPEC-HAL-079)
→ TreeView::handle_pointer
→ detect affordance hit
→ emit TreeViewAction::Expansion(Toggle(node))
→ ActionSystem
→ TreeSubstrate.apply_expansion(...)
→ projection rebuild
→ ListView invalidation
```

Expansion never mutates UI state directly.
All state mutation flows through substrate.

---

## 5. Expansion Semantics

### INV-EXP-001: Idempotence

Expanding an already expanded node:
- MUST be no-op.

Collapsing already collapsed node:
- MUST be no-op.

---

### INV-EXP-002: Collapse Hides Descendants

If node N collapses:
- All visible descendants of N MUST disappear from projection.
- Expansion set for descendants MAY remain stored.
- Upon re-expand, previously expanded subtree MUST restore.

This preserves user intent.

---

### INV-EXP-003: ExpandAll / CollapseAll

If implemented:
- ExpandAll(N):
  - Must recursively expand all descendants.
- CollapseAll(N):
  - Must recursively collapse all descendants.

Complexity guarantee: `O(size_of_subtree)`

---

## 6. Projection Rebuild Trigger

TreeSubstrate MUST:
- Recompute projection immediately after expansion mutation.
- Emit revision change if projection affects visible rows.

No lazy rebuild allowed for now.

---

## 7. Fixed Row Height Constraint

Since we enforce fixed row height:

Expansion does NOT change per-row height.
It only changes:
- Visible row count
- Y offsets (derived from index × row_height)

Therefore virtualization remains O(1) index-based.

Expansion MUST NOT trigger geometry recalculation beyond row count.

---

## 8. Focus & Selection Interaction

Expansion MUST NOT:
- Clear selection
- Clear focus
- Change selection ordering

If collapsing hides the currently focused node:

Two options (choose one, binding):

**Option A (Recommended):**
- Focus moves to collapsed parent.

**Option B:**
- Focus remains logically on hidden node (not recommended).

**Binding decision: Option A.**

---

### INV-EXP-004: Focus Recovery

If collapsing node N hides focused node F:
- Focus MUST move to N.

---

## 9. Required Test Cases

1. Toggle expansion adds/removes children from projection.
2. Collapse preserves descendant expansion state.
3. Re-expand restores subtree.
4. Collapsing parent of focused node moves focus to parent.
5. ExpandAll expands entire subtree deterministically.
6. Repeated toggles are idempotent.
7. Expansion does not affect selection state.
8. Virtualization range adjusts correctly after expand/collapse.
9. Keyboard right arrow expands only if expandable.
10. Expand gesture does not trigger selection or drag-reorder.

---

## 10. Explicit Non-Goals

Not in this SPEC:
- Async loading
- Lazy child fetch
- Virtualized subtree partial loading
- Animated expand/collapse
- Variable row height

Those require separate SPEC.
