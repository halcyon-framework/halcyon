# SPEC-HAL-158B: Dirty Authority Contract Resolution

**Status:** Binding
**Depends on:** SPEC-HAL-158 (Layout Invalidation Law)
**Addresses:** Audit findings from Widget Dirty-Marking Audit (2025-01)

---

## Problem Statement

SPEC-HAL-158 establishes that layout reuse is gated by:
1. `dirty_layout == false`
2. `last_constraints.is_some()`
3. `constraints == last_constraints`

This gate is **correct and release-grade**.

However, audit revealed that multiple widgets expose runtime setters that mutate layout-affecting state **without propagating `mark_layout_dirty()` to the tree**.

If a caller mutates layout-affecting state and constraints happen to be identical, the reuse gate passes, and **stale cached geometry is returned**.

This is a silent correctness bug.

---

## Affected Widgets

### Category A — Architectural Violations (Must Be Fixed)

| Widget | Setter | File:Line | Issue |
|--------|--------|-----------|-------|
| TextField | `set_text()` | text_field.rs:229-236 | Text length affects measured width; no tree dirty |
| TextArea | `set_text()` | text_area.rs:355-363 | Multiline height depends on text; no tree dirty |

### Category B — Undefined Contract (Needs Decision)

| Widget | Setter | File:Line | Issue |
|--------|--------|-----------|-------|
| List | `set_item_count()` | list.rs:807-820 | Changes total content height |
| List | `set_item_height()` | list.rs:854-861 | Changes row height |
| List | `set_visible_range()` | list.rs:832-837 | Changes realized widget count |
| List | `set_viewport_height()` | list.rs:873-876 | Changes visible range computation |

### Category C — Test-Only Escapes (Acceptable If Documented)

| Widget | Setter | File:Line | Current Use |
|--------|--------|-----------|-------------|
| ScrollArea | `set_viewport_size()` | scroll_area.rs:310-312 | Test setup |
| ScrollArea | `set_content_size()` | scroll_area.rs:317-319 | Test setup |

---

## Design Decisions

### Rule 1: TextField / TextArea — Context-Required Setters

**Decision:** Setters that mutate intrinsic size MUST take `NodeId` and `&mut WidgetTree` (or equivalent context) and call `mark_layout_dirty()` internally.

**Rationale:**
- These widgets own their text content
- Text content directly determines layout size
- Callers cannot reliably know to dirty externally
- The widget is the authority; the widget must enforce

**Contract:**
```
pub fn set_text(&mut self, text: impl Into<String>, tree: &mut WidgetTree, node_id: NodeId)
```

Or split pattern:
```
// Internal: no dirty (for undo/redo paths that batch)
fn set_text_internal(&mut self, text: impl Into<String>)

// Public: always dirties
pub fn set_text(&mut self, text: impl Into<String>, tree: &mut WidgetTree, node_id: NodeId)
```

**Invariant (INV-DIRTY-TEXT-1):**
> Any mutation to `TextField.state.document.text` or `TextArea.state.document.text` via public API MUST propagate `mark_layout_dirty()` to the widget tree.

---

### Rule 2: List — Widget-Owned Model, Widget-Enforced Dirty

**Decision:** List setters are part of the widget's **model**, not external controller state. The widget MUST enforce dirty propagation.

**Rationale:**
- `item_count` and `item_height` are intrinsic to List's layout computation
- Callers use these setters to configure the List; they should not need to know tree internals
- Leaving invalidation to callers creates an implicit contract that is never enforced

**Contract:**
```
pub fn set_item_count(&mut self, count: usize, tree: &mut WidgetTree, node_id: NodeId)
pub fn set_item_height(&mut self, height: f32, tree: &mut WidgetTree, node_id: NodeId)
```

**Exception — `set_visible_range()` and `set_viewport_height()`:**
These are derived/computed during layout, not caller-set in normal operation. They MAY remain internal (no tree param) if:
1. They are marked `pub(crate)` or `#[doc(hidden)]`
2. Documentation states they are for internal/test use only

**Invariant (INV-DIRTY-LIST-1):**
> Any mutation to `List.item_count` or `List.item_height` via public API MUST propagate `mark_layout_dirty()` to the widget tree.

---

### Rule 3: ScrollArea — Test-Only Classification

**Decision:** ScrollArea setters `set_viewport_size()` and `set_content_size()` are **test-only escapes**.

**Rationale:**
- In production, layout() computes and sets these values from constraints
- These setters exist for unit tests that need to configure ScrollArea without full layout cycles
- Production code paths do not call these setters

**Contract:**
```rust
/// Test-only: Set viewport size without layout cycle.
///
/// # Contract
/// This method does NOT propagate dirty to the widget tree.
/// Callers MUST ensure layout will run before next paint.
/// Production code MUST NOT call this method.
#[cfg(test)]
pub fn set_viewport_size(&mut self, size: Size)

/// Test-only: Set content size without layout cycle.
///
/// # Contract
/// This method does NOT propagate dirty to the widget tree.
/// Callers MUST ensure layout will run before next paint.
/// Production code MUST NOT call this method.
#[cfg(test)]
pub fn set_content_size(&mut self, size: Size)
```

**Alternative (if needed in production for dynamic content):**
Expose context-requiring versions:
```rust
pub fn set_content_size(&mut self, size: Size, tree: &mut WidgetTree, node_id: NodeId)
```

**Invariant (INV-DIRTY-SCROLL-1):**
> `ScrollArea.set_viewport_size()` and `ScrollArea.set_content_size()` are test-only APIs. Production code MUST NOT call them.

---

## Summary of Rules

| Widget | Setter Type | Rule |
|--------|-------------|------|
| TextField | `set_text` | MUST take tree context and dirty |
| TextArea | `set_text` | MUST take tree context and dirty |
| List | `set_item_count`, `set_item_height` | MUST take tree context and dirty |
| List | `set_visible_range`, `set_viewport_height` | Internal/test-only, no public dirty contract |
| ScrollArea | `set_viewport_size`, `set_content_size` | Test-only (`#[cfg(test)]`) |

---

## Enforcement

1. **Mechanical:** Category A/B fixes make the dirty call unconditional at the setter level. No caller discipline required.

2. **Compile-time:** Category C uses `#[cfg(test)]` to prevent accidental production use.

3. **Review:** Any new setter that mutates layout-affecting fields MUST follow these patterns.

---

## Non-Goals

- This SPEC does not change layout reuse logic (that is correct)
- This SPEC does not add runtime assertions (the fix is structural)
- This SPEC does not address event-driven mutations (handle_event paths have EventContext and can dirty via `cx.invalidate_layout()`)

---

## Implementation Order

1. TextField `set_text()` — add tree context, propagate dirty
2. TextArea `set_text()` — add tree context, propagate dirty
3. List `set_item_count()`, `set_item_height()` — add tree context, propagate dirty
4. ScrollArea setters — add `#[cfg(test)]` gate
5. Update call sites (likely minimal; most mutations happen in event handlers with context)
