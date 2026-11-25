# SPEC-HAL-249: TreeSubstrate — Projection and Expand/Collapse Law

**Status:** Draft (Binding upon adoption)
**Layer:** Data Substrate / Projection Law
**Applies To:** TreeSubstrate, TreeView, ListView integration
**Depends On:** SPEC-HAL-179 (ListView Identity & Projection), TabularSubstrate row virtualization model

---

## 1. Purpose

TreeSubstrate introduces hierarchical data support without modifying ListView, virtualization, geometry, or column systems.

TreeSubstrate:
- Owns hierarchy
- Owns expand/collapse state
- Produces a flat, ordered projection of visible rows

It does not:
- Perform layout
- Perform virtualization
- Perform painting
- Perform scrolling
- Modify column geometry

ListView continues to operate on a flat row projection.

---

## 2. Architectural Position

TreeSubstrate is a projection substrate.

It sits between:

```
Hierarchical Data
↓
TreeSubstrate
↓
Flat Visible Row Projection
↓
ListView (unchanged)
```

TreeSubstrate is responsible for visibility, not geometry.

---

## 3. Core Concepts

### 3.1 Tree Node Identity

Each node has:
- NodeId (stable identity)
- Parent reference (optional)
- Ordered list of children

Identity must remain stable across expand/collapse.

---

### 3.2 Visible Projection

TreeSubstrate maintains:

```rust
visible_rows: Vec<NodeId>
```

This vector is the only list consumed by ListView.

The projection is:
- Depth-first
- Pre-order traversal
- Includes only nodes whose ancestors are expanded

---

### 3.3 Depth Information

TreeSubstrate must provide:

```rust
depth(node_id) -> usize
```

Depth is:
- 0 for root-level nodes
- parent_depth + 1 for children

Depth affects indentation only.

Depth does not affect row height.

---

### 3.4 Expand State

TreeSubstrate maintains:

```rust
expanded: HashSet<NodeId>
```

Rules:
- A node is considered visible if all its ancestors are expanded.
- Leaf nodes may be marked expanded but this has no effect.
- Collapsing a node hides all descendants recursively.

---

## 4. Binding Invariants

### INV-249-1: Projection Purity

TreeSubstrate must produce a flat, ordered projection of visible nodes.

It must not expose hierarchy to ListView.

ListView must never require tree awareness.

---

### INV-249-2: Fixed Row Height Compatibility

TreeSubstrate must not alter row height behavior.

Row height remains fixed.

Depth must not affect height.

---

### INV-249-3: Deterministic Traversal

Given identical:
- Tree structure
- Expand state

The projection must be identical.

Traversal order must be stable.

---

### INV-249-4: Collapse Removes Descendants

When collapsing a node:
- All descendants must be removed from visible projection.
- No partially visible subtree may remain.

---

### INV-249-5: Selection Integrity

If a selected node becomes hidden due to collapse:
- It must be removed from selection.
- If it was focused, focus must move to the collapsing parent.

No invisible row may remain selected.

---

### INV-249-6: Scroll Stability

Expand/collapse must not corrupt scroll state.

If a parent collapses:
- The parent row must remain in place.
- Scroll offset must remain stable relative to top visible row.

No jump-to-top behavior allowed.

---

### INV-249-7: O(Visible) Projection Rebuild

Projection rebuild complexity must be:

- `O(total_nodes)` worst case
- `O(subtree_size)` on collapse

No per-frame rebuilds.

Projection updates must be event-driven.

---

### INV-249-8: Geometry Isolation

TreeSubstrate must not:
- Access geometry_view()
- Modify column offsets
- Interact with drag system
- Interact with animation system

Hierarchy is independent of layout.

---

## 5. Public API Surface (Minimal)

TreeSubstrate must expose:

```rust
pub fn visible_rows(&self) -> &[NodeId];

pub fn depth(&self, node: NodeId) -> usize;

pub fn is_expanded(&self, node: NodeId) -> bool;

pub fn toggle_expand(&mut self, node: NodeId);

pub fn expand(&mut self, node: NodeId);

pub fn collapse(&mut self, node: NodeId);
```

Expand/collapse must trigger projection rebuild.

---

## 6. TreeView Responsibilities

TreeView:
- Uses TreeSubstrate as row provider.
- Delegates row projection to ListView.
- Adds indentation rendering in first column only.
- Adds expand/collapse triangle hit testing in first column only.
- Maps keyboard:
  - Left → collapse
  - Right → expand

TreeView must not modify ListView virtualization logic.

---

## 7. Rendering Rules

Indentation:

```
indent_px = depth(node) * INDENT_WIDTH
```

Applied only in first column cell renderer.

Other columns remain unchanged.

Column system (pin, reorder, resize, hide/show) remains fully operational.

---

## 8. Forbidden Behaviors

TreeSubstrate must not:
- Introduce variable row height
- Modify ListView scroll model
- Modify ListView virtualization engine
- Depend on animation subsystem
- Store layout information
- Introduce tree awareness into geometry layer

Hierarchy is a projection concern only.

---

## 9. Performance Constraints

TreeSubstrate must support:
- ≥ 100k nodes
- Deep nesting
- Frequent expand/collapse

Projection rebuild must not allocate per frame.

All rebuilds must be explicit on state change.

---

## 10. Non-Goals (Deferred)

- Variable-height tree rows
- Lazy-loaded children
- Async node population
- Partial subtree virtualization
- Multi-column hierarchical grouping

These require separate SPECs.

---

## 11. Summary

TreeSubstrate:
- Owns hierarchy
- Owns expand/collapse
- Produces flat visible projection
- Preserves ListView invariants
- Preserves virtualization integrity
- Preserves column system integrity

ListView remains untouched.

Hierarchy is projection, not layout.
