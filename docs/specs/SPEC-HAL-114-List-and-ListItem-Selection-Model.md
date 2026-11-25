# SPEC-HAL-114

**List & ListItem Selection Model**

**Status:** Design — Approved
**Scope:** Non-virtualized, non-columnar list selection and interaction
**Applies to:** List, ListItem (non-virtual, non-columnar)
**Non-Goals:** Virtualization (SPEC-HAL-118), columnar layout (SPEC-HAL-117), tree semantics (SPEC-HAL-116)

---

## 1. Problem Statement

Simple list widgets fail due to:

* unclear row ownership and hit-testing
* ad-hoc selection state management
* inconsistent keyboard focus vs selection
* undefined empty-space click behavior
* selection logic entangled with rendering

This SPEC defines a **strict selection and interaction model** for non-virtualized, non-columnar List widgets, providing the foundation upon which columnar and virtualized variants are built.

---

## 2. Core Definitions

### 2.1 List

A **List** is a linear, ordered collection of items:

* index-addressable (`0..N-1`)
* non-hierarchical
* non-columnar
* non-virtualized (all items exist in widget tree)

---

### 2.2 ListItem

A **ListItem** is a single selectable entry within a List:

* owns a single row rect
* participates in selection
* may contain arbitrary content

---

## 3. Row Rect Authority

### INV-L1 — Single Authoritative Row Rect

Each ListItem MUST have exactly one authoritative rect:

```
row_rect = { x, y, width, height }
```

This rect MUST be used identically for:

* painting
* hit-testing
* hover detection
* selection bounds

No secondary rect calculations allowed.

---

### INV-L2 — Row Rect Stability

Row rects MUST be stable between layout passes.

Rect changes require explicit layout invalidation.

---

## 4. Focus and Selection State

### 4.1 State Definitions

### INV-STATE1 — Explicit State Types

List MUST maintain two independent state values:

* `focused_index: Option<RowIndex>` — keyboard navigation position
* `selected_indices: Set<RowIndex>` — currently selected rows

These states are **independent**. Focus does not imply selection. Selection does not imply focus.

---

### 4.2 Selection Ownership

### INV-SEL1 — Selection Lives in List

Selection state MUST be owned by the List widget, not individual ListItems.

ListItems are projections of selection state, not owners.

---

### 4.3 Selection Modes

### INV-SEL2 — Explicit Selection Mode

Each List MUST declare exactly one selection mode:

1. **None** — no selection allowed
2. **Single** — at most one selected item
3. **Multi** — multiple selected items

Mode is immutable at runtime.

---

### INV-SEL3 — Index-Based Selection

Selection MUST be expressed as indices, not pointers or widget references.

* `selection: Option<usize>` (single mode)
* `selection: Set<usize>` (multi mode)

---

### 4.4 Selection Invariants

### INV-SEL4 — Selection Only Via Explicit Action

Selection changes occur only via:

* pointer click
* keyboard navigation with selection modifier
* programmatic API

Layout, scrolling, and focus changes MUST NOT mutate selection.

---

## 5. Pointer Interaction

### INV-P1 — Click Selects Row

Single click on a ListItem:

* clears previous selection (unless modifier held)
* selects clicked item
* sets focus to clicked item

---

### INV-P2 — Modifier Behavior

* **Shift + Click**: extends selection from anchor to clicked item (multi mode)
* **Ctrl/Cmd + Click**: toggles selection of clicked item (multi mode)

In single mode, modifiers have no effect.

---

### INV-P3 — Activation Timing

Selection occurs on pointer release, not press.

* press inside → release inside → select
* press inside → release outside → cancel

Follows SPEC-HAL-112 timing principles.

---

## 6. Empty-Space Click Semantics

### INV-E1 — Empty-Space Click Clears Selection (Single Mode)

Clicking empty space (below all items, or in padding):

* MUST clear selection in Single mode
* MAY clear selection in Multi mode (configurable)

---

### INV-E2 — Empty-Space Does Not Focus

Clicking empty space MUST NOT change the focused item index.

---

### INV-E3 — Empty-Space Does Not Select Last Row

Clicking empty space MUST NOT implicitly select the last row or any row.

Empty space is not a proxy for any item.

---

### INV-E4 — Empty-Space Modifier Behavior

Modifier keys during empty-space click:

* **Shift + Click on empty space**: no effect (MUST NOT extend selection to empty space)
* **Ctrl/Cmd + Click on empty space**: no effect (MUST NOT toggle anything)

Modifiers are ignored when clicking empty space.

---

## 7. Keyboard Focus Model

### INV-K1 — Focus Row Is Distinct from Selection

The **focus row** is a single index indicating keyboard navigation position.

* Focus row may differ from selection
* Focus row is always a valid index (or none if list is empty)

---

### INV-K1a — Single Focused Row Invariant

At most one row may have focus at any time.

* Focus MUST be `None` or exactly one valid index
* Multiple focus states are FORBIDDEN

---

### INV-K1b — Focus Movement Rules

Focus movement differs by input source:

**Mouse:**
* Click on row → focus moves to clicked row
* Click on empty space → focus unchanged

**Keyboard:**
* Arrow keys → focus moves per navigation rules
* Tab into List → focus moves to first item (or last selected, if configured)
* Tab out of List → focus unchanged internally

---

### INV-K1c — Empty List Focus Behavior

When list has zero items:

* `focused_index` MUST be `None`
* All navigation keys are no-op
* Tab into List focuses nothing

---

### INV-K2 — Navigation Keys

Required keys:

* **Up / Down** — move focus to previous / next item
* **Home / End** — move focus to first / last item
* **Space** — toggle selection of focused item (multi mode)
* **Enter** — activate focused item (MUST NOT change selection)

---

### INV-K2a — Enter MUST NOT Change Selection

Enter key activates the focused item but MUST NOT:

* add focused item to selection
* clear selection
* modify selection in any way

Activation and selection are orthogonal operations.

---

### INV-K3 — Navigation Without Modifier

Arrow navigation without modifiers:

* moves focus
* clears selection
* selects focused item

---

### INV-K4 — Navigation With Shift

Arrow navigation with Shift:

* moves focus
* extends selection from anchor to focus (multi mode)

---

## 8. Anchor Model

### INV-A1 — Selection Anchor

The **anchor** is the index from which range selection extends.

* Set on click or explicit selection
* Used for Shift+Click and Shift+Arrow range extension
* Persists until next non-range selection action

---

## 9. Focus Independence

### INV-F1 — Focus Does Not Imply Selection

Receiving keyboard focus MUST NOT change selection.

---

### INV-F2 — Focus Loss Preserves Selection

Losing focus MUST NOT clear selection.

Selection persists with inactive visual state.

---

## 10. Programmatic Control

### INV-PROG1 — Explicit APIs

List MUST expose:

* `select(index)` / `select_range(start, end)`
* `deselect(index)` / `clear_selection()`
* `get_selection() -> &[usize]`
* `set_focus(index)`
* `get_focus() -> Option<usize>`

---

### INV-PROG2 — No Event Emission on Programmatic Change

Programmatic selection changes MUST NOT emit user selection events.

---

## 11. Responsibility Boundaries

**List** owns:

* selection state (`selected_indices`)
* focus state (`focused_index`)
* selection mode enforcement
* keyboard navigation logic

**List** MUST NOT own:

* visual rendering of items (delegated to ListItem painters)
* row layout calculations (delegated to layout system)
* scroll mechanics (delegated to scroll container)
* item content (delegated to application)

**ListItem** owns:

* single row rect
* hit-test response for its rect

**ListItem** MUST NOT own:

* selection state
* focus state
* interaction with other items

**Delegated responsibilities:**

* Layout system: determines item rects
* Scroll container: handles overflow
* Application: provides item data and content

---

## 12. Disabled State

### INV-D1 — Disabled List Is Inert

When disabled:

* no selection changes
* no focus changes
* no keyboard navigation
* visual state reflects disabled

---

## 13. Forbidden Behavior

List MUST NOT:

* let ListItems own selection state
* change selection on focus gain/loss
* change selection during paint
* compute different rects for paint vs hit-test
* implement columnar semantics
* implement virtualization
* auto-select on focus gain
* implicitly select on scroll
* mutate selection on layout changes
* select items as side effect of any non-selection operation

The following are explicitly forbidden in this SPEC:

- Column headers or multi-column layout
- Item virtualization or recycling
- Hierarchical indentation or parent/child relationships
- Hover-based selection
- Time-based selection (no auto-select after delay)
- Selection changes during programmatic scroll

---

## 14. Test Requirements

Any List implementation MUST include tests for:

* single-click selection
* Shift+click range selection
* Ctrl+click toggle selection
* keyboard navigation
* empty-space click behavior
* focus vs selection independence
* selection persistence on focus loss
* disabled state

---

## 15. Reference Implementation

Planned widget: `List`, `ListItem`

Related SPECs:

* SPEC-HAL-111 (Checkbox & RadioButton — toggle/selection semantics reference)
* SPEC-HAL-112 (Button Interaction Contract — timing principles)
* SPEC-HAL-116 (Tree & TreeItem — hierarchical extension)
* SPEC-HAL-117 (Columnar Views — column extension)
* SPEC-HAL-118 (Virtualization — performance extension)

---

## 16. Final Note

A List is a **selection state machine**, not a container of clickable rows.

Any List implementation where selection logic leaks into ListItems is **incorrect by definition**.

---

---

## 17. Virtualization Extension Stages

SPEC-HAL-114 extends to support list virtualization through the following stages:

### Stage 2 — ListState Binding

Binds the ListState reducer to the real List widget. Selection and focus state are owned by List.

### Stage 3.1 — Visual State Provider Wiring

ListItem receives visual state (selected, focused, disabled) via provider callback, not storage.

### Stage 4 — Virtualization

Logical `item_count` may exceed instantiated widgets. Only `visible_range` items are laid out.
Selection and focus operate on logical indices independent of visible widgets.

### Stage 5 — Scroll-Driven Visible Range

`update_visible_range_from_scroll(scroll_y)` maps scroll offset to visible_range.
Formula: `first = floor(scroll_y / item_height)`, `count = ceil(viewport_height / item_height) + 1`.

### Stage 6 — Focus Auto-Reveal

`desired_scroll_for_focus()` computes scroll intent for off-screen focus.
Pure function—no side effects, no mutations.

### Stage 7 — Scroll Application Wiring

`on_scroll_request` callback fires exactly once per focus change requiring scroll.
One-way wiring: List computes intent, caller executes scroll.

### Stage 8 — ScrollArea ↔ List Integration

`ScrollArea::scroll_to_y(y)` allows programmatic scroll.
Tests verify full integration: focus → scroll → visible_range update.

### Stage 9 — Spatial Layout Realization

**Problem:** Virtualized ListItem widgets all laid out at y=0, causing overlap.

**Invariant:** For each visible child at slot index `i`:
```
child.y = i * item_height
```

Where `logical_index = visible_range.start + i`.

**Implementation:** In `List::layout()`, when `item_height > 0` (virtualized mode):
- Position each child at `widget_idx * item_height`
- Non-virtualized lists (`item_height <= 0`) fall back to accumulated height

**Result:** Proper vertical list layout, items recycle while scrolling, no overlap.

---

## Validation Checklist

- [ ] Scope is non-overlapping with other SPECs
- [ ] Ownership boundaries are explicit
- [ ] All cross-SPEC references are valid
- [ ] Forbidden behaviors are explicitly listed
- [ ] Behavior is mechanically testable
