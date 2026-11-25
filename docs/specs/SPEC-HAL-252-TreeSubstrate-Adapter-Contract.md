# SPEC-HAL-252: TreeSubstrate Adapter Contract

**Status:** Draft (Binding upon adoption)
**Layer:** Data Adapter / Identity & Topology Supply
**Depends On:** SPEC-HAL-002 (Constitution & Architectural Invariants), SPEC-HAL-037 (Widget Identity & Lifecycle)

---

## 1. Purpose

Define the minimum adapter interface required to populate TreeSubstrate with hierarchical topology and stable identities, enabling deterministic projection and fixed-row-height virtualization downstream.

Adapter is the authority for:
- Node existence
- Node stable identity mapping
- Parent/child ordering
- Optional node metadata needed for row rendering (via ListView adapter, not here)

TreeSubstrate is the authority for:
- Expansion state
- Visible-row projection

---

## 2. Authority Boundaries

Adapter MAY:
- Provide roots
- Provide children for a node
- Provide stable IDs
- Provide ordering among siblings
- Provide "has children" hints (optional)
- Provide revision token (optional)

Adapter MUST NOT:
- Encode expansion state
- Encode projection state
- Encode UI semantics (selection, focus, gestures)
- Perform caching mandates on TreeSubstrate

TreeSubstrate MUST NOT:
- Infer or guess topology
- Reorder children
- Create synthetic nodes

---

## 3. Identity Law

### INV-ADP-001: Stable Node Identity

For a node that logically persists across frames/refresh:
- Adapter MUST return the same TreeNodeId.

If adapter cannot guarantee stability, TreeSubstrate cannot guarantee stable expansion behavior.

### INV-ADP-002: Unique IDs Within Adapter Domain

At any instant, no two distinct logical nodes may share a TreeNodeId.

---

## 4. Topology Law

### INV-ADP-003: Acyclic Parentage

The adapter-provided topology MUST be a forest (set of trees). Cycles are forbidden.

TreeSubstrate MAY debug-assert cycle detection and treat cycles as adapter bug.

### INV-ADP-004: Deterministic Ordering

Within a given parent, children(parent) ordering MUST be deterministic for a given adapter "revision".

---

## 5. Data Model Types

```rust
pub type TreeNodeId = crate::core::widget_tree::NodeId; // or a dedicated id type
```

TreeSubstrate treats TreeNodeId as opaque.

---

## 6. Adapter Trait

### 6.1 Minimal Synchronous Contract

```rust
pub trait TreeAdapter {
    /// Returns the current forest root nodes, in stable display order.
    fn roots(&self) -> &[TreeNodeId];

    /// Returns this node's children, in stable display order.
    ///
    /// Must return an empty slice if the node has no children.
    fn children(&self, node: TreeNodeId) -> &[TreeNodeId];

    /// Optional fast hint. If not implemented, TreeSubstrate may use children(node).is_empty().
    fn has_children(&self, node: TreeNodeId) -> bool {
        !self.children(node).is_empty()
    }
}
```

Binding notes:
- Returning slices implies adapter owns storage.
- Adapter must keep returned slices valid until next mutation/revision boundary (see §7).

If your existing ecosystem can't return slices safely, use `fn children_vec(&self, node) -> SmallVec<[TreeNodeId; N]>` instead; but do not change this SPEC unless necessary.

---

## 7. Mutation / Revision Boundary

TreeSubstrate needs to know when topology changed.

### 7.1 Revision Token (Required if Topology Can Change)

```rust
pub trait TreeAdapter {
    fn revision(&self) -> u64;

    fn roots(&self) -> &[TreeNodeId];
    fn children(&self, node: TreeNodeId) -> &[TreeNodeId];
    fn has_children(&self, node: TreeNodeId) -> bool { !self.children(node).is_empty() }
}
```

### INV-ADP-005: Revision Monotonicity

revision() MUST be monotonically increasing whenever roots/children ordering or membership changes.

TreeSubstrate uses this to invalidate cached topology/projection.

If your system already has a monotonic "generation" concept (like ListView adapter revisions), reuse it.

---

## 8. Error Handling

Adapter methods are infallible by contract.
If adapter cannot provide data for a node ID, it MUST treat it as "no children" and/or omit it from roots/children.

TreeSubstrate may debug-assert on "unknown node id" during development, but production must not panic.

---

## 9. Ownership / Allocation Rules

TreeSubstrate will likely internalize an arena of TreeNode { parent, children }.

Adapter is not required to provide parent pointers.
Parent pointers are derived during ingest.

Adapter must not require TreeSubstrate to retain references into adapter-owned memory beyond the synchronous call boundary unless protected by revision() invariants.

---

## 10. Ingest Contract (TreeSubstrate Side)

TreeSubstrate will build its internal topology from adapter data.

Binding behavior:
- On initial attach or revision change:
  - rebuild internal nodes arena
  - rebuild roots vector
  - drop expansion entries for nodes no longer present
  - preserve expansion for nodes whose IDs still exist

### INV-TS-INGEST-001: Expansion Preservation Under Stable IDs

If TreeNodeId remains present after ingest:
- expansion state MUST remain unchanged.

---

## 11. Required Test Cases

Case 1 — Stable IDs preserve expansion across revision change where sibling order changes.

Case 2 — Node removal drops its expansion state and all descendant expansion states.

Case 3 — Multiple roots ordering stable and deterministic.

Case 4 — Children ordering determinism: repeated calls within same revision yield identical slices.

Case 5 — Cycle protection: adapter returns child equal to ancestor; TreeSubstrate detects and avoids infinite traversal (debug assert / safe cutoff).

Case 6 — Large breadth: parent with 100k children; ingest does not blow stack; projection remains O(V).
