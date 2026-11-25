# SPEC-HAL-118B: ListView — Columnar List Contract

**Status:** Deprecated
**Layer:** Widget (Tier-1 Composite)

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

**Depends on:**

* SPEC-HAL-010 (Widget System)
* SPEC-HAL-011 (Tier-1 Widgets)
* SPEC-HAL-079 (Unified Event Flow)
* SPEC-HAL-086 (Scroll and Viewport System)
* SPEC-HAL-114 (List and ListItem Selection Model)
* SPEC-HAL-118 / SPEC-HAL-118A (List / List v1 Scope)
* SPEC-HAL-135 / 135A / 135B / 136 (Selection Semantics)

**Does NOT depend on:**
Tree, hierarchy, expand/collapse, sorting, column resize, grouping, data models, view containers.

---

## 1. Purpose

This SPEC defines **ListView** as a **columnar, table-like view** built **strictly on top of List**.

ListView introduces **columns and headers** only.
It **does not** introduce hierarchy, collapse/expand, or new interaction semantics.

TreeView and hierarchical views are explicitly **out of scope** and will be layered later via projection.

---

## 2. Core Principle

> **ListView = List + column layout**
> **Nothing more. Nothing less.**

ListView is a *presentation and layout specialization*, not a behavioral widget.

---

## 3. Non-Goals (Hard Exclusions)

ListView **MUST NOT**:

* Introduce hierarchy or parent/child relationships
* Support expand/collapse
* Introduce column resizing
* Introduce sorting or grouping
* Modify selection semantics
* Own or duplicate selection state
* Own scroll state independently
* Introduce data models or adapters
* Perform policy decisions

Violations invalidate this SPEC.

---

## 4. Relationship to Other Widgets

| Widget                | Role                                                |
| --------------------- | --------------------------------------------------- |
| List                  | Owns selection, range logic, drag, virtualization   |
| ListView              | Defines columnar layout & headers                   |
| TreeView              | Future composition: ListView + hierarchy projection |
| Disclosure / Expander | Separate widget (future)                            |

---

## 5. Structural Definition

### 5.1 Widget Identity

* ListView is a **distinct widget**
* Internally owns exactly **one List instance**
* List identity and lifecycle remain authoritative

### 5.2 Structural Invariant

```
ListView
 ├── HeaderBand (optional, visual-only)
 ├── List       (authoritative, interactive)
 └── FooterBand (optional, visual-only)
```

### 5.3 Viewport Invariant (Binding)

**INV-VIEWPORT-1**: HeaderBand and FooterBand are **outside** the scrollable viewport.

* HeaderBand is fixed at the top of the ListView visible area.
* FooterBand is fixed at the bottom of the ListView visible area.
* Only List rows scroll.
* **ListView MUST NOT be wrapped by an external ScrollArea that scrolls the entire ListView.**
* **ListView MUST provide its own scroll/viewport substrate for the body region** (embedding ScrollArea internally, wrapping only the List body).
* This is a binding constraint, not an optional mode.

Violations of INV-VIEWPORT-1 produce undefined behavior.

### 5.4 Structural Invariant (Implementation)

```
ListView
 ├── HeaderBand (fixed, visual-only)
 ├── BodyViewport (ScrollArea, scrollable) → List (authoritative, interactive)
 └── FooterBand (fixed, visual-only)
```

* BodyViewport is the ScrollArea instance that receives wheel/scroll input.
* List is the direct child of BodyViewport.
* Scroll state remains owned by List (virtualization), but scroll input flows through BodyViewport.

---

## 6. Row Model

* ListView rows are **flat**
* One List item = one ListView row
* Row identity = List item identity
* Row order = List order

No transformation, filtering, or grouping is allowed.

---

## 7. Column Model

### 7.1 Column Definition

Columns are declared explicitly and immutably:

```rust
ListViewColumn {
    id: ColumnId,
    width: f32,
    cell_renderer: fn(row_id, column_id, rect, &mut PaintContext)
}
```

### 7.2 Column Invariants

* Column `id` is stable
* Column `width` is fixed (no resize)
* Column order is fixed
* Column renderer is pure (no state mutation)

---

## 8. Header Model

### 8.1 Header Row

* Header row is rendered by ListView
* Header height is fixed
* Header is **not selectable**
* Header does **not** participate in List selection

### 8.2 Scroll Behavior

* Vertical scroll: header is fixed
* Horizontal scroll (future): header scrolls with content

No sticky logic beyond this.

---

## 9. FooterBand Model

### 9.1 FooterBand — Binding Semantics

* FooterBand is **optional**
* FooterBand is **fixed-position**
* FooterBand **does not scroll**
* FooterBand is **not selectable**
* FooterBand **does not participate in hit-testing for List selection**
* FooterBand **must never mutate List state**
* FooterBand **must not own input capture**
* FooterBand rendering is **purely visual**
* FooterBand aligns **column geometry exactly** with header and body
* FooterBand may be used for:

    * summaries
    * totals
    * status indicators
* FooterBand is always realized (non-virtualized)

### 9.2 Explicit Prohibitions

FooterBand must not:

* affect selection
* affect focus
* affect scroll position
* affect drag / range selection
* introduce state
* participate in layout authority beyond its own rect

---

## 10. Layout Model

### 10.1 Vertical Layout

* Vertical layout is delegated entirely to List
* ListView **must not** compute row Y positions

### 10.2 Horizontal Layout

* Columns define horizontal segmentation
* Cell rect = row rect ∩ column rect

---

## 11. Rendering Model

Rendering proceeds in this order:

1. Header background
2. Header cells (per column)
3. List background
4. Rows (delegated to List)
5. Per-cell rendering via column renderers
6. Footer background
7. Footer cells (per column)

ListView **does not** render selection visuals directly.

---

## 12. Selection Semantics

### 12.1 Ownership

* Selection state is owned by List
* ListView forwards all input events unchanged

### 12.2 Guarantees

* Shift-click behavior identical to List
* Drag selection identical to List
* Transactional selection unchanged
* No column-specific selection

---

## 13. Input Routing

* Input routing follows SPEC-HAL-079
* Hit testing:

    * Row hit → List
    * Column awareness is layout-only
* Header receives pointer events but does not modify selection
* Footer receives pointer events but does not modify selection

---

## 14. Virtualization

* Virtualization is fully owned by List
* ListView must not:

    * Track visible ranges
    * Cache row geometry
    * Interfere with realization logic

Cells are rendered only for realized rows.

---

## 15. State Model

ListView owns **no persistent interaction state**.

Allowed internal state:

* Column definitions
* Header metrics
* Footer metrics

Forbidden:

* Selection state
* Scroll offsets
* Focus state
* Transaction state

---

## 16. API Surface (Conceptual)

```rust
ListView::new(
    list: List,
    columns: Vec<ListViewColumn>,
)
```

No mutation APIs for columns in v1.

---

## 17. Performance Constraints

* O(visible_rows × columns) rendering only
* No per-frame allocations proportional to total row count
* No additional hit-testing passes

---

## 18. Error Conditions

ListView must fail fast (debug) if:

* Column widths sum to NaN or negative
* Duplicate column IDs are provided
* Cell renderer panics

---

## 19. Future Extensions (Explicitly Deferred)

* Column resize → separate SPEC
* Sorting → separate SPEC
* Grouping → separate SPEC
* TreeView → separate SPEC
* Sticky headers → separate SPEC

No placeholders permitted in code.

---

## 20. Compliance Checklist

A ListView implementation is compliant iff:

* List selection behavior is unchanged
* List tests pass without modification
* No hierarchy exists
* No expand/collapse exists
* No policy logic exists
* Virtualization is untouched

---

## 21. Final Law

> **ListView is not a Tree.
> ListView is not a data model.
> ListView is not a policy engine.
> ListView is a columnar projection of List.**

Anything else is illegal.

---

## ListView v1 Binding Invariants

The following invariants are binding for all ListView v1 implementations:

**INV-LV-V1-WIDTH-1**: Column widths are owned exclusively by `ColumnStructureController`. ListView never mutates width directly.

**INV-LV-V1-MEASURE-1**: ListView NEVER fabricates cell content measurements. Header text is measured only because ListView renders it.

**INV-LV-V1-MEASURE-2**: Cell content measurement is owned by slots and reported upstream via `ColumnContentWidthSink`.

**INV-LV-V1-AUTOSIZE-1**: Auto-size width derives from ACTUAL rendered content, not estimates or fabricated samples.

**INV-LV-V1-AUTOSIZE-2**: Auto-size actions emit directives (`ColumnStructureDirective`), never mutate state directly.

**INV-LV-V1-CLICK-1**: Multi-click behavior is recipient-aware. Double-click on separator triggers auto-size. Higher arity is not suppressed.

**INV-LV-V1-FOCUS-1**: Modifier state is reset on window focus loss.

---

## Implementation Notes (ListView v1)

ListView is implemented as a composite widget with three child containers: `ListViewHeaderContainer`, `ListViewContentContainer` (wrapping ScrollArea → List), and `ListViewFooterContainer`. Column geometry is computed once per frame and shared across all bands. Header/footer remain fixed while body scrolls vertically.

---

## Implementation Reference

- `ListView` — main columnar list composite widget
- `ListViewHeaderContainer` — fixed header band container
- `ListViewContentContainer` — scrollable body region (owns ScrollArea)
- `ListViewFooterContainer` — fixed footer band container
- `ComputedColumnGeometry` — per-frame column layout snapshot
- `ColumnDef` — column definition (id, width, header, visibility)

---

**End of SPEC-HAL-118B**
