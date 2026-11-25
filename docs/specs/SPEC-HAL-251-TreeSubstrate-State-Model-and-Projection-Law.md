# SPEC-HAL-251: TreeSubstrate — State Model & Projection Law

**Status:** Draft (Binding upon adoption)
**Layer:** Data Substrate / Projection Authority
**Depends On:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-158 (Layout Authority & Invalidation Law)
- SPEC-HAL-146 (Temporal Ownership of Geometry)
- SPEC-HAL-183 (Row Height Model)
- SPEC-HAL-179 (TabularSubstrate Identity Model)

---

## 1. Purpose

TreeSubstrate is the hierarchical data substrate that projects a tree-structured dataset into a flat, deterministic, virtualizable row stream suitable for consumption by ListView.

TreeSubstrate:
- Owns hierarchical identity and expansion state.
- Produces a flat projection of visible rows.
- Guarantees deterministic ordering.
- Maintains O(n) projection cost per rebuild.
- Does not own layout.
- Does not own rendering.
- Does not own geometry.
- Does not own semantics.

It is a projection engine only.

---

## 2. Authority Boundaries

TreeSubstrate MAY:
- Store tree topology (parent → children relations).
- Store per-node expansion state.
- Produce ordered visible row projection.
- Provide depth metadata per visible row.
- Provide parent linkage via NodeId.

TreeSubstrate MUST NOT:
- Compute geometry.
- Compute indentation pixel values.
- Mutate external UI state.
- Perform layout.
- Own selection semantics.
- Own column logic.
- Store references to widgets.

All identity is ID-based per Constitution law.

---

## 3. Identity Model

```rust
pub type TreeNodeId = NodeId; // Stable, opaque, engine-assigned
```

Nodes are uniquely identified and stable across frames unless explicitly removed.

TreeSubstrate must preserve identity stability across:
- Expand
- Collapse
- Reorder (within same parent)
- Data refresh (if node identity preserved by adapter)

---

## 4. State Model

### 4.1 Core State

```rust
pub struct TreeSubstrate {
    nodes: Arena<TreeNode>,
    roots: Vec<TreeNodeId>,          // Explicit root set
    expanded: HashSet<TreeNodeId>,   // Expansion state
}
```

### 4.2 TreeNode

```rust
pub struct TreeNode {
    pub id: TreeNodeId,
    pub parent: Option<TreeNodeId>,
    pub children: Vec<TreeNodeId>,
}
```

---

## 5. Explicit Root Model

TreeSubstrate uses an explicit root set.

There is NO implicit synthetic root node.

`roots: Vec<TreeNodeId>` defines top-level ordering.

The root container is not visible.
Only actual nodes appear in projection.

This guarantees:
- No phantom row.
- No extra depth level.
- Clean projection semantics.

---

## 6. Projection Model

TreeSubstrate produces:

```rust
pub struct TreeRowProjection {
    pub node_id: TreeNodeId,
    pub depth: u32,
}
```

Projection is a flat Vec.

Order is:
- Depth-first
- Pre-order traversal
- Stable within sibling order

---

## 7. Projection Algorithm

### 7.1 Pseudocode

```
visible_rows = []

for each root in roots:
    dfs(root, depth=0)

function dfs(node, depth):
    push (node, depth) into visible_rows

    if node is expanded:
        for child in node.children:
            dfs(child, depth + 1)
```

---

### 7.2 Complexity Guarantees

Let:
- N = total nodes
- V = visible nodes

Projection rebuild cost:
- Worst case: O(N)
- Common case: O(V)
- Expand/collapse subtree: O(size_of_subtree)

Memory:
- O(N) storage
- O(V) projection buffer

No recursion must allocate heap memory beyond stack frames.

Implementation MAY use explicit stack instead of recursion to avoid deep recursion overflow.

---

## 8. Expand / Collapse Semantics

### 8.1 Expand

Operation:

```rust
expanded.insert(node_id)
```

Effect:
- Subtree becomes visible.
- Projection invalidated.
- Requires projection rebuild.

### 8.2 Collapse

Operation:

```rust
expanded.remove(node_id)
```

Effect:
- Entire subtree hidden.
- Projection invalidated.
- Requires projection rebuild.

---

## 9. Fixed Row Height Contract

TreeSubstrate assumes:
- Fixed row height (per SPEC-HAL-183).

TreeSubstrate does NOT:
- Store row heights.
- Compute per-row geometry.
- Support variable height.

Expand/collapse does not affect row height.

Only row count changes.

---

## 10. Projection Invariants

### INV-TREE-001: Deterministic Projection

Given identical:
- nodes
- roots
- expanded set

Projection MUST be identical in ordering and depth values.

---

### INV-TREE-002: Stable Identity

If a node remains in the tree and its parent relation unchanged:
- Its position in projection is stable relative to sibling ordering.

---

### INV-TREE-003: No Hidden Structural Mutation

Projection MUST NOT mutate tree topology.

Projection is pure function of state.

---

### INV-TREE-004: Expand Does Not Reorder Siblings

Expand only reveals children.
Sibling order must not change.

---

### INV-TREE-005: Collapse Hides Entire Subtree

If node is collapsed:
- None of its descendants appear in projection.

---

## 11. Edge Case Matrix (For Test Implementation)

### Case A — Single Root, No Children

- Projection size = 1
- depth = 0

---

### Case B — Single Root, Collapsed With Children

Tree:

```
A
├ B
└ C
```

Expanded = {}

Projection:

```
A (depth 0)
```

---

### Case C — Single Root, Expanded

Expanded = {A}

Projection:

```
A (0)
B (1)
C (1)
```

---

### Case D — Nested Expansion

```
A
└ B
  └ C
```

Expanded = {A, B}

Projection:

```
A (0)
B (1)
C (2)
```

---

### Case E — Collapse Parent After Deep Expand

Expanded = {A, B}
Collapse A

Expanded = {}

Projection:

```
A (0)
```

C must not appear.

---

### Case F — Multiple Roots

roots = [A, X]

Projection must preserve root ordering.

---

### Case G — Deep Chain (10k Depth)

Must not stack overflow.
Implementation must support iterative DFS.

---

### Case H — Expand Node With No Children

Projection unchanged except expanded set updated.

---

### Case I — Remove Node While Expanded

If expanded node removed:
- Must be removed from expanded set.
- Projection must not panic.

---

### Case J — Reorder Children Under Same Parent

Projection must reflect new sibling order.

---

## 12. Interaction With ListView

TreeSubstrate feeds:

```rust
Vec<TreeRowProjection>
```

ListView:
- Handles row selection.
- Handles column projection.
- Handles geometry.
- Handles scrolling.
- Handles drag-reorder of columns.

TreeSubstrate does not know about columns.

---

## 13. Explicit Non-Goals

TreeSubstrate does NOT implement:
- Variable row height
- Column logic
- Node spanning
- Lazy child loading
- Partial projection diffs
- Async mutation

These require separate SPECs.

---

## 14. Migration and Future Extensions

Future SPECs MAY introduce:
- Partial projection diffing
- Incremental subtree invalidation
- Lazy child fetch
- Variable-height rows
- Virtual depth compression

Such SPECs MUST:
- Preserve deterministic projection invariant
- Preserve identity stability
- Preserve authority boundaries
