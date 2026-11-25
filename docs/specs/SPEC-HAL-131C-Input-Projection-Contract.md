# SPEC-HAL-131C: Input Projection Contract

**Status:** Implemented
**Layer:** Engine coordination (input routing)
**Scope:** Virtualization → Input hit-testing bridge
**Depends on:** SPEC-HAL-131A, SPEC-HAL-131B
**Constrains:** Hit-testing, pointer events, focus targeting

---

## SECTION A — Problem Statement

SPEC-HAL-131A/B established virtualization correctness: realization during `layout_tree()`, anchor resolution post-layout, no realization from scroll writes.

What is missing is the **input → virtualization boundary**:

* Hit-testing operates on realized widgets, but virtualized lists have unrealized items
* Clicking on an "unrealized" region must still identify the logical item
* Input processing must not trigger realization
* Logical item identity must be stable across realization churn

These behaviors require input to operate on a **projection snapshot**, not on the realized widget tree.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Projection, not realization**
Input queries a pre-computed snapshot of logical item positions, not the realized tree.

**PRINCIPLE-2: Logical identity first**
Hit results include logical item identity, not just widget references.

**PRINCIPLE-3: Read-only after layout**
The projection snapshot is computed during layout and is immutable for input processing.

**PRINCIPLE-4: No surprise work**
Input processing must not trigger realization, layout, or anchor resolution.

---

## SECTION C — Terminology

**Input Projection**
A snapshot of logical item bounds and identities for a virtualizing container.

**Logical Item ID**
A stable identifier for a logical item that survives realization and unrealization cycles.

**Projected Hit Result**
A hit-test result that includes logical identity, not just widget reference.

**Realized Widget**
An item that currently has an instantiated widget in the tree.

---

## SECTION D — Invariants

### INV-INPUT-PROJECTION-1: Query Projection, Not Tree
Input hit-testing must query the projection snapshot, never the realized widget tree directly.

### INV-INPUT-PROJECTION-2: Read-Only Processing
Input resolution does not mutate projection or trigger realization.

### INV-INPUT-IDENTITY-1: Stable Logical Identity
Input targets logical identity, stable across realization churn.

### INV-PROJECTION-SNAPSHOT-1: Computed During Layout
Projection computed during `layout_tree()`, read-only after.

### INV-PROJECTION-CONSUMER-1: Authoritative Range (SPEC-HAL-134 Phase 3)
`post_layout_populate_input_projections()` MUST treat `realized_range` as authoritative.
The function MUST NOT recompute, clamp, extend, or reinterpret the range.
Enforcement: Debug assertions in `src/core/app.rs`.

---

## SECTION E — Architecture

### E.1 InputProjection Type

```rust
pub struct InputProjection {
    container_id: Option<TreeWidgetId>,
    viewport_bounds: RectF,
    scroll_offset: (f32, f32),
    entries: Vec<InputProjectionEntry>,
    content_extent: (f32, f32),
}

pub struct InputProjectionEntry {
    logical_id: LogicalItemId,
    content_bounds: RectF,
    realized_widget: Option<TreeWidgetId>,
    logical_index: usize,
}
```

### E.2 InputProjectionProvider Trait

Widgets that virtualize content implement this trait:

```rust
pub trait InputProjectionProvider {
    fn logical_item_count(&self) -> usize;
    fn logical_item_bounds(&self, index: usize) -> Option<RectF>;
    fn logical_item_id(&self, index: usize) -> Option<LogicalItemId>;
    fn realized_widget_for_item(&self, index: usize) -> Option<TreeWidgetId>;
    fn realized_range(&self) -> Range<usize>;
    fn content_width(&self) -> f32;
}
```

### E.3 Widget Trait Extension

```rust
trait Widget {
    fn as_input_projection_provider(&self) -> Option<&dyn InputProjectionProvider> {
        None
    }
}
```

---

## SECTION F — Pipeline Ordering

Input projection is integrated into the frame pipeline:

```
1. layout_tree()                           — compute widget geometry
2. post_layout_populate_input_projections() — build projection snapshots
3. post_layout_resolve_scroll_anchors()    — resolve scroll anchors
4. paint_tree()                            — build scene graph
```

**INV-PROJECTION-SNAPSHOT-1** is satisfied by placement at step 2.

---

## SECTION G — Hit-Testing Flow

### G.1 Standard Hit-Test (unchanged)
`hit_test(x, y)` returns `HitTestResult` with widget ID and local position.

### G.2 Projection-Aware Hit-Test
`hit_test_projected(x, y)` returns `ProjectedHitResult`:

```rust
pub struct ProjectedHitResult {
    logical_item_id: Option<LogicalItemId>,
    logical_index: Option<usize>,
    realized_widget_id: Option<TreeWidgetId>,
    container_id: TreeWidgetId,
    local_pos: (f32, f32),
    item_bounds: RectF,
}
```

Algorithm:
1. Perform standard hit-test to find container
2. Walk ancestors looking for input projection
3. If projection found, hit-test within projection
4. Return logical identity (even for unrealized items)

---

## SECTION H — Implementation

### H.1 List Widget

List implements `InputProjectionProvider`:

```rust
impl InputProjectionProvider for List {
    fn logical_item_count(&self) -> usize {
        self.item_count
    }

    fn logical_item_bounds(&self, index: usize) -> Option<RectF> {
        let y = if self.item_height > 0.0 {
            index as f32 * self.item_height
        } else {
            self.item_y_position(index)
        };
        // ...
    }

    fn logical_item_id(&self, index: usize) -> Option<LogicalItemId> {
        // Use index as stable identity
        Some(LogicalItemId::new(index as u64))
    }
}
```

### H.2 Engine Population

`post_layout_populate_input_projections()` iterates all widgets, finds those implementing `InputProjectionProvider`, and builds their projections.

---

## SECTION I — Tests

Required tests (all passing):

1. **Click on unrealized item** — resolves to correct logical ID
2. **Projection is read-only** — repeated queries don't mutate state
3. **Repeated hit-test stable** — same input yields same result
4. **List provides projection** — correct bounds and IDs
5. **Scroll offset respected** — viewport → content transform correct
6. **Find by logical ID** — lookup by identity works
7. **Binary search (item_at_y)** — variable height lookup efficient

---

## SECTION J — Acceptance Criteria

SPEC-HAL-131C is satisfied when:

* [x] InputProjection type and storage added to WidgetTree
* [x] InputProjectionProvider trait defined
* [x] List implements InputProjectionProvider
* [x] `hit_test_projected()` returns logical identity
* [x] Projection populated after layout, before anchor resolution
* [x] Click on unrealized item resolves correctly
* [x] Input does not trigger realization
* [x] All 7 tests passing

---

## SECTION J.2 — Scroll Sync Scope Isolation (Added 2025-12-23)

This section formalizes the scope isolation for scroll synchronization
between ScrollAreas and their virtualizing children.

### INV-SCROLL-SYNC-SCOPE-1 — Nested ScrollArea Isolation

Scroll synchronization (both pre-layout and post-layout) applies ONLY to
descendants of a ScrollArea's content subtree and MUST NOT cross into
nested ScrollAreas.

**Rationale:**
Each ScrollArea owns its own scroll offset. A child ScrollArea has
independent scroll state that MUST NOT be overwritten by parent sync.

**Enforcement:**
- `pre_layout_sync_scroll_to_lists()` stops traversal at nested ScrollAreas
- `post_layout_sync_scroll_to_lists()` stops traversal at nested ScrollAreas
- This behavior is intentional, not incidental

### INV-SCROLL-SYNC-SCOPE-2 — Input Projection Scope

Input projections are scoped to their owning ScrollArea.
A nested ScrollArea produces a separate InputProjection.

### INV-SCROLL-SYNC-SCOPE-3 — No Cross-Container Leakage

Scroll offset from one ScrollArea MUST NOT affect input projection
calculations for a nested ScrollArea.

**See also:** SPEC-HAL-131A Section L.5

---

## SECTION J.3 — Scroll & Interaction Projection Consistency (Binding)

This section establishes binding law for how input projections interact with
scroll capture and interaction state. These invariants are engine-grade,
survive refactors, and override widget preferences.

---

### INV-PROJECTION-SCROLL-1 — Projections do not change scroll target

Input projections MUST NOT implicitly change the scroll target.

Hit-testing through a projection returns the logical item identity.
It does NOT establish scroll capture, scroll authority, or scroll routing.

Scroll target is established by explicit user action, not by projection
query results.

---

### INV-PROJECTION-CAPTURE-1 — Projections respect capture owner

Projection hit-testing MUST respect the current scroll capture owner.

If a widget has scroll capture:
- Projection queries still return accurate logical identities
- But scroll routing ignores projection results
- Captured widget receives all scroll input regardless of pointer position

Projection is informational. Capture is authoritative.

---

### INV-PROJECTION-HOVER-1 — Hover projections do not imply authority

Hover projections MUST NEVER imply capture or scroll authority.

A pointer hovering over a projected item:
- Updates hover state for visual feedback
- Does NOT capture scroll
- Does NOT consume scroll input
- Does NOT establish scroll routing

Hover is visual. Capture requires explicit action.

---

### INV-PROJECTION-IMMUTABLE-1 — Projections are read-only for input

Input processing MUST NOT mutate projection state.

Projections are computed during layout and frozen for the frame.
Input queries read projections. Input processing does not write them.

Any mutation to projection occurs in the next layout pass, not during
input handling.

---

## SECTION K — Authoritative Statements

**AUTH-131C-1: "Input operates on projection, not realization."**

**AUTH-131C-2: "Logical identity survives the realized/unrealized boundary."**

**AUTH-131C-3: "Input processing is read-only with respect to virtualization state."**

---

## SECTION L — Files Modified

- `src/core/input_projection.rs` — New module with types and traits
- `src/core/mod.rs` — Added input_projection module
- `src/core/widget.rs` — Added `as_input_projection_provider()` method
- `src/core/widget_tree.rs` — Added projection storage and methods
- `src/core/app.rs` — Added `post_layout_populate_input_projections()`
- `src/widgets/list.rs` — Implemented InputProjectionProvider
- `tests/input_projection_tests.rs` — 7 tests for projection behavior

---
