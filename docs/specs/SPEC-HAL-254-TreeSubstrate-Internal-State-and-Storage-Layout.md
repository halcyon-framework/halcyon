# SPEC-HAL-254: TreeSubstrate Internal State Structure & Storage Layout

**Status:** Draft (Binding upon adoption)
**Layer:** Data Substrate / Projection Engine
**Depends On:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-007 (State Model)
- SPEC-HAL-188 (Data Source & Mutation Contracts)
- SPEC-HAL-251 (TreeSubstrate — Core Model)

---

## 1. Purpose

Define the exact internal state layout of TreeSubstrate.

This SPEC specifies:
- Canonical stored state
- Derived projection state
- Storage invariants
- Memory complexity guarantees
- Mutation boundaries
- Rebuild algorithm contract

No UI.
No interaction.
No rendering.

Only substrate state.

---

## 2. High-Level Model

TreeSubstrate is a pure projection engine over hierarchical topology.

It stores:
1. Expansion state
2. Visible linear projection
3. Structural indices for O(1) access

It does NOT store:
- Tree topology
- Child lists
- Parent relationships
- Node ordering

Topology authority belongs to the Adapter.

---

## 3. Canonical Stored State

```rust
pub struct TreeSubstrate<N: TreeAdapter> {
    adapter: N,

    /// Expanded node set.
    expanded: HashSet<TreeNodeId>,

    /// Linear visible projection.
    projection: Vec<ProjectedNode>,

    /// Optional index for O(1) row lookup.
    row_index: HashMap<TreeNodeId, usize>,

    /// Revision counter.
    revision: u64,
}
```

---

## 4. ProjectedNode Layout

```rust
pub struct ProjectedNode {
    pub node_id: TreeNodeId,
    pub depth: u16,
}
```

Minimal by design.

No parent pointers.
No sibling pointers.
No subtree sizes.

All topology queries go through adapter.

---

## 5. Expansion State

### 5.1 Storage

```rust
expanded: HashSet<TreeNodeId>
```

Meaning:

Node ∈ expanded ⇒ children are visible.

If node not in set ⇒ collapsed.

Root is implicitly expanded and never stored.

---

### 5.2 Invariants

#### INV-TREE-STATE-001: Expansion Validity

Every node in expanded:
- MUST exist in adapter.
- MUST be expandable (has_children).

Invalid entries MUST be removed during reconciliation.

---

#### INV-TREE-STATE-002: No Duplicate Projection

Each TreeNodeId appears at most once in projection.

---

#### INV-TREE-STATE-003: Projection Order

Projection order MUST match:

Depth-first pre-order traversal of expanded tree.

No other ordering allowed.

---

## 6. Projection Storage

```rust
projection: Vec<ProjectedNode>
```

Represents visible nodes in linear order.

Index in vector = row index for virtualization.

Since row height is fixed:

```
row_y = index * row_height
```

No geometry stored in substrate.

---

## 7. Row Index Map

Optional but recommended:

```rust
row_index: HashMap<TreeNodeId, usize>
```

Used for:
- O(1) focus lookup
- O(1) selection anchor lookup
- O(1) scroll-to-node

Must be rebuilt together with projection.

Invariant:

```rust
projection[row_index[node]].node_id == node
```

---

## 8. Revision Model

Every structural mutation MUST:
- Increment revision
- Rebuild projection
- Rebuild row_index

No partial updates allowed.

This ensures deterministic behavior.

---

## 9. Projection Rebuild Algorithm

### 9.1 Trigger Conditions

Rebuild MUST occur when:
- Expansion changes
- Adapter revision changes
- Adapter topology mutation reported

---

### 9.2 Rebuild Pseudocode

```
function rebuild():
    clear projection
    clear row_index

    root = adapter.root()

    for child in adapter.children(root):
        traverse(child, depth = 0)

    revision += 1
```

Recursive traversal:

```
function traverse(node, depth):
    projection.push(ProjectedNode(node, depth))
    row_index[node] = projection.len - 1

    if node in expanded:
        for child in adapter.children(node):
            traverse(child, depth + 1)
```

---

### 9.3 Complexity Guarantees

Let:
- V = number of visible nodes
- T = total nodes in tree

Then:
- Rebuild = O(V)
- Memory = O(V)
- Expansion toggle worst-case = O(size_of_subtree_visible)

No algorithm may exceed O(V) during rebuild.

---

## 10. Memory Model

Memory usage:
- expanded: O(E) where E ≤ T
- projection: O(V)
- row_index: O(V)

Total worst-case: O(T)

No duplication of topology memory.

TreeSubstrate never mirrors full tree structure.

---

## 11. Mutation Entry Point

```rust
pub fn apply_expansion(&mut self, op: TreeExpansionOp)
```

Steps:
1. Mutate expanded
2. Validate expansion set
3. Call rebuild()

No incremental projection diffing allowed in v1.

Determinism over cleverness.

---

## 12. Collapse Semantics

When collapsing node N:
- Remove N from expanded
- Do NOT remove descendants from expanded
- They remain stored

This preserves subtree expansion state.

Re-expanding N restores previous expansion structure.

---

## 13. Adapter Revision Handling

TreeSubstrate must track adapter revision.

If adapter revision changes:
- Reconcile expanded:
  - Remove nodes that no longer exist
  - Remove nodes that are no longer expandable
- Rebuild projection

Invariant:

Projection must never reference removed nodes.

---

## 14. Forbidden State

TreeSubstrate MUST NOT:
- Cache children lists
- Store parent pointers
- Store subtree sizes
- Store indentation widths
- Store geometry
- Store UI state
- Store selection
- Store focus

Separation of concerns is absolute.

---

## 15. Determinism Guarantees

Given:
- Same adapter state
- Same expanded set

Projection must be identical.

No randomness.
No hash iteration order leaks.
Traversal order strictly defined by adapter.children order.

---

## 16. Test Case Categories

1. Projection order matches depth-first traversal.
2. Collapsing node removes only visible subtree.
3. Re-expanding restores previous subtree state.
4. Expanded set survives collapse of ancestor.
5. Removing node from adapter removes it from projection.
6. Removing expandable node removes it from expanded set.
7. row_index correctness invariant holds.
8. Multiple toggles produce deterministic projection.
9. Adapter revision change triggers rebuild.
10. Root never appears in projection.

---

## 17. Non-Goals

Not included:
- Variable row height
- Lazy subtree loading
- Partial projection rebuild
- Animated expand/collapse
- Subtree virtualization beyond linear visible model
