# SPEC-HAL-118

**Virtualized ListView & TreeView Contract**

**Status:** Deprecated
**Scope:** Virtualization architecture for large item collections

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.
**Applies to:** VirtualizedListView, VirtualizedTreeView, VirtualizedTable
**Non-Goals:** Selection semantics (SPEC-114), tree interaction (SPEC-116), column headers (SPEC-117), styling

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

## 1. Problem Statement

Virtualized views fail due to:

* selection state tied to widget instances
* scroll position instability on data changes
* item identity loss during recycling
* layout thrashing from eager instantiation
* incorrect viewport calculations

This SPEC defines a **strict virtualization contract** that separates logical state from widget realization, enabling unbounded item counts with constant memory usage.

---

## 2. Core Definitions

### 2.1 Logical Item Space

The **logical item space** is the complete set of items:

* indexed from `0..N-1`
* exists independently of viewport
* survives recycling and scroll

---

### 2.2 Realized Item

A **realized item** is a widget instance created for rendering:

* exists only while in or near viewport
* has no authority over state
* may be recycled

---

### 2.3 Viewport

The **viewport** is the visible region:

* defined by scroll offset and size
* determines which items are realized
* is the sole driver of realization

---

## 3. Viewport-Driven Realization

### INV-V1 — Realization Is Viewport-Driven

Item widgets are created and destroyed based solely on viewport visibility.

* items enter viewport → realize
* items exit viewport → recycle or destroy

No eager realization of off-screen items.

---

### INV-V2 — Overscan Buffer

Virtualization MAY realize items slightly outside the viewport (overscan):

* `overscan_before: usize` — extra items above/before viewport
* `overscan_after: usize` — extra items below/after viewport

Overscan smooths scrolling but MUST be bounded.

---

### INV-V3 — Visible Range Calculation

The visible index range is:

```
visible_start = floor(scroll_offset / item_height)
visible_end = ceil((scroll_offset + viewport_height) / item_height)
realized_start = max(0, visible_start - overscan_before)
realized_end = min(item_count, visible_end + overscan_after)
```

For variable-height items, calculation is more complex but principle remains.

---

## 4. Item Recycling

### INV-R1 — Recycling Is Mandatory

When items exit the realized range:

* widgets MUST be recycled, not destroyed
* recycled widgets are reused for newly visible items

Recycling avoids allocation churn.

---

### INV-R2 — Recycle Pool

A **recycle pool** holds unrealized widget instances:

* bounded size (e.g., 2× viewport capacity)
* FIFO or LRU eviction when full

---

### INV-R3 — Rebinding on Reuse

When a recycled widget is reused:

* it MUST be rebound to new item data
* previous state MUST be cleared
* identity MUST update to new item

No state leakage between items.

---

## 5. Item Identity

### INV-I1 — Stable Item Identity

Each logical item MUST have a stable identity:

* index-based (simple lists)
* OR explicit `ItemId` (mutable lists)

Identity MUST survive recycling.

---

### INV-I2 — Widget ≠ Item

Widget instance identity MUST NOT be conflated with item identity.

* same widget may represent different items over time
* same item may be represented by different widgets over time

---

## 6. Scroll ↔ Data Coupling

### INV-S1 — Scroll Anchor Preservation

On data changes (insert, remove, reorder):

* scroll position MUST anchor to a logical item + offset
* pixel-only anchoring is forbidden

---

### INV-S2 — Anchor Strategy

Supported anchor strategies:

1. **Top-item anchor**: maintain top visible item at same position
2. **Focus-item anchor**: maintain focused item at same position
3. **Selection anchor**: maintain selected item at same position

Strategy MUST be explicitly configured.

---

### INV-S3 — Content Height Tracking

Total content height MUST be tracked:

* for fixed-height: `height = item_count × item_height`
* for variable-height: cumulative height cache

Scroll range depends on this value.

---

## 7. Height Models

### INV-H1 — Supported Height Modes

Virtualized views MUST support at least one:

1. **Fixed Item Height**: all items same height, known upfront
2. **Variable Height (Measured)**: heights computed on realization, cached
3. **Variable Height (Estimated)**: estimated heights, refined on realization

---

### INV-H2 — Height Cache Invalidation

On data change:

* affected heights MUST be invalidated
* scroll position MUST be recalculated
* partial invalidation is allowed

---

## 8. Keyboard Navigation

### INV-K1 — Navigation Is Index-Based

Keyboard navigation operates on logical indices:

* `Up` / `Down` → index ± 1
* `PageUp` / `PageDown` → viewport-sized jump
* `Home` / `End` → first / last item

---

### INV-K2 — Scroll Follows Focus

Focus changes MUST auto-scroll to keep focused item visible.

Realization follows scroll, not focus directly.

---

## 9. Selection in Virtualized Context

### INV-SEL1 — Selection Is Index-Based

Selection state is expressed as indices (or ItemIds), not widget references.

Selection survives recycling.

---

### INV-SEL2 — Selection on Unrealized Items

Selection operations MUST work on unrealized items:

* `select(index)` — works even if item not realized
* `get_selection()` — returns indices, not widgets

---

## 10. TreeView Virtualization

### INV-T1 — Tree Is Flattened Projection

Virtualized TreeView operates on a **flattened projection**:

* hierarchical model → expansion state → flat index list
* virtualization operates on flat indices
* tree semantics (SPEC-116) apply to the model, not realization

---

### INV-T2 — Expansion Triggers Projection Update

On expand/collapse:

* projection recalculates
* affected index ranges update
* scroll anchor adjusts

---

## 11. Programmatic Control

### INV-PROG1 — Explicit APIs

Virtualized views MUST expose:

* `scroll_to_item(index)` — ensures item is visible
* `get_visible_range() -> Range<usize>`
* `get_realized_range() -> Range<usize>`
* `invalidate_heights(range)` — for variable-height views
* `set_item_count(count)` — update logical item count

---

## 12. Responsibility Boundaries

**VirtualizedListView/TreeView** owns:

* viewport calculation
* item realization decisions
* recycle pool management
* scroll anchor preservation
* height cache (variable-height mode)

**VirtualizedListView/TreeView** MUST NOT own:

* selection state (delegated per SPEC-HAL-114)
* tree expansion state (delegated per SPEC-HAL-116)
* column geometry (delegated per SPEC-HAL-117)
* item visual rendering (delegated to item painters)
* scroll mechanics (delegated to scroll overlay per SPEC-HAL-110)

**Realized item widgets** MUST NOT own:

* selection state
* identity (borrowed from logical item)
* position (assigned by virtualizer)

**Delegated responsibilities:**

* SPEC-HAL-110: scroll overlay and viewport contract
* SPEC-HAL-114: selection model
* SPEC-HAL-116: tree semantics (for VirtualizedTreeView)
* SPEC-HAL-117: column synchronization (for VirtualizedTable)

---

## 13. Performance Invariants

### INV-P1 — Constant Memory

Memory usage MUST NOT scale with item count.

Memory ∝ viewport size + overscan + recycle pool.

---

### INV-P2 — Constant Layout Cost

Layout cost per frame MUST NOT scale with item count.

Layout ∝ realized items only.

---

## 14. Forbidden Behavior

Virtualized views MUST NOT:

* realize all items eagerly
* tie selection to widget instances
* lose scroll position on data change
* allow unbounded recycle pool growth
* compute heights for unrealized items (except estimation)

---

## 15. Test Requirements

Any virtualized view implementation MUST include tests for:

* viewport-driven realization
* recycling correctness
* scroll anchor preservation on insert/remove
* selection persistence across recycling
* navigation to unrealized items
* height cache invalidation
* large item counts (10,000+)

---

## 16. Reference Implementation

Planned widgets: `VirtualizedListView`, `VirtualizedTreeView`

Related SPECs:

* SPEC-HAL-114 (List Selection — selection model)
* SPEC-HAL-116 (Tree Interaction — tree semantics)
* SPEC-HAL-117 (Columnar Views — header synchronization)
* SPEC-HAL-110 (Scroll Overlay — scroll semantics)

---

## 17. Final Note

Virtualization is a **projection of logic into viewport**, not a rendering optimization.

Any implementation where state depends on realization is **architecturally invalid**, regardless of performance characteristics.

---

---

## Implementation Notes (ListView v1)

The virtualization contract is implemented via the `List` widget (`src/widgets/list.rs`), which realizes a fixed pool of slot widgets based on viewport visibility. Selection, focus, and interaction states are keyed by `LogicalItemId`, surviving realization churn. Scroll anchoring preserves position on data changes.

---

## Implementation Reference

- `List` — virtualized list container with slot pool
- `VirtualizedSlot` — demo slot widget for virtualization testing
- `LogicalItemId` — stable item identity for selection/focus
- `ListInteractionModel` — selection and interaction state owner
- `visible_range()` / `realized_range()` — viewport-driven realization

---

## Validation Checklist

- [x] Scope is non-overlapping with other SPECs
- [x] Ownership boundaries are explicit
- [x] All cross-SPEC references are valid
- [x] Forbidden behaviors are explicitly listed
- [x] Behavior is mechanically testable
