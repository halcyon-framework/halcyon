---
doc-id: halcyon-guide-layout
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-008
  - SPEC-HAL-024
  - SPEC-HAL-026
---

# Layout, Scrolling, and Geometry

Revision `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`; two-axis tags per
SPEC-HAL-309 §7. Invalidation timing lives in
`runtime-and-frame-lifecycle.md` §5-§6; this document covers the geometry
contracts themselves.

## 1. Coordinate Authority

SPEC-HAL-024 is the single coordinate authority — `Status: Stable`,
`Implementation: Complete`, and it "supersedes all coordinate-related
content in SPEC-HAL-001…011"
[authority: binding — SPEC-HAL-024:1-10]. Read it before writing any
geometry code; older SPECs' coordinate prose is superseded by declaration.
Layout and paint purity is constitutional
[authority: binding — SPEC-HAL-002 §4.7], and ephemeral layout structures
must not be retained across phases [authority: binding — SPEC-HAL-002 §3.9].

## 2. The Layout Protocol

`Widget::layout(cx, constraints, children) -> LayoutResult`
[implementation: implemented — `src/core/widget/mod.rs:2067-2072`].

- `Constraints { min_width, max_width, min_height, max_height }`
  [implementation: implemented — `src/core/layout.rs:106-111`].
- `LayoutResult` (SPEC-HAL-086 Stage 2/5)
  [implementation: implemented — `src/core/layout.rs:24-33`]:
  - `size` — final rendered size, respects constraints;
  - `content_size` — natural content size, may overflow `size`;
  - `viewport_size: Option<Size>` — scroll containers only;
  - `scroll_offset: Option<(f32, f32)>` — scroll containers only.
  Non-scroll widgets: `content_size == size`, scroll fields `None`
  (`LayoutResult::new`, `:38-40`; `LayoutResult::fit` per
  `src/core/widget/mod.rs:2065`).
- Flex participation is tree-side state: `WidgetTree::set_flex/get_flex`
  (`FlexParams`), plus `set_overflow(OverflowMode)`,
  `set_layout_scroll_offset`, `set_absolute_xy`, `set_rect/get_rect`
  [implementation: implemented — `src/core/widget_tree.rs:951-1053`].
- `Widget::layout_axis()` exposes a widget's primary axis where meaningful
  [implementation: implemented — `src/core/widget/mod.rs:2330`].

In-frame ordering around layout is engine law (SPEC-HAL-131A/C): pre-layout
scroll sync → `layout_tree` → input projections → scroll-anchor resolution
→ focus reveal → list scroll sync → `paint_tree`
[implementation: implemented — `src/core/app.rs:1665-1677`].

## 3. Dirty Rules

SPEC-HAL-026 (`Status: Final`) owns re-layout rules; the five-rule model —
constraint change, internal state change, child size change, upward
propagation once per frame, window resize dirties the root
[authority: binding — SPEC-HAL-026:5, :192-218]. SPEC-HAL-354 extends it:
layout dirtiness is not a value-refresh signal; the update-phase triage is
implemented at `src/core/app.rs:1261-1276`
[authority: binding — SPEC-HAL-026:8 | implementation: implemented].
Widget-side entry points: `UpdateContext::invalidate_layout` /
`invalidate_paint` / `invalidate_paint_subtree`
[implementation: implemented — `src/core/widget/mod.rs:1980-1994`].

## 4. Scrolling

### 4.1 ScrollContainer — the shared state machine

Every scrollable widget owns a `ScrollContainer`
[implementation: implemented — `src/core/scroll_container.rs:35-55`]:
`content_size`, `viewport_size`, and a dual offset —
`scroll_offset_target` (logical, precise, clamped) vs
`scroll_offset_visual` (smoothed, for rendering) (SPEC-HAL-086 Stage 3).
The unified input pipeline (Stage 4/12): wheel updates the target via
`scroll_by`; scrollbar drag pushes absolute positions; the frame loop calls
`update_visual()` every frame; both modes interpolate
(`NORMAL_LERP = 0.25`, `DRAG_LERP = 0.5`) so motion is frame-driven and
independent of pointer event frequency
[implementation: implemented — `src/core/scroll_container.rs:10-33`,
`ScrollInputMode`].

### 4.2 Widget-trait scroll seams

`scroll_offset()`, `scroll_container()/scroll_container_mut()`,
`wants_scroll()`, `needs_viewport_constraint()`, `sync_scroll()`,
`take_requested_scroll_y()`, `has_pending_anchor_resolution()`,
`height_stable()` [implementation: implemented —
`src/core/widget/mod.rs:2160-2254`]. The tree mirrors several for
traversal-time queries (`src/core/widget_tree.rs:1000-1016`).

### 4.3 ScrollArea law

"ScrollArea is a fixed-width container. It fills all available cross-axis
space provided by its parent. Children never influence ScrollArea's width.
Scrollbars always align to ScrollArea.rect" — stated as the binding header
of both the layout and widget-system SPECs
[authority: binding-as-declared — SPEC-HAL-008:4-8, SPEC-HAL-010:3-7].

### 4.4 Scroll anchoring

Layout-scroll anchoring is tree-owned: `scroll_anchor_nodes`,
`begin_layout_scroll_anchor(anchor, affected_region, intent)`, resolved
post-layout in the pipeline (§2 step "scroll anchors")
[implementation: implemented — `src/core/widget_tree.rs:231`, `:643-676`,
`:716-721`].

### 4.5 Horizontal scroll continuity (composite widgets)

Composite tabular widgets keep header/footer horizontally aligned with a
body that scrolls inside a ScrollArea via a shared
`HorizontalScrollLink` plus a resize-time freeze flag
(`Rc<Cell<bool>>`) — both live in the composite's core state
[implementation: implemented — `src/widgets/list_view/core.rs:162-165`;
mechanism detail in `tabular-and-hierarchical-views.md` §4].

## 5. Geometry Ownership Rules (what not to cross)

- Column geometry in tabular views belongs exclusively to the
  ColumnGeometryEngine; scroll state must not influence geometry
  resolution; geometry resolves exactly once per layout pass and is
  epoch-versioned — stale reads are illegal and panic in debug builds
  [implementation: implemented —
  `src/engines/column_geometry/mod.rs:8-17`, INVARIANT-1..7].
- Overlay/compositor layers never mutate widget geometry
  (INV-COMP-N01/N02, `src/compositor/mod.rs:27-28`).
- Persistent access to ephemeral layout structures is a constitutional
  poison [authority: binding — SPEC-HAL-002 §3.9].

## 6. Statuses To Keep In Mind

SPEC-HAL-008 and SPEC-HAL-010 are draft-era documents with binding header
laws (§4.3) — treat their body prose as intent and verify against
`src/core/layout.rs` / live widgets before relying on details
[authority: mixed — header law vs draft body; recorded for audit].
SPEC-HAL-024 and SPEC-HAL-026 are the firm ground (§1, §3).
