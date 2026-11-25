# SPEC-HAL-117

**Columnar Views, Headers & Resize Contract**

**Status:** Deprecated
**Scope:** Column layout, header interaction, and column resizing

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.
**Applies to:** ColumnHeader, ColumnView, TableHeader
**Non-Goals:** Tree semantics (SPEC-116), selection logic (SPEC-114), virtualization (SPEC-118), sorting UI, styling

---

## 1. Problem Statement

Columnar views fail due to:

* inconsistent header hit-testing
* broken resize drag behavior
* column width drift between header and content
* unclear ownership of column geometry
* resize handle ambiguity at column boundaries

This SPEC defines a **strict contract** for column headers, column geometry ownership, and resize interaction.

---

## 2. Core Definitions

### 2.1 Column

A **Column** is a vertical subdivision of a columnar view:

* has a stable identity (`ColumnId`)
* has a width
* has optional header content

---

### INV-COL-ID — Column Identity Stability

`ColumnId` MUST be stable across:

* resize operations
* scroll operations
* reorder operations (if later added)
* layout passes

Width changes MUST NOT reassign or regenerate column identity.

`ColumnId` is immutable for the lifetime of the column definition.

---

### 2.2 ColumnHeader

A **ColumnHeader** is the interactive header row above columns:

* displays column labels
* provides resize handles
* may provide sort indicators (visual only, logic out of scope)

---

### 2.3 ColumnView

A **ColumnView** is a widget that arranges content in columns:

* owns column definitions
* synchronizes header and content column widths
* does NOT own row data or selection

---

### 2.4 Responsibility Boundaries

**ColumnHeader** is responsible for:

* hit-testing within header row
* resize handle interaction
* cursor feedback on hover
* sort indicator display (visual only)

**ColumnView** is responsible for:

* column geometry (widths, order, constraints)
* scroll synchronization between header and content
* layout coordination

**Row widgets** MUST be geometry-agnostic:

* MUST NOT store column widths
* MUST NOT cache column positions
* MUST query ColumnView for all geometry on every layout/paint

---

## 3. Column Geometry Ownership

### INV-C1 — Single Source of Column Widths

Column widths MUST be defined in exactly one place:

* `columns: Vec<ColumnDef>` where `ColumnDef { id, width, min_width, max_width }`

Header and content rows MUST read from this single source.

---

### INV-C2 — Width Synchronization

Column widths in header and content MUST be identical.

No drift allowed. Any width change updates both simultaneously.

---

### INV-C3 — Width Constraints

Each column MAY define:

* `min_width: f32` — minimum allowed width
* `max_width: Option<f32>` — maximum allowed width (None = unbounded)

Resize operations MUST respect constraints.

---

### INV-C4 — Column Geometry Ownership

Column width, order, and constraints MUST be owned by ColumnView.

Row widgets:
- MUST NOT store column widths
- MUST NOT cache column geometry
- MUST read column geometry from ColumnView on every layout and paint pass

---

## 4. Header Hit-Testing

### INV-H1 — Header Zones

The header row contains multiple hit zones:

1. **Column Label Zone**: the content area of each header cell
2. **Resize Handle Zone**: the boundary between adjacent columns

---

### INV-H2 — Resize Handle Geometry

The resize handle is a narrow vertical strip at column boundaries:

* width: platform-dependent (typically 4-8 pixels)
* centered on the column boundary
* extends full header height

---

### INV-H3 — Resize Handle Priority

Resize handle hit-testing takes priority over column label hit-testing.

If pointer is within resize handle zone, resize behavior activates.

---

### INV-H3a — Resize Handle Exclusivity

Resize handle zones:

* MUST NOT overlap with each other
* MUST have deterministic boundaries at pixel level
* MUST take priority over all label hit-zones
* MUST NOT leave gaps between adjacent handles and labels

At any pixel coordinate, exactly one hit zone is active.

---

### INV-H4 — Cursor Feedback

When pointer enters resize handle zone:

* cursor MUST change to resize cursor (e.g., `col-resize`)

When pointer leaves resize handle zone:

* cursor MUST revert to default

---

## 5. Resize Drag Contract

### INV-R1 — Resize Initiation

Resize begins on:

* pointer press within resize handle zone

Resize affects the column to the **left** of the handle.

---

### INV-R2 — Resize Drag Behavior

During resize drag:

* column width updates continuously
* minimum/maximum constraints are enforced
* content reflows in real-time (or on release, if configured)

---

### INV-R3 — Resize Completion

Resize ends on:

* pointer release
* pointer cancel
* escape key (reverts to original width)

---

### INV-R4 — Resize Event Emission

During resize:

* `on_resize_start(column_id, initial_width)`
* `on_resize(column_id, new_width)` — continuous or throttled
* `on_resize_end(column_id, final_width)`

---

### INV-R5 — Constraint Clamping

If drag would violate min/max constraints:

* width clamps to constraint boundary
* pointer may move beyond, but width stays clamped
* no "rubber band" behavior

---

### INV-R6 — Resize During Scroll Interaction

During active resize drag:

* Horizontal scroll MUST be frozen (content does not scroll while resizing)
* Vertical scroll MAY continue (rows may scroll vertically during resize)
* Scroll offset at resize start is preserved

On resize end:

* Horizontal scroll is unfrozen
* Scroll offset remains as it was at resize start
* No automatic scroll adjustment based on new column width

---

### INV-R7 — Header Height Stability

Header row height MUST NOT change during resize operations.

* Resize affects column width only
* Header height is fixed
* No vertical reflow during resize

---

## 6. Double-Click to Auto-Size (Optional)

### INV-A1 — Auto-Size Trigger

Double-click on resize handle MAY:

* resize column to fit content
* resize to default width

Behavior MUST be explicitly configured.

---

## 7. Column Reordering (Out of Scope)

Column drag-to-reorder is out of scope for this SPEC.

If implemented, it requires a separate SPEC.

---

## 8. Header Click Behavior

### INV-H5 — Label Click Does Not Resize

Click on column label zone (not resize handle):

* does NOT initiate resize
* MAY trigger sort (visual feedback only; sort logic is application responsibility)
* MAY trigger selection of column (if applicable)

---

## 9. Horizontal Scrolling Interaction

### INV-S1 — Header Scrolls with Content

When content scrolls horizontally:

* header MUST scroll in sync
* column alignment MUST remain perfect

---

### INV-S2 — Vertical Scroll Independence

When content scrolls vertically:

* header MUST remain fixed at top
* header does NOT scroll vertically

---

### INV-S3 — Exact Scroll Offset Synchronization

Header horizontal scroll offset MUST equal content horizontal scroll offset exactly.

* No rounding divergence allowed
* No delayed synchronization allowed
* No fractional pixel discrepancy allowed
* Offset synchronization MUST occur in the same frame as content scroll

---

## 10. Programmatic Control

### INV-PROG1 — Explicit APIs

ColumnView MUST expose:

* `set_column_width(column_id, width)`
* `get_column_width(column_id) -> f32`
* `set_column_constraints(column_id, min, max)`
* `auto_size_column(column_id)` — if auto-size is supported

---

## 11. Disabled State

### INV-D1 — Disabled Headers Are Inert

When column headers are disabled:

* no resize interaction
* no click interaction
* cursor does not change

Content may still be visible and scrollable.

---

## 12. Forbidden Behavior

ColumnView MUST NOT:

* allow header and content column widths to diverge
* resize on hover
* ignore min/max constraints
* resize wrong column (must resize left column of handle)
* allow resize handle to be unreachable

Row-level column width caching is forbidden.

The following are explicitly forbidden:

* Per-row width storage or caching
* Implicit width inference from content
* Layout-driven resize (content MUST NOT cause column resize)
* Delayed scroll synchronization between header and content
* Fractional pixel divergence in column boundaries
* Hit-zone gaps or overlaps in header
* Auto-column creation based on data
* Content-driven column inference
* Dynamic column addition without explicit API call

---

## 13. Future Extensions (Out of Scope)

The following features are explicitly OUT OF SCOPE and require separate SPECs:

* **Grouped Headers** — hierarchical column headers require SPEC-HAL-XXX
* **Column Reordering** — drag-to-reorder columns requires SPEC-HAL-XXX
* **Sorting Logic** — sort implementation is application responsibility; this SPEC defines visual indicators only
* **Column Filtering** — filter UI and logic require separate SPEC
* **Column Freezing/Pinning** — frozen columns require separate SPEC

Any implementation of these features without corresponding SPEC is FORBIDDEN.

---

## 14. Test Requirements

Any ColumnView implementation MUST include tests for:

* resize handle hit-testing
* resize drag with constraints
* escape to cancel resize
* header/content width synchronization
* horizontal scroll synchronization
* cursor feedback on handle hover
* double-click auto-size (if supported)
* disabled state

---

## 15. Reference Implementation

Planned widgets: `ColumnHeader`, `ColumnView`, `TableHeader`

Related SPECs:

* SPEC-HAL-114 (List Selection — row interaction)
* SPEC-HAL-116 (Tree Interaction — hierarchical content)
* SPEC-HAL-118 (Virtualization — large datasets)

---

## 16. Final Note

Column headers are **geometry authorities**, not decorations.

Any implementation where header and content column widths can diverge is **incorrect by definition**.

---

## Validation Checklist

- [ ] Scope is non-overlapping with other SPECs
- [ ] Ownership boundaries are explicit
- [ ] All cross-SPEC references are valid
- [ ] Forbidden behaviors are explicitly listed
- [ ] Behavior is mechanically testable
