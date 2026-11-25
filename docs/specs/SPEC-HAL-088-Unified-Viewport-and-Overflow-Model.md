# **SPEC-HAL-088 — Unified Viewport, Overflow, and Scroll Boundary Model**

**Status:** Implemented
**Depends on:**

* SPEC-HAL-008 (Layout Engine)
* SPEC-HAL-024 (Coordinate Model)
* SPEC-HAL-009 (Event Routing / Hit-Testing)
* SPEC-HAL-086 (Scroll Container & Visual Interpolation)

**Purpose:**
Halcyon currently has *correct* local behaviors in isolation:

* Layout computes logical rects
* Coordinate model produces absolute paint rects
* ScrollContainer manages offsets, physics, and overscroll interpolation
* Event routing maps pointer positions into widget-local coordinates
* Widgets paint using absolute rects

What we *do not yet have* is:

1. A **unified definition of “viewport”** across layout, event routing, and painting.
2. A consistent model for **overflow**, **clip boundaries**, and **scrollable regions**.
3. A deterministic way to decide:

    * what is visible
    * what receives pointer events
    * what defines the local coordinate origin inside scrolled content
    * how nested scrollables interact
4. A clear contract for **layout → scene → event mapping**, so no subsystem drifts.

SPEC-088 seals this gap.

---

# **1. Canonical Definitions**

### **1.1 Widget Rects**

Halcyon maintains three distinct rects:

1. **layout_rect** — Output of layout: `(x, y, w, h)` in parent-local space.
2. **absolute_rect** — Positioned in global logical coordinates.
3. **absolute_paint_rect** — After scroll offsets applied; used for painting.

SPEC-088 formalizes their relationships.

---

# **2. Viewport**

For any widget **W**, define:

```
viewport(W) = intersection_of_all_ancestor_clip_rects(W)
```

Including:

* Clip from any scroll container
* Clip from panels with overflow hidden
* Clip from root window bounds

**Viewport determines:**

* Visible region of W
* Valid pointer hit-test region
* Region in which W paints

If a widget paints outside its viewport → clipped.

---

# **3. Overflow Model**

Every widget declares:

```
overflow: Visible | Clip
```

Defaults:

* Row, Column, Flex → Visible
* ScrollArea → Clip
* Panel/Container → Clip
* Root → Clip to window

Rules:

* `Visible` means children may paint outside layout_rect.
* `Clip` means children are clipped to layout_rect (after scroll offset).

---

# **4. Scroll Containers**

Scroll containers are the first widgets that introduce **coordinate transform** beyond simple translation:

```
child_absolute_paint_rect = parent_absolute_paint_rect + (child_layout_offset - scroll_offset)
```

SPEC-086 handles dynamics; SPEC-088 defines boundaries.

### **4.1 Scroll Boundary = scrollport**

For a scroll container SC:

```
scrollport(SC) = SC.absolute_rect
```

**Children cannot paint outside `scrollport(SC)`** regardless of overflow rules downstream.

### **4.2 Nested Scroll**

Nested scroll containers create nested scrollports.

Event routing must detect when pointer motion delta should:

* go to inner scroll area
* bubble to outer scroll area
  (based on boundary tests and overscroll conditions)

SPEC-088 defines the geometry; 086 defines dynamics.

---

# **5. Unified Event Coordinate Mapping**

Pointer events must map through all transforms:

1. Parent absolute rect
2. Child offset
3. Scroll offset
4. Clipping boundaries

Define:

```
local_pos(W) = global_pointer - absolute_paint_origin(W)
```

**absolute_paint_origin(W)** = origin of W after all scroll offsets.

---

# **6. Hit-Testing Rules**

Hit-testing uses:

1. **viewport(W)**
2. **absolute_paint_rect(W)**

A widget is hittable when:

```
pointer ∈ viewport(W)
AND
pointer ∈ absolute_paint_rect(W)
```

The **viewport** test ensures clipping correctness.
The **paint rect** ensures layout correctness.

**2026-10-10 scoped A1/R1 clarification (Author: JC; delegated technical decision):** Local target containment and permission to descend are distinct.
An `OverflowMode::Visible` ancestor's allocation does not clip its descendants,
even when its own target region misses the pointer or is fully clipped. Canonical
DFS and the existing grid collector/candidate validation use the same effective
inherited child clip. Root/window bounds, `OverflowMode::Clip`, scrollports and
explicit widget clips remain binding in their existing scope order; targets
still require their own canonical region and inherited clipping. Immediate
virtualized scroll-content containment, accumulated scroll, local coordinates
and front-to-back control order remain unchanged. This corrects an existing
geometry consumer and does not adopt the global Draft status of HAL-032.

---

# **7. SceneGraph & Clip Nodes**

Scene builder will emit:

* **PushClip(viewport)** at entry of clipped subtree
* **PopClip** at exit

Sequence must follow DFS of widget tree.

Scroll containers always emit a clip node.

Panels / Containers with overflow=Clip also emit a clip node.

---

# **8. Layout → Paint Pipeline Integration**

Add stage:

```
compute_viewports(widget_tree, root)
```

Outputs:

* viewport_rect(W) for every widget
* stored in node metadata for

    * hit-testing
    * scene builder
    * scroll physics boundaries

---

# **9. API Additions**

### **9.1 Widget Metadata**

Add to `WidgetNode`:

```
overflow: OverflowMode,
viewport: RectF,        // computed after scroll + clip stacking
```

### **9.2 Scroll Container**

Expose helper:

```
fn scrollport(&self) -> RectF
```

Used by event routing & scrolling physics.

---

# **10. Implementation Stages**

### **Stage 1 — Overflow & viewport fields**

* Add overflow enum to widget metadata.
* Add viewport to node metadata.

### **Stage 2 — Compute viewports**

Algorithm:

```
fn compute_viewports(node, inherited_viewport):
    my_viewport = intersection(inherited_viewport, node.absolute_rect)
    if node.begin_scrollport: 
        my_viewport = intersection(my_viewport, node.scrollport)
    store my_viewport
    for each child:
        compute_viewports(child, my_viewport)
```

### **Stage 3 — Hit-testing (rect-based; scroll-aware)**

Replace raw absolute_rect checking with scroll-aware containment.

### **Stage 4 — Scroll-aware viewport model (renumbered from Stage 5)**

Use scrollports + viewport to determine:

* pointer wheel routing
* drag boundaries
* overscroll interop

### **Stage 5 — SceneGraph clip nodes (renumbered from Stage 4)**

Insert PushClip/PopClip around:

* scroll containers
* overflow=Clip widgets

---

# **11. Why This SPEC Matters**

It prevents:

* scroll bleed
* invisible widget hit-testing
* caret painting outside visible region
* inconsistent scroll-to-focus
* broken nested scroll
* inconsistent popups

This SPEC ties layout, scroll, paint, and events into one geometric model.

---

# **12. Non-Goals**

* No text editing
* No caret movement
* No animations
* No focus changes
* No styling/theme concerns

Pure structural geometry.

---

# **13. Result**

After SPEC-088 implementation:

* Scroll containers behave 100% correctly.
* Nested scrollers interact deterministically.
* Widgets cannot "escape" clip boundaries.
* Hit-testing becomes robust.
* Scene graph becomes predictable.
* Any future TextField or caret logic becomes *much* easier.

---

# **14. Implementation Status**

| Stage | Description | Status |
|-------|-------------|--------|
| Stage 1 | OverflowMode enum + viewport storage on WidgetNode | ✅ IMPLEMENTED |
| Stage 2 | Viewport propagation & ancestor clipping | ✅ IMPLEMENTED |
| Stage 3 | Hit-testing (rect-based; scroll-aware) | ✅ IMPLEMENTED |
| Stage 4 | Scroll-aware viewport model (formerly Stage 5) | ✅ IMPLEMENTED |
| Stage 5 | SceneGraph clip nodes (formerly Stage 4) | ✅ IMPLEMENTED |

**Stage Renumbering Rationale:**
During implementation of Stage 3 it became clear that scroll-aware viewport correction must precede GPU clip-node
integration. SceneGraph clipping depends on accurate scroll-adjusted viewports; therefore the original Stage 5 has been
promoted to Stage 4, and the original Stage 4 is now Stage 5. No functional changes; the ordering is corrected to match
architectural dependencies.

## Stage 1 Implementation Details

**Files Modified:**
- `src/core/widget_tree.rs`: Added `OverflowMode` enum, `overflow` and `viewport` fields to `WidgetNode`
- `src/core/app.rs`: Updated `compute_absolute_paint_rects()` to set `viewport = absolute_rect`

**Invariants Established:**
- `OverflowMode` always initialized to `Visible` (default for all widgets in Stage 1)
- `viewport` always equals `absolute_rect` after layout (Stage 2 will compute intersections)
- No behavior changes: hit-testing, painting, and scroll logic remain identical

## Stage 2 Implementation Details

**Files Modified:**
- `src/core/widget_tree.rs`: Added `compute_viewports()` and `compute_viewports_dfs()` methods
- `src/core/app.rs`: Removed viewport from `compute_absolute_paint_rects()`, added call to `compute_viewports()` in `layout_tree()`

**Algorithm:**
- Viewport = ancestor-clip region computed via top-down DFS
- Root always clips to its own `absolute_paint_rect`
- `OverflowMode::Clip` nodes intersect incoming clip with their rect (introduces new clip boundary)
- `OverflowMode::Visible` nodes pass incoming clip unchanged (children see only ancestor clips)

**Invariants Established:**
- All nodes with `OverflowMode::Visible` (current default) have viewport = root's rect
- viewport is computed but NOT used for hit-testing or painting yet
- No behavior changes: hit-testing, painting, and scroll logic remain identical

## Stage 3 Implementation Details

**Files Modified:**
- `src/core/widget_tree.rs`: Updated `hit_test_node_with_path()`, `hit_test_node()`, and `collect_hit_test_nodes_recursive_static()` to use viewport for containment tests
- Added `set_overflow()` method for setting node overflow mode
- Added 3 tests: `test_SPEC088_clip_parent_blocks_child_hit`, `test_SPEC088_nested_clip_scopes`, `test_SPEC088_visible_parent_preserves_old_behavior`

**Behavior Changes:**
- Hit-testing now uses `node.viewport` instead of `node.absolute_paint_rect` for containment checks
- Nodes outside their clipped visible region (viewport) are no longer hittable
- Overlay hit regions (scrollbars) remain unchanged - they use their own explicit geometry

**Preserved Semantics:**
- Traversal order and z-order logic unchanged
- local_pos and absolute_bounds in HitTestResult still use absolute_paint_rect
- Non-clipping (Visible) parents behave exactly as before

**Frame Ordering Fix:**
- `src/core/app.rs`: Reordered frame loop to update scroll animations and recompute positions BEFORE event routing
- Previous order: event routing → scroll update → layout
- Fixed order: scroll update → recompute positions → event routing
- Examples also updated with same frame ordering

**Viewport-Based Clipping Limitation (Discovered):**
- Viewport-based hit-testing doesn't work correctly for scrollable containers
- Problem: viewport = intersection(absolute_paint_rect, window_clip)
- Content at layout y=1200 in a 600px window has empty viewport
- But this content IS visible when scrolled into view
- The scroll_offset transformation maps screen coords to content coords correctly
- But viewport clipping rejects content outside window bounds in layout space
- **Current workaround**: Hit-testing uses `absolute_paint_rect` instead of `viewport` for containment
- Scroll_offset transformation handles the screen-to-content coordinate mapping
- Future work: Properly integrate viewport clipping with scroll containers (Stage 4)

## Stage 4 Implementation Details

**Files Modified:**
- `src/core/widget_tree.rs`: Modified `compute_viewports_dfs()` to detect scroll containers via `layout_scroll_offset.is_some()`

**Geometry Change:**
- Scroll container detected → use `scrollport` (its `absolute_paint_rect`) as clip boundary
- Children inside scrollport inherit scrollport clip; do NOT intersect with own `absolute_paint_rect`
- `inside_scrollport` flag tracks traversal state to skip rect intersection for descendants

**Tests Added:**
- `test_SPEC088_scrollport_basic`: Button at y=1200 inside 600px scrollport gets viewport height=600
- `test_SPEC088_scrollport_nested`: Nested scroll containers create nested scrollports
- `test_SPEC088_scrollport_nonscroll_unchanged`: Non-scroll containers behave as Stage 2

## Stage 5 Implementation Details

**Files Modified:**
- `src/core/app.rs`: Modified `paint_node()` to emit `begin_clip(viewport)` for nodes with `overflow == Clip` or `layout_scroll_offset.is_some()`

**SceneGraph Integration:**
- Clip nodes emitted via existing `SceneBuilder::begin_clip(viewport, None)` / `end_clip()` API
- Clip rect = `node.viewport` (computed in Stage 4)
- WGPU renderer already handles `ClipNode` with scissor conversion via `clip_rect_to_scissor()`

**Tests Added:**
- `test_SPEC088_clip_node_for_scroll_container`: Scroll container emits ClipNode with viewport
- `test_SPEC088_clip_node_for_overflow_clip`: Overflow=Clip emits ClipNode with viewport
- `test_SPEC088_clip_node_nesting`: Nested scroll containers create nested ClipNodes
- `test_SPEC088_clip_node_no_duplicate_push`: Single clip per widget (OR condition, not AND)

---

# **15. Invariant: ScrollArea Requires Tight Constraints Final Pass**

## Problem

`ScrollArea` only finalizes scroll metrics (max_scroll, content extent, etc.) when it receives a **final/tight** layout pass (`constraints.min == constraints.max`). When `ScrollArea` is mounted as a screen root (or otherwise receives only loose constraints), it will **never** hit final-pass → `max_scroll` stays `0` even when `content_h > viewport_h`. This silently breaks wheel scrolling.

## Invariant Statement (INV-SCROLL-TIGHT-CONSTRAINTS)

**ScrollArea MUST receive at least one tight-constraint layout pass per frame when content exceeds viewport.**

Finalization of scroll metrics occurs only on final/tight pass. Embedders/screens must structure layout so ScrollArea gets at least one tight pass per frame. Mounting ScrollArea as screen-root without a tight parent is **invalid** and will not scroll.

## Correct Structure

```rust
// Root container - Flex that fills window (gives tight constraints to children)
let root = cx.mount_child(Box::new(Flex::column()));
tree.set_flex(root, FlexParams::flexible(1.0));

// ScrollArea inside Flex - will get tight constraints triggering final-pass
let scroll_area = tree.add_child(root, Box::new(ScrollArea::new(NodeId(0))), None, 0);
tree.set_flex(scroll_area, FlexParams::flexible(1.0));
```

## Invalid Structure

```rust
// WRONG: ScrollArea as screen root - only receives loose constraints
let scroll_area = cx.mount_child(Box::new(ScrollArea::new(flow_grid)));
// max_scroll will stay 0, scrolling will not work
```

## Debug-Build Detection

In debug builds, ScrollArea checks for this invariant violation and emits a warning when:
- Content exceeds viewport (content_h > viewport_h or content_w > viewport_w)
- No tight pass was received for the frame

**Enable with:** `HAL_WARN_SCROLL_TIGHT_CONSTRAINTS=1`

Warning format:
```
[INV-SCROLL-TIGHT-CONSTRAINTS] ScrollArea content exceeds viewport but no tight pass received.
Scrolling will NOT work. Fix: Wrap ScrollArea in Flex/other parent that gives tight constraints.
viewport_h=600.0 content_h=978.0 c_min_h=0.0 c_max_h=inf final=0
```

The warning is rate-limited to once per ScrollArea instance until a final pass successfully occurs.

## Why This Design

Stage 2.8 of SPEC-HAL-086 established that scroll state updates are gated to final passes to avoid premature clamping during Flex measurement passes. This is correct behavior, but creates a silent failure mode when the embedding structure never produces tight constraints. This section locks the invariant so future developers understand the requirement.

# **16. Invariant: ScrollArea Measurement Is Finite**

## Invariant Statement (INV-SCROLL-FINITE-MEASURE)

Every ScrollArea construction path, including `ScrollArea::empty()` and
`ScrollArea::new_unwired()` followed by `wire_content()`, may receive an
unbounded maximum on either axis during a parent container's measurement pass.
An unbounded maximum is measurement capability, not allocated viewport
geometry. On each unbounded axis, ScrollArea uses a configured finite viewport
when one exists; otherwise it reports its finite incoming minimum until a
bounded allocation pass supplies the final extent. A viewport-constrained child
receives the resulting finite measurement viewport, including a degenerate
zero-sized axis when no intrinsic minimum exists.

The measurement result, retained `container_size`, child rects, and
`clip_bounds()` must remain finite after every pass. A later tight allocation
replaces the measurement extent normally. Scene validation remains strict;
rendering must never clamp or accept a non-finite clip produced by layout.

# **17. Canonical Child-Clip Source under ScrollNode (2026-11-14)**

**Status: Adopted / Binding corrective implementation decision under the recorded configurable-material mandate.** The required behavior remains sections 3, 4 and 6: a declared Clip container restricts its children, Visible introduces no child boundary, and every outer scrollport remains effective. HAL002/002A, HAL024, HAL086 Stage 7 and HAL354 remain unchanged.

## 17.1 Observed failure and representation boundary

The actual S2A public Lab::build test failed before native execution: cell0 required RectF(366,76,272,204), but its descendants inherited RectF(354,12,914,796) three times. Current compute_viewports_dfs passes its incoming scrollport through every inside_scrollport descendant without consulting OverflowMode::Clip. Current App composition then uses that layout aggregate as the declaring container's own ClipNode.

HAL086 Stage 7 and compute_absolute_paint_rects_recursive establish that stored absolute_paint_rect is unscrolled authoring geometry. Batcher::push_clip projects each source rect by the current enclosing ScrollNode offsets before intersecting it with already projected ancestor clips. A window-clipped aggregate cannot be placed under a different scroll scope as if it were a new raw boundary. Merely intersecting an offscreen content rect with the fixed outer scrollport during Layout can also discard content that later scrolls into view.

The effective viewport defined in section 2 is the intersection AFTER each boundary's own projection. It is not any individual source ClipNode. Section 14 Stage 5's historical instruction to use node.viewport as every raw clip source is superseded by 17.2. This does not redefine the legacy get_viewport allocation snapshot as a new screen-space API or authorize a broad metadata/custom-render migration.

## 17.2 Selected existing-owner correction

Add one crate-private, read-only WidgetTree::child_clip_source(NodeId) -> Option<RectF>. It derives its result from the current node's existing authoritative geometry and declaration: return its unmodified absolute_paint_rect when overflow is Clip OR layout_scroll_offset is Some; otherwise return None. A missing node returns None. No stored duplicate geometry, registry, cache, public API, ancestor scan, scroll subtraction, inverse compensation or new coordinate space is introduced.

Use that ONE owner-derived source at all existing consumers which author or describe the corresponding child boundary:
- App retained/fresh composition's ordinary viewport clip and transported main-affordance viewport clip.
- WidgetTree::own_presentation_geometry's ancestor viewport-clip intersection.
- WidgetTree::absolute_drag_clip_basis's corresponding declaration.

These consumers retain their existing scope order. The source encloses children, not the declaring owner's local paint or post-child overlay. It is opened before that owner's own ScrollNode; an ancestor's scroll affects it, but its own scroll affects only its descendants. The renderer still applies the enclosing scroll once and intersects the full stack. Presentation geometry uses its existing cumulative-scroll projection once for each ancestor source. Drag comparison retains exact declaration/mount identity and existing cancellation ownership.

Do not change compute_viewports_dfs or get_viewport into a globally different representation in this correction. Their existing allocation/compatibility metadata and custom-render metric callers remain intact; they no longer supply raw child-clip authority to the three consumers above. Do not use a shader crop, clip_bounds override, widget-emitted ClipNode, new viewport mirror, or example coordinates inside the engine. Root/target bounds, explicit widget rounded clips, clip nesting and effective ordering remain binding. Preserve existing dirty-layout/composition publication and clean local-fragment reuse.

**INV-088-CHILD-CLIP-SOURCE:** Every declared child clipping boundary derives from that declaring node's current canonical allocation in its actual scope; no inherited aggregate impersonates the source.
**INV-088-CLIP-PROJECTION-ONCE:** Final coverage intersects correctly projected ancestor sources, without applying any scroll twice or pre-clipping movable content against a different scroll basis.
**INV-088-VISIBLE-DESCENT:** A Visible node contributes no allocation clip to its descendants.

## 17.3 Proof and bounded source scope

Extend the existing WidgetTree/App owners and their existing inline/internal tests, not production rendering or input policy. Prove the original six actual lab host boundaries at zero and nonzero scroll. Keep raw allocation assertions separate from projected scene assertions: scroll_to(0,48) jumps the visual offset; verify that fact, then derive expected projected cell rectangles from the raw allocation minus the actual accumulated ancestor offset. Do not compare already projected ClipNodes to unscrolled expected rectangles or weaken the zero-scroll RED.

At the retained internal owner prove scroll-only recomposition with unchanged local child Arcs, partial outer clipping, nested scroll scopes, a Clip host initially beyond the window scrolling into view, and Visible overflow preservation. Verify effective clipping/hit parity through the existing canonical/accelerated paths and transported main-affordance scope. Reuse existing tests where they cover the actual changed boundary; no public test-only engine entry. A fresh public scene helper does not prove persistent-cache reuse.

The current allowed engine files are src/core/widget_tree.rs and the exact child-clip consumption spans of src/core/app.rs. Existing src/tests/retained_value_updates.rs and src/tests/renderer_z_order.rs own necessary retained and actual GPU proof. Continue the preserved glass_material_lab example/public integration candidate and its scoped CHANGELOG entry. Do not change optical equations, resources, sealed-command custody, ScrollArea dynamics, focus/capture policy, custom-render admission or other projects. Finish by returning to the original native material-lab acceptance scenario in HAL386 sections 12-13. Implementation and successful execution are not claimed by this adoption.

## 17.4 Independent source and owner-boundary acceptance (2026-11-14)

**Disposition: ACCEPTED within the canonical child-clip source and focused CPU/Build boundary.** Primary inspected the live `child_clip_source` derivation, its presentation/drag consumers and both App composition paths against section17.2. The declaring allocation remains unmodified; inherited projection occurs once, the source opens before the declaring owner's own ScrollNode, and local/post-child overlay ownership is unchanged. Legacy viewport/custom-render metadata were not redefined.

The seven current owned source images match the S2B source manifest. Original receipts under the existing Actual visualization directory `hal-optics-s2b-20261114-wdl6go0s` establish check exit0 with127 warnings; final `glass_material_lab` has five library executions plus four public executions (four cases are the same included example tests, not nine distinct behaviors); `child_clip_source`3/3 and `strict_z_`3/3 passed. Primary inspected the actual six-cell zero/nonzero-scroll oracle, nested/offscreen retained-Arc and hit-parity proof, and transported-affordance regression reuse. No verification was repeated for this review. The original S2A RED remains historical evidence.

This acceptance does not close the material lab's isolated native control scenario or JC optical acceptance. HAL386 section14 owns that remaining continuation. No further canonical viewport or clipping change follows merely from an unqualified native input run.
