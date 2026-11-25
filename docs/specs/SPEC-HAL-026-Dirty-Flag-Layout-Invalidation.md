# **SPEC-HAL-026: Dirty Flag System & Layout Invalidation Model**

**Author:** JC
**Date:** 2025-11-24
**Status:** Final
**Scope:** Core Layout System
**Purpose:** Define the precise rules for when widgets must re-layout, how dirty flags propagate through the tree, and how size changes trigger parent invalidation.
**Extended by:** SPEC-HAL-354, which separates paint-only invalidation from layout-and-paint invalidation and forbids using layout dirtiness as a generic value-refresh signal.

---

## 1. Purpose

Halcyon requires deterministic, high-performance layout behavior.
This SPEC defines the **dirty flag system** and the **rules for invalidation propagation** in the widget tree.

Goals:

* Ensure widgets re-layout only when necessary
* Guarantee parents respond to child size changes
* Guarantee full reflow after window resize
* Prevent layout inconsistencies, stale heights, and overlapping content
* Maintain predictable performance for trading-grade UIs

This SPEC supersedes all previous implicit behavior and provides a single authoritative specification.

---

## 2. Definitions

### Dirty Flag

A boolean flag stored per widget:

* `dirty_layout: bool`
* Indicates whether the widget **must run layout() again**.

### Constraint Hash

A compact representation of constraints affecting the widget:

* Derived from `(min_w, max_w, min_h, max_h)`
* Used to detect when constraints change across frames

### Size Signature

A tuple:

```
(previous_width, previous_height)
```

Used to detect real size change after layout.

---

## 3. When is a Widget Dirty?

A widget becomes dirty when **any** of the following occur:

### 3.1. Constraints Change

If the incoming constraints differ from last frame:

```
if self.last_constraints != constraints:
    dirty_layout = true
```

Constraint changes include:

* Window resize
* Parent size change
* Any change in `max_width` or `max_height`, even if subtle

### 3.2. Internal State Change

If the widget mutated any property that affects its size:

* Text changed
* Font size changed
* Padding changed
* Style changed
* Wrap mode changed
* Any internal flag that affects layout

### 3.3. Child Size Change

After layout:

* If any child’s `new_size != old_size`
* Or any child reports dirty_layout = true

Then the parent becomes dirty.

### 3.4. Forced Invalidate

Widgets may explicitly invalidate themselves:

```
cx.invalidate_layout();
```

Used for:

* Dynamic content changes
* Runtime style changes
* Error state toggles
* Animations that affect size

---

## 4. Dirty Propagation Rules

HAL-128's geometry-dependent Absolute profile (delegated technical decision,
2026-10-08) observes the actual invalidation origin before early return/upward
propagation. Its bounded causal evidence is distinct from propagated dirtiness
and later geometry production. It permits pre-Layout terminal retirement only;
these propagation/layout rules and rect ownership are unchanged.

Dirty flags propagate ** upward ** through ancestors:

```
child → parent → parent → … → root
```

Propagation stops when:

* A parent is already dirty
* Or the root is reached

This ensures:

* Parent containers re-measure when children grow/shrink
* Columns/Rows reflow on resize
* Complex layouts remain consistent

---

## 5. Layout Resolution Phase

During layout traversal:

### 5.1. Recompute Only When Dirty

```
if !dirty_layout:
    return cached_size
```

### 5.2. Perform Layout

The widget computes:

* Updated child layouts
* Updated size
* Updated absolute offset (via widget_tree)

### 5.3. Dirty if Size Changed

After computing size:

```
if size != previous_size:
    mark parent dirty
```

This is the rule that **fixes TextBlock**, ensuring wrapped lines push down siblings.

### 5.4. Cache Results

Store:

* `cached_size`
* `last_constraints`
* `dirty_layout = false`

---

## 6. Window Resize Invalidation

Window resize **always** marks the root dirty unconditionally:

```
root.dirty_layout = true
```

This ensures:

* All widgets get new constraints
* All layout passes recompute
* No stale positions or heights

This rule is essential for correct resizing behavior.

---

## 7. The Five Invalidation Rules (Summary)

This is the canonical minimal model:

### Rule 1 — Constraint Change Invalidation

`constraints != last_constraints → widget.dirty = true`

### Rule 2 — Internal State Change

Any internal change affecting layout must set:
`dirty_layout = true`

### Rule 3 — Child Size Change

If child size changes after layout:
`parent.dirty = true`

### Rule 4 — Upward Propagation

Any dirty widget marks all ancestors dirty (once per frame).

### Rule 5 — Window Resize

`root.dirty_layout = true` on ANY window resize event.

These five rules guarantee deterministic, recursive, correct reflow.

The scoped HAL-354 section 2.2 incremental-layout qualification (Author JC;
delegated technical R1 decision, 2026-10-10) owns the corresponding retained-paint
dependencies. An actually executed layout invalidates its local paint; a cached
child visit does not. Final changed geometry/viewport publication invalidates
its affected consumers without changing these upward propagation or skip rules.
HAL-088 continues to own the viewport equations. Implementation proof remains
pending with the admitted typography unit.

---

## 8. Pseudocode for Full Layout Pass

```
fn layout_node(node):
    if !node.dirty_layout and constraints == node.last_constraints:
        return node.cached_size

    node.last_constraints = constraints
    node.dirty_layout = false

    let size_before = node.cached_size

    // Layout children bottom-up
    for child in node.children:
        let child_size = layout_node(child)
        if child_size != child.cached_size_prev:
            node.dirty_layout = true

    // If we are dirty, recompute
    if node.dirty_layout:
        node.cached_size = node.compute_layout(constraints)

    // Detect size change after computing
    if node.cached_size != size_before:
        node.parent.dirty_layout = true

    return node.cached_size
```

---

## 9. Impact on Performance

### Positive

* Prevents redundant layouts
* Prevents layout storms
* Ensures minimal reflow
* Locks Halcyon to deterministic behavior needed for trading dashboards

### Neutral

* Small overhead for constraint hashing
* Small overhead for comparing sizes

### Negative

None, if implemented correctly.

---

## 10. Integration Checklist

### Widget Author Must:

* Mark dirty when changing any state affecting layout
* Use constraints to compute size
* Never mutate layout inside paint

### Layout System Must:

* Store constraint hash
* Store previous size
* Propagate dirty flags up
* Re-layout from root when window resized

### Event System Must:

* Never trigger layout directly
* Only state mutations cause layout changes

HAL-101 section 2.5 child operations report child-owned text/layout/paint invalidation and host continuations report their own actual projection changes. No operation directly runs Layout/Paint. Changed geometry uses ordinary layout invalidation; a pure motion sample does not. Existing handled-event paint invalidation is not evidence of no-op clean output.

---

## 11. Tests Required

### 11.1. Width shrink expands lines

* TextBlock wraps
* Height increases
* Column pushes down siblings
* No overlap

### 11.2. Width grow collapses lines

* Height decreases
* Siblings move up
* No stale space

### 11.3. Parent responds to child size change

* Changing label text inside button affects button height
* Changing padding affects layout pass

### 11.4. Window resize

* Everything recalculates
* No stale cached widths

---

## 12. Conclusion

This SPEC defines the authoritative, deterministic dirty flag and invalidation model Halcyon must use.

It directly fixes:

* TextBlock reflow
* Column/Row child handling
* Layout inconsistencies
* Multi-line wrapping bugs
* Resize bugs

This becomes a foundational guarantee for all future work on Halcyon’s layout engine.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
