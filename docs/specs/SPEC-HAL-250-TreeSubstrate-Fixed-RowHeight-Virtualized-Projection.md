# SPEC-HAL-250: TreeSubstrate — Fixed Row Height Virtualized Projection

**Status:** Draft (Binding upon adoption)
**Layer:** Substrate / Projection Engine (Tree)
**Scope:** TreeSubstrate core state model, projection algorithm, fixed row height virtualization, expand/collapse stability, complexity guarantees.
**Non-goals:** Rendering, input mapping, selection semantics beyond projection-facing hooks, variable row height, async loading, filtering, sorting, incremental diff/patch protocol.

---

## Binding Invariants

### INV-250-001: Root Invisibility

An implicit root exists only as an internal anchor. It must not be visible, selectable, focusable, or exposed through public APIs.

### INV-250-002: Fixed Row Height

All visible rows are assumed to share one row_height (f32). Variable-height virtualization is forbidden.

### INV-250-003: Expand/Collapse Must Not Invalidate Virtualization

Expand/collapse must preserve projection correctness under fixed-height virtualization and must not require re-measuring or reflow.

### INV-250-004: Projection Is Pure Function of State

Given identical state (tree structure snapshot + expansion map + ordering + viewport), project() must return identical results. No timing, easing, or frame history.

### INV-250-005: Stable Node Identity

All projection-facing references use stable NodeId. Indices are derived, never authoritative.

### INV-250-006: No Implicit Adapter Calls During Projection

Projection must not "peek" content values, measure text, or query render state. Only child enumeration is permitted.

### INV-250-007: Deterministic Traversal Order

Sibling order is defined by the adapter's child enumeration order (or explicit order list provided by adapter). No hidden sorting.

### INV-250-008: Scroll Offsets Are in Row Units Only

Vertical scroll is interpreted as continuous y in pixels; row mapping uses `floor(y / row_height)`.

### INV-250-009: Pinned Header/Footer Domains Are Out of Scope

TreeSubstrate returns only row projections; consumer decides header/footer.

### INV-250-010: Complexity Bounds Are Binding

All operations must meet the complexity guarantees stated below; violating them is a bug.

---

## Terminology

- **NodeId**: stable identity of a node (adapter-defined).
- **root**: implicit internal node. Not in adapter domain (or if adapter has a real root, TreeSubstrate still wraps it in an implicit invisible root).
- **expanded**: a node whose children are included in the visible sequence.
- **collapsed**: a node whose children are excluded from the visible sequence.
- **visible sequence**: pre-order traversal of the tree restricted by expansion state, excluding root.
- **row_index**: 0-based index in the visible sequence.
- **viewport**: (scroll_y_px, viewport_height_px).

---

## State Model

### Adapter Contract (Minimal)

TreeSubstrate requires an adapter that can:
- enumerate root-level children: `children_of_root() -> Iterator<NodeId>`
- enumerate children: `children(node: NodeId) -> Iterator<NodeId>`
- optionally: `has_children(node: NodeId) -> bool` (may be derived by peeking iterator, but that's allowed only if adapter provides it or can do it cheaply)

No node values, no layout, no text metrics.

### Core State

```rust
struct TreeSubstrate {
    row_height: f32,

    // Expansion state (only meaningful for nodes with children).
    // Missing => collapsed by default (or configured default policy).
    expanded: HashSet<NodeId>,

    // Optional: cache of child lists to avoid repeated adapter enumeration.
    // This is a performance optimization; projection correctness must not depend on it.
    children_cache: HashMap<NodeId, Vec<NodeId>>,

    // Optional: cached subtree visible counts used for skipping.
    // Key: node_id, Value: number of visible rows in subtree rooted at node_id
    // (including the node itself, excluding implicit root).
    // Only valid relative to current expanded set and current tree snapshot.
    subtree_count_cache: HashMap<NodeId, usize>,

    // Structural version to invalidate caches when adapter snapshot changes.
    structure_version: u64,

    // Realized rows cache (output of last project()).
    realized: Vec<RowProjection>,
}
```

### Output Projection Record

```rust
struct RowProjection {
    node_id: NodeId,
    row_index: usize,
    depth: u16,          // 0 for top-level children of root
    y_offset: f32,       // row_index * row_height
    height: f32,         // row_height
    has_children: bool,
    is_expanded: bool,
}
```

y_offset is in content-space (not viewport-space). Consumers compute viewport y by subtracting scroll_y_px.

### Public Operations (State Transitions)

```
expand(node)
collapse(node)
toggle(node)
set_expanded(node, bool)
set_row_height(f32)
clear_expansion()      // optional
project(adapter, scroll_y_px, viewport_height_px, overscan_rows) -> &[RowProjection]
total_visible_rows(adapter) -> usize
```

All mutations must bump an internal projection_version so consumers can detect changes (optional but useful).

---

## Projection Algorithm

Two core requirements:
1. produce the correct visible rows for the viewport with overscan
2. do it without materializing the entire visible sequence for large trees

This SPEC mandates a skip-capable pre-order enumerator using cached subtree visible counts.

### Key Idea: Subtree Visible Row Count

For any node n (excluding implicit root):
- `subtree_visible_count(n) = 1 + sum(subtree_visible_count(c))` for each child c if n is expanded
- if n is collapsed: `subtree_visible_count(n) = 1`

For implicit root:
- `subtree_visible_count(root) = sum(subtree_visible_count(child))` for root-level children
- root itself is not counted as a row

If we can query subtree_visible_count(n) efficiently, we can skip entire subtrees when searching for the first visible row index in the viewport.

### Cache Invalidation Law

subtree_count_cache is valid only if:
- same structure_version
- same expanded set (or a tracked expanded_version)

Any expand/collapse must either:
- invalidate the entire cache, OR
- update along the ancestor chain (requires parent links; optional)

This SPEC allows full invalidation for correctness but requires the complexity bounds below (so full invalidation is allowed only if combined with bounded recomputation during queries).

---

## Required Complexity Guarantees

Let:
- N = total nodes in tree
- V = total visible nodes under current expansion
- R = number of realized rows returned for current viewport (including overscan)
- H = tree height
- K = number of expansion toggles since last cache rebuild

Guarantees:

**G-250-001** project() must be O(R * avg_child_enum_cost + S) where S is skipping cost. With subtree counts present, S must be O(H * log B) or O(H) depending on implementation; worst-case must not exceed O(H * M) where M is number of siblings traversed at skip boundaries. Practically: project() must not walk unrelated off-screen subtrees.

**G-250-002** total_visible_rows() must be O(number_of_expanded_nodes * avg_child_enum_cost) with cached subtree counts, and must not be O(V) on every call once caches are warm.

**G-250-003** toggle() / expand() / collapse() must be O(1) for state mutation plus bounded cache invalidation bookkeeping (allowed: clear caches). Any expensive recomputation is deferred to next project()/total_visible_rows().

---

## Pseudocode

### Helper: get children

```
fn children(adapter, node) -> &[NodeId] {
    if let Some(v) = children_cache.get(node) { return v; }
    let v = adapter.children(node).collect_vec();
    children_cache.insert(node, v);
    return children_cache.get(node).unwrap();
}
```

For root-level children, use a dedicated key ROOT_KEY in cache (implementation detail) or a separate field.

### Helper: is_expanded

```
fn is_expanded(node) -> bool { expanded.contains(node) }
```

### Helper: subtree_visible_count (memoized)

```
fn subtree_count(adapter, node) -> usize {
    if let Some(c) = subtree_count_cache.get(node) { return *c; }

    let has_children = adapter.has_children(node) OR children(adapter, node).len() > 0;
    let mut total = 1;

    if has_children && is_expanded(node) {
        for child in children(adapter, node) {
            total += subtree_count(adapter, child);
        }
    }

    subtree_count_cache.insert(node, total);
    total
}
```

### total_visible_rows

```
fn total_visible_rows(adapter) -> usize {
    // root is implicit and not counted as a row
    let mut total = 0;
    for child in children_of_root(adapter) {
        total += subtree_count(adapter, child);
    }
    total
}
```

### Finding the Start Row Index for Viewport

Compute:

```
first_row = floor(scroll_y_px / row_height)
last_row_exclusive = ceil((scroll_y_px + viewport_height_px) / row_height)
target_start = max(0, first_row as isize - overscan_rows)
target_end   = last_row_exclusive + overscan_rows
R = target_end - target_start
```

Now we must locate the node corresponding to target_start in the visible pre-order sequence.

We do this by descending from implicit root, consuming subtree counts to skip.

### locate_by_row_index

Returns an iterator state positioned at row k (0-based) and the current depth.

```
fn locate_by_row_index(adapter, k: usize) -> Cursor {
    let mut idx = k;
    let mut depth = 0;

    // iterate root-level children
    for child in children_of_root(adapter) {
        let c = subtree_count(adapter, child);
        if idx >= c {
            idx -= c;
            continue;
        }
        // the desired node lies within this child's visible subtree
        return locate_within_subtree(adapter, child, idx, depth);
    }

    return Cursor::End;
}

fn locate_within_subtree(adapter, node, idx, depth) -> Cursor {
    // idx is within node's subtree visible range.
    // pre-order: node first
    if idx == 0 {
        return Cursor::AtNode { node, depth, stack: [(node, 0, depth, State::Entered)] };
    }

    // otherwise, skip the node itself
    let mut idx = idx - 1;

    let has_children = adapter.has_children(node) OR children(adapter, node).len() > 0;
    if !has_children || !is_expanded(node) {
        return Cursor::End; // defensive
    }

    let kids = children(adapter, node);
    let mut child_i = 0;
    while child_i < kids.len() {
        let child = kids[child_i];
        let c = subtree_count(adapter, child);
        if idx >= c {
            idx -= c;
            child_i += 1;
            continue;
        }
        let mut stack = vec![];
        stack.push((node, child_i + 1, depth, State::InChildren));
        return locate_within_subtree_with_stack(adapter, child, idx, depth + 1, stack);
    }

    return Cursor::End;
}
```

The above is conceptually recursive; implementation should be iterative to avoid deep recursion.

### Enumerating Next Rows from Cursor

Once we have a cursor positioned at the start node, we produce R rows by walking pre-order respecting expansion.

Cursor stack element: `(node, next_child_index, depth, entered_flag)`

Rules:
- When visiting a node, emit it once.
- If node is expanded and has children, descend into child 0 next.
- When finishing a node's children, pop and continue with parent's next child.

### project()

```
fn project(adapter, scroll_y_px, viewport_height_px, overscan_rows) -> &[RowProjection] {
    realized.clear();

    let total_rows = total_visible_rows(adapter);
    if total_rows == 0 { return &realized; }

    let first_row = (scroll_y_px / row_height).floor() as isize;
    let start = max(0, first_row - overscan_rows as isize) as usize;
    let end_excl = min(total_rows,
        ((scroll_y_px + viewport_height_px) / row_height).ceil() as usize + overscan_rows);

    if start >= end_excl { return &realized; }

    let mut cursor = locate_by_row_index(adapter, start);
    let mut row_index = start;

    while row_index < end_excl && cursor.not_end() {
        let (node, depth) = cursor.current_node();

        realized.push(RowProjection {
            node_id: node,
            row_index,
            depth: depth as u16,
            y_offset: row_index as f32 * row_height,
            height: row_height,
            has_children: adapter.has_children(node) OR children(adapter, node).len() > 0,
            is_expanded: is_expanded(node),
        });

        row_index += 1;
        cursor.advance_preorder(adapter, children_cache, expanded);
    }

    &realized
}
```

### Cursor Advance

```
fn advance_preorder(adapter, ...) {
    let (node, depth, next_child_i, state) = stack.last_mut().unwrap();

    let has_children = adapter.has_children(node) OR children(node).len() > 0;

    // if not yet attempted children, and expanded, go to first child
    if state == Entered {
        state = InChildren;
        if has_children && is_expanded(node) {
            let kids = children(node);
            if kids.len() > 0 {
                let child = kids[0];
                stack.last_mut().unwrap().next_child_i = 1;
                stack.push((child, 0, depth + 1, Entered));
                return;
            }
        }
    }

    // otherwise, try next sibling from parent frames
    loop {
        let frame = stack.last_mut().unwrap();
        let node = frame.node;
        let depth = frame.depth;

        if has_children && expanded {
            let kids = children(node);
            if frame.next_child_i < kids.len() {
                let child = kids[frame.next_child_i];
                frame.next_child_i += 1;
                stack.push((child, 0, depth + 1, Entered));
                return;
            }
        }

        // done with this node
        stack.pop();
        if stack.is_empty() {
            set_end();
            return;
        }
        // continue loop; parent will try its next child
    }
}
```

---

## Expand/Collapse Behavior Guarantees

**Expand(node)**
- Adds node to expanded.
- Does not require computing any row y-values beyond fixed height.
- Must invalidate subtree_count_cache entries for affected subtree (allowed: clear entire cache).
- project() after expand must show children immediately in correct pre-order locations.

**Collapse(node)**
- Removes node from expanded.
- Must cause subtree rows to disappear.
- Scroll position interpretation stays pixel-based; no variable height, so no drift.

**"Doesn't kill virtualization"**

Meaning: expand/collapse cannot require scanning unrelated nodes; projection must remain viewport-bounded with subtree skipping.

---

## Required Edge Cases

**EC-250-001** Expand a node with zero children: no-op (but expansion flag may exist; projection must not break).

**EC-250-002** Collapse a node that is not expanded: no-op.

**EC-250-003** Toggle deep node with large subtree while viewport is elsewhere: project() cost must remain bounded (skip via cached subtree counts).

**EC-250-004** Structural changes from adapter (nodes added/removed): must bump structure_version and clear caches. Projection correctness is required; stability under live mutation is not otherwise specified in this SPEC.

**EC-250-005** Focus/selection currently on a node inside a collapsing subtree: projection must not crash; consumer policy decides where focus moves (out of scope). TreeSubstrate must provide enough information (node ids) to allow consumer to resolve.

---

## Complexity Notes

If you do not maintain subtree_visible_count, you are forced into O(V) traversal to locate viewport start, which kills tree virtualization. Therefore subtree counting is mandatory, with caching.

This SPEC permits two implementations:
1. Lazy memoized counts + full cache clear on toggle (simpler, may cause spikes on first project after toggle for huge expanded regions).
2. Parent links + incremental count maintenance on toggle (harder, bounded toggles).

This SPEC accepts (1) initially, but requires that project() remains viewport-bounded when caches are warm and that worst-case spikes are observable and fixable later.

### Pseudocode Summary

- `subtree_count(node)` memoized with expansion gating
- `locate_by_row_index(k)` skip via subtree_count
- `advance_preorder()` iterative stack walk
- `project()` computes [start, end) row indices by fixed row height math

---

## Acceptance Criteria for Implementation

1. Root never appears in realized rows; depth starts at 0 for root children.
2. project() returns correct pre-order visible rows for any expansion set.
3. Expand/collapse affects projection deterministically without any layout recalculation.
4. No timing/easing/frame loop.
5. Complexity: With warm caches, project() does not walk off-screen subtrees; it skips by subtree counts.
6. Unit tests cover:
   - simple tree expand/collapse
   - deep chain
   - wide siblings
   - viewport start in middle of large collapsed subtree
   - expand while viewport is elsewhere
