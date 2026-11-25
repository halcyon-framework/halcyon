# **SPEC-HAL-035: Widget Diff Optimizer**

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Adopted by:** SPEC-HAL-354 for binding dirty-domain separation, retained widget scene fragments, and public-path completion proof. This draft remains non-binding where it proposes incompatible or unimplemented detail.
**Depends on:**

* SPEC-HAL-001 – Core Architecture
* SPEC-HAL-010 – Widget System
* SPEC-HAL-015 – Caching & Diffing
* SPEC-HAL-026 – Dirty-Flag Layout Invalidation
* SPEC-HAL-027 – SceneGraph Batching

---

## 1. Purpose

Define a **high-efficiency diffing pass** that determines:

* Which widgets must re-layout
* Which widgets must re-paint
* Which widgets must re-emit scene nodes
* Which children must propagate changes
* Which updates can be **skipped entirely**

The purpose is to eliminate unnecessary work in Halcyon’s retained-mode UI, especially under high-frequency update conditions (charts, orderbooks, DOM simulators).

The diff optimizer is **the final layer** that ties together all dirty-flag strategies and batchable pipelines, ensuring the UI runs at stable frame times even at 120–240 FPS with live trading data.

---

## 2. Problem

Current behavior:

* Changes at one node may cause too many children or ancestors to recompute.
* Dirty flags exist (SPEC-HAL-026) but lack a **precision diff pass**.
* Widgets opt into diffing on a per-widget basis, but the engine doesn’t unify them.
* Re-layout often cascades further than required.
* Re-painting always walks entire subtree even if nothing changed.
* Scene nodes are rebuilt more often than necessary.

This SPEC defines a **strict, deterministic, minimal diff pass**, executed once per frame, before layout and paint.

---

## 3. Design Overview

Introduce a three-tier diff model:

1. **State Diff**
   Detect whether a widget’s logical input changed.

2. **Layout Diff**
   Decide if the widget’s *size* or *constraints* might change.

3. **Paint Diff**
   Decide if the widget’s *visual output* might change.

Only widgets with a positive diff in these categories are allowed to proceed to layout/paint. Everything else uses cached rects and cached scene nodes.

---

## 4. Dirty State Classification

We define three dirty bits per widget:

```rust
pub struct WidgetDirty {
    pub state: bool,   // logical data changed
    pub layout: bool,  // size/constraints may change
    pub paint: bool,   // visual output may change
}
```

**Rules:**

* `state = true` always implies at least `layout = true` or `paint = true` depending on widget type.
* `layout = true` implies `paint = true` (layout changes affect paint).
* `paint = true` does NOT imply `layout = true`.

---

## 5. Diff Pass Workflow

The diff pass runs in this exact order:

1. **State comparison phase**

    * For each widget: compare former vs current public fields.
    * Widget authors expose a `.diff_state(&old, &new) -> WidgetDirty`.

2. **Constraint propagation phase**

    * If parent layout is dirty → child layout becomes dirty.
    * If constraints changed → child layout dirty.

3. **Structural consistency checks**

    * If widget changed its number of children → full subtree layout dirtied.

4. **Paint dirtiness propagation**

    * If parent is paint-dirty → child is *not* automatically paint-dirty (important).
      Paint dirtiness is local unless overwritten.

5. **Scene node pruning**

    * If `paint == false` AND `layout == false`, reuse previous scene nodes entirely.

This produces a minimal set of nodes that actually need to run layout/paint.

---

## 6. Integration with SceneGraph Batching (SPEC-HAL-027)

The diff optimizer directly enables batching:

* If two sibling widgets are paint-clean → they stay in the same batch without recomputation.
* If one sibling becomes dirty → only that widget’s batch fragment is rebuilt.
* Top-level batch segments can skip full rebuild when all children remain clean.

This prevents “batch invalidation storms.”

---

## 7. Integration with Clip Stack Rework (SPEC-HAL-028)

Clipping interacts tightly with diffing:

* If the clip stack fingerprint changes → paint dirty.
* If ancestor introduces or removes a clip → descendant layout may also become dirty.

The diff pass must compute clip fingerprints early and treat them as part of paint-diff state.

---

## 8. Integration with Glyph Atlas (SPEC-HAL-029)

* Text widgets compute `TextDirty` by comparing:

    * font_size
    * text
    * color
    * wrapping width
* If none changed → no text shaping, no glyph insertion, no scene rebuild.

This optimization is crucial for charts with hundreds of static labels.

---

## 9. Rules for Each Widget

### 9.1 Parent → Child Propagation

Parent dirty flags propagate like this:

| Parent Dirty | Child Layout | Child Paint |
| ------------ | ------------ | ----------- |
| state        | maybe        | maybe       |
| layout       | **yes**      | yes         |
| paint        | no           | no          |

Key: **paint does not propagate** unless layout is also dirty.

### 9.2 Widget Behavior Contract

Each widget must implement:

```rust
fn diff(&mut self, old: &Self) -> WidgetDirty;
```

Widgets must:

* Compare user-facing fields.
* Not compare internal caches.
* Set only necessary dirty bits.
* Never escalate dirty bits unless logically required.

---

## 10. Invalidation Rules (Strict)

### 10.1 State Changes

Must set:

* `paint = true`
* `layout = depends`

Example:

* Button label text changes → layout may change.
* Button click state changes (hover, pressed) → paint only.

### 10.2 Layout-Relevant Properties

Any change to:

* padding
* margin
* font size
* wrap width
* align
* spacing
* flex ratio
* row/column arrangement

→ `layout = true`.

### 10.3 Paint-Relevant Properties

Any change to:

* color
* background
* border radius
* border width
* opacity
* shadows

→ `paint = true` only.

---

## 11. Output of Diff Pass

The diff pass produces:

* **Set of widgets requiring layout**
* **Set of widgets requiring paint**
* **Set of widgets requiring scene rebuild**
* **Set of unchanged widgets (skip everything)**

This defines the minimum work required to produce next frame.

---

## 12. Implementation Plan for Claude Code

### Step 1 — Add Dirty Flags to Widget Instance

Extend:

`src/core/widget.rs`:

* Add `WidgetDirty`.
* Add lifecycle: `old_widget.clone_before_update()`.

### Step 2 — Diff Protocol

* Implement `Widget::diff(&old_self)` for all Tier-1 widgets.
* Integrate in `widget_tree.rs` before layout.

### Step 3 — Global Diff Pass

Create:

`src/core/diff.rs`:

```rust
pub fn compute_dirty_flags(tree: &mut WidgetTree) { ... }
```

Responsibilities:

* Call `diff()` per widget.
* Propagate dirty up/down.
* Mark children appropriately.
* Mark paint/layout rebuild targets.

### Step 4 — Layout Integration

Modify `layout.rs`:

* Layout only dirty widgets.
* Reuse cached size + rect for clean widgets.
* Stop at clean subtrees.

### Step 5 — Paint Integration

Modify `scene.rs`:

* Rebuild scene nodes only for paint-dirty widgets.
* Reuse cached scene fragments.

### Step 6 — Testing

Add tests:

* `tests/diffing_tests.rs`
* Scenarios:

    * Text change → layout+paint only at that node.
    * Color change → paint only.
    * Spacing change → layout cascade.
    * No change → no layout, no paint, no scene build.

---

## 13. Invariants

* Visual output must be identical before/after enabling diff optimizer.
* Clean subtrees must not incur any computation cost.
* No excessive invalidation.
* No stale scene nodes.

---

## 14. Result

This SPEC completes the optimization suite:

* 100 — Dirty Layout
* 101 — Scene Batching
* 102 — Clip Stack
* 103 — Atlas Cache
* 104 — Frame Pacing
* 105 — Input Fusion
* 106 — Hit-Test Acceleration
* 107 — GPU Text Merging
* **109 — Widget Diff Optimizer (this one)**

Together these form the full Halcyon performance core.
