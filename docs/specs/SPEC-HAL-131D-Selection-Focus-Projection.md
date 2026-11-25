# SPEC-HAL-131D: Selection & Focus Projection

**Status:** Implemented
**Layer:** State management (virtualization bridge)
**Scope:** Selection and focus ownership via LogicalItemId
**Depends on:** SPEC-HAL-131C (Input Projection)
**Constrains:** List selection, focus state, widget rendering

---

## SECTION A — Problem Statement

SPEC-HAL-131C established input projection for virtualization: hit-testing operates on logical identity, not realized widgets.

What was missing is **state projection**:

* Selection was bound to `usize` indices
* Focus was bound to `usize` indices
* State could become stale when items were unrealized
* Widgets stored selection/focus, violating virtualization principles

These behaviors require selection and focus to operate on **LogicalItemId**, not indices or widget identity.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Identity-based state**
Selection and focus bind to LogicalItemId, not indices or widget references.

**PRINCIPLE-2: State survives realization**
Selection/focus survive unrealize → re-realize cycles unchanged.

**PRINCIPLE-3: Widgets reflect, never store**
Realized widgets derive visual state from authoritative state; they never own it.

**PRINCIPLE-4: No implicit realization**
State changes never trigger realization or layout.

---

## SECTION C — Terminology

**LogicalItemId**
A stable identifier for a logical item (from SPEC-HAL-131C).

**Selection State**
The set of currently selected LogicalItemIds.

**Focus State**
The currently focused LogicalItemId (if any).

**Visual State**
Derived state for rendering (selected, focused, disabled).

---

## SECTION D — Invariants

### INV-STATE-IDENTITY-1: Identity Binding
Selection and focus bind to LogicalItemId, never widget identity.

### INV-STATE-SURVIVAL-1: Realization Survival
Selection/focus survive realization and unrealization cycles.

### INV-REALIZATION-VIEW-1: Read-Only Widgets
Realized widgets reflect state; they never store it.

### INV-NO-IMPLICIT-REALIZE-1: No Implicit Work
Selection/focus changes must NOT trigger realization.

---

## SECTION E — Architecture

### E.1 ListState Type (Updated)

```rust
pub struct ListState {
    /// Focused item bound to LogicalItemId (INV-STATE-IDENTITY-1)
    pub focused_id: Option<LogicalItemId>,
    /// Selected items bound to LogicalItemId (INV-STATE-IDENTITY-1)
    pub selected_ids: HashSet<LogicalItemId>,
    /// Anchor for range selection bound to LogicalItemId
    pub anchor_id: Option<LogicalItemId>,
    pub selection_mode: SelectionMode,
    pub disabled: bool,
}
```

### E.2 Index ↔ LogicalItemId Conversion

```rust
impl ListState {
    /// Convert index to LogicalItemId
    pub fn index_to_id(index: usize) -> LogicalItemId {
        LogicalItemId::new(index as u64)
    }

    /// Convert LogicalItemId to index
    pub fn id_to_index(id: LogicalItemId) -> usize {
        id.0 as usize
    }

    /// Query by index (convenience)
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_ids.contains(&Self::index_to_id(index))
    }

    pub fn is_focused(&self, index: usize) -> bool {
        self.focused_id == Some(Self::index_to_id(index))
    }
}
```

### E.3 Visual State Derivation

```rust
impl List {
    /// Derive visual state for rendering
    /// INV-REALIZATION-VIEW-1: Read-only derivation
    pub fn item_visual_state(&self, index: usize) -> ListItemVisualState {
        ListItemVisualState {
            selected: self.state.is_selected(index),
            focused: self.focused && self.state.is_focused(index),
            disabled: self.state.disabled,
        }
    }
}
```

---

## SECTION F — State Flow

### F.1 Input → State

1. Click/keyboard event provides index
2. Index converted to LogicalItemId
3. State updated with LogicalItemId
4. INV-NO-IMPLICIT-REALIZE-1: No realization triggered

### F.2 State → Visual

1. Widget queries `is_selected(index)`, `is_focused(index)`
2. Methods convert index to LogicalItemId internally
3. Return derived boolean for rendering
4. INV-REALIZATION-VIEW-1: No state stored in widget

---

## SECTION G — Tests

Required tests (all passing):

1. **Select unrealized item** — state binds to LogicalItemId
2. **Scroll away and back** — state preserved across realization churn
3. **Focus survives churn** — focus stable under repeated scrolling
4. **Multi-selection survives** — multiple selections survive churn
5. **No realization on state change** — visible_range unchanged
6. **LogicalItemId stable** — identity equality works correctly
7. **Visual state queries** — derived correctly from authoritative state
8. **Index ↔ ID roundtrip** — conversion is lossless
9. **Empty state** — handles None correctly
10. **Range selection** — uses LogicalItemId internally

---

## SECTION H — Acceptance Criteria

SPEC-HAL-131D is satisfied when:

* [x] ListState uses LogicalItemId for focused_id
* [x] ListState uses HashSet<LogicalItemId> for selected_ids
* [x] ListState uses LogicalItemId for anchor_id
* [x] Index ↔ LogicalItemId conversion helpers added
* [x] List widget derives visual state from authoritative state
* [x] Selection/focus survive realization churn
* [x] State changes do not trigger realization
* [x] All 10 tests passing

---

## SECTION I — Authoritative Statements

**AUTH-131D-1: "Selection and focus bind to LogicalItemId, not widget identity."**

**AUTH-131D-2: "State survives the realized/unrealized boundary."**

**AUTH-131D-3: "Widgets reflect state; they never store it."**

**AUTH-131D-4: "State changes are read-only with respect to virtualization."**

---

## SECTION J — Files Modified

- `src/widgets/list_state.rs` — Updated to use LogicalItemId
- `src/widgets/list.rs` — Updated to derive visual state
- `tests/list_state_tests.rs` — Updated for new API
- `tests/list_widget_tests.rs` — Updated for new API
- `tests/selection_focus_projection_tests.rs` — New tests for projection
- `docs/SPECs/SPEC-HAL-131D-Selection-Focus-Projection.md` — This document

---
