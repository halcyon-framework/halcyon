# SPEC-HAL-191: ListView to TreeView Generalization Boundary

**Status:** Design Note
**Layer:** Widget Architecture
**Scope:** TreeView v1 precursor, no code
**Depends on:** SPEC-HAL-172, SPEC-HAL-187, SPEC-HAL-188, SPEC-HAL-189, SPEC-HAL-190 Appendix A
**Non-Goals:** TreeView implementation, tree data model design, hierarchy algorithms

---

## 1. Purpose

This SPEC defines which parts of ListView generalize to TreeView and establishes one hard invariant for TreeView v1 implementation.

It exists to prevent architectural drift before TreeView work begins.

---

## 2. ListView Internals Classification

### 2.1 Reusable (TreeView Inherits These)

| Component | Location | What It Does |
|-----------|----------|--------------|
| **TabularSubstrate** | `tabular_view/` | Column geometry, focus, selection, navigation |
| **Row virtualization** | `body.rs` | Viewport-bounded row realization |
| **Column projection** | `geometry_view.rs` | Header/body geometry slots |
| **RowHost lifecycle** | `list_view/row_host.rs` | RowId-keyed widget state, dirty flags |
| **CellHost lifecycle** | `list_view/cell_host.rs` | (RowId, ColumnId)-keyed widget state |
| **DataAdapter contract** | `data_adapter.rs` | Invalidation, extent, cell reads |
| **SPEC-HAL-189 dirty flags** | `row_host.rs` | Binding, Policy, Geometry, Presentation |
| **Intent forwarding** | `handle_intent()` | Selection, focus, navigation intents |

These are consumer-agnostic. TreeView uses them unchanged.

### 2.2 ListView-Specific (TreeView Replaces These)

| Component | Why It's ListView-Specific |
|-----------|---------------------------|
| **Flat row indexing** | TreeView has hierarchy; row index is visual, not semantic |
| **No expand/collapse** | ListView rows have no children |
| **Single depth level** | ListView depth is always 0 |
| **No tree metadata** | No parent_id, no depth, no expanded state |
| **Sorted indices = visual order** | TreeView's visible_indices filters by expand state |

### 2.3 Shared But Extended

| Component | ListView | TreeView Extension |
|-----------|----------|-------------------|
| **Row model** | Flat RowId list | RowId + TreeNodeMetadata (depth, parent_id, expanded) |
| **Realization** | All rows potentially visible | Only expanded subtrees realized |
| **Hit-testing** | Row hit → selection | Row hit + tree control zone hit |
| **First column** | Cell content only | Cell content + indentation + disclosure triangle |

---

## 3. TreeView v1 Classification

**INV-TREEVIEW-CLOSED-COMPOSITE:**
TreeView v1 is a **closed composite** per SPEC-HAL-190 Appendix A.

This means:

- TreeView owns a `TabularSubstrate` directly
- Hierarchy lives inside TreeView's owned state (TreeNodeMetadata array)
- Layout/paint/event phases access substrate via `&self.substrate`
- No `cx.authority<T>()` lookups
- No `cx.emit_intent()` calls
- No `AuthorityOwner` implementation

TreeView follows the same direct-substrate pattern as ListView.

---

## 4. Where Hierarchy Lives

**INV-TREEVIEW-HIERARCHY-INTERNAL:**
Tree hierarchy (parent_id, depth, expanded) is owned by TreeView, not by TabularSubstrate.

Per SPEC-HAL-172:

- Tree metadata is **external state**, not owned by TabularSubstrate
- TabularSubstrate provides column model, focus, selection, navigation
- TreeView owns expansion state and computes visible_indices

This separation means:

- TabularSubstrate remains frozen (no tree-specific changes)
- TreeView owns the tree logic
- Hierarchy does not leak into layout authority

---

## 5. The Hard Invariant

**INV-TREEVIEW-DIRECT-SUBSTRATE:**
TreeView v1 MUST be implementable with the same direct-substrate pattern as ListView.

Concretely:

```rust
// TreeView layout
fn layout(&mut self, cx: &mut LayoutContext, ...) -> LayoutResult {
    // Direct substrate access — no cx.authority<T>()
    self.substrate.set_body_viewport(viewport);
    self.substrate.prepare_evaluation(width);
    // ...
}

// TreeView paint
fn paint(&self, cx: &mut PaintContext, rect: RectF, ...) {
    // Direct substrate access — no cx.authority<T>()
    let realized_rows = self.substrate.body().realized_rows();
    let header_slots = self.substrate.geometry_view().header_slots();
    // ...
}

// TreeView event handling
fn handle_event(&mut self, cx: &mut EventContext, ...) -> bool {
    // Direct intent forwarding — no cx.emit_intent()
    self.substrate.handle_intent(&intent);
    // ...
}
```

If TreeView cannot be implemented this way, the architecture is wrong — not the invariant.

---

## 6. What TreeView v1 Does NOT Need

Per SPEC-HAL-190 Appendix A §A.2, SPEC-HAL-190 machinery is required only when:

- Children are externally pluggable ❌ (TreeView's rows are internal)
- Multiple heterogeneous widgets must coordinate ❌ (TreeView is self-contained)
- Ownership crosses widget boundaries ❌ (TreeView owns its substrate)
- Deferred multi-step interaction semantics ❌ (expand/collapse is single-step)
- Siblings need shared truth ❌ (TreeView is not coordinating with siblings)

None of these apply to TreeView v1. SPEC-HAL-190 machinery is not required.

---

## 7. When SPEC-HAL-190 Would Be Required

SPEC-HAL-190 becomes required only when:

- A **SearchCoordinator** needs to coordinate ListView + TreeView + Grid
- A **shared selection authority** must span multiple independent views
- **Cross-widget drag-and-drop** needs common state
- An **external filter bar** needs to affect multiple consumers

These are v2+ concerns. TreeView v1 is self-contained.

---

## 8. Implementation Checklist (Non-Normative)

When implementing TreeView v1:

- [ ] Create `TreeView<S: DataSource>` owning `TabularSubstrate` and `TreeNodeMetadata`
- [ ] Implement `Widget` trait with direct substrate passing
- [ ] Compute `visible_indices` from expand state before `prepare_evaluation()`
- [ ] Paint disclosure triangles using `geometry_view().body_slots()` + depth offset
- [ ] Handle tree control zone hits in `handle_event()`
- [ ] Forward expand/collapse via `TreeIntent` to self (not via SPEC-HAL-190)
- [ ] Verify no `cx.authority<T>()` or `cx.emit_intent()` calls exist

---

## 9. Closure Condition

This SPEC is satisfied when TreeView v1 compiles and runs using only:

- Direct `&self.substrate` access
- Direct `self.tree_metadata` access
- Direct `self.substrate.handle_intent()` calls

No SPEC-HAL-190 runtime machinery.

---

## 10. Summary

| Question | Answer |
|----------|--------|
| Is TreeView a closed composite? | **Yes** |
| Does TreeView need SPEC-HAL-190? | **No** (v1) |
| Where does hierarchy live? | **TreeView-owned TreeNodeMetadata** |
| What pattern does TreeView use? | **Direct substrate passing** |
| What invariant protects this? | **INV-TREEVIEW-DIRECT-SUBSTRATE** |

ListView remains the canonical example. TreeView follows the same pattern.
