SPEC-HAL-131B
Realization & Virtualization

Status: Binding / Locked
Locked: 2025-12-20
Note: Any future behavior change requires a new SPEC.
Scope: DatasetView, RealizationWindow, slot reuse, variable height handling
Layer: Virtualization consumer (depends on scroll substrate)
Companion: SPEC-HAL-131A (Scroll Substrate & Input Arbitration)
Depends on: SPEC-HAL-131A, WidgetTree, Layout, Selection model
Explicit goal: Virtualization consumes scroll state, never produces it

────────────────────────────────────────────────────────────────────────────────
SECTION A: Problem Statement
────────────────────────────────────────────────────────────────────────────────

Virtualization has historically:
- Computed content extent (wrong: scroll substrate concern)
- Managed scroll position (wrong: scroll substrate concern)
- Conflated data identity with visual position

This SPEC establishes virtualization as a pure consumer of scroll state.

Virtualization projects data into the visible window. Nothing else.

────────────────────────────────────────────────────────────────────────────────
SECTION B: Formal Objects
────────────────────────────────────────────────────────────────────────────────

B.1 DatasetView

DatasetView MUST provide:

    len() -> usize                       // Total item count in current view
    id_at(index) -> RowId                // RowId at logical index
    index_of(RowId) -> Option<usize>     // Reverse lookup (may be partial or lazy)
    view_epoch: ViewEpoch                // Monotonic version counter

INV-VIEW-REPRESENTATION
    SPEC-HAL-131B does NOT mandate concrete storage structures.
    Implementations MAY use lazy, paged, or external-backed views.

INV-VIEW-1
    ViewEpoch advances only on structural change (filter, sort, insert, remove).

INV-VIEW-2
    Scroll does not advance ViewEpoch.

INV-VIEW-3
    RowId → index mapping is valid only for current ViewEpoch.

INV-VIEW-4
    Logical order is immutable within a ViewEpoch.

────────────────────────────────────────────────────────────────────────────────

B.2 RowId

    struct RowId(u64);  // Opaque, stable identity

INV-ROWID-1
    RowId is stable across filter, sort, scroll, and pagination.

INV-ROWID-2
    RowId is never derived from index.

INV-ROWID-3
    Index is never used as identity. Index is position in current DatasetView.

INV-ROWID-4
    RowId equality is the only valid identity comparison.

────────────────────────────────────────────────────────────────────────────────

B.3 ViewEpoch

    struct ViewEpoch(u64);  // Monotonic counter

| Operation              | ViewEpoch Advances? |
|------------------------|---------------------|
| Scroll                 | No                  |
| Filter change          | Yes                 |
| Sort change            | Yes                 |
| Item insert            | Yes                 |
| Item remove            | Yes                 |
| Item update (data)     | No                  |
| Pagination change      | Yes                 |

────────────────────────────────────────────────────────────────────────────────

B.4 RealizationWindow

    struct RealizationWindow {
        logical_range: Range<usize>,  // Index range in DatasetView
        overscan_policy: OverscanPolicy,
        slot_count: usize,            // Fixed pool size
    }

INV-REAL-1
    slot_count is bounded and independent of dataset size.

INV-REAL-2
    slot_count = visible_count + overscan (overscan is constant).

INV-REAL-3
    logical_range is computed from scroll state, not from realized widgets.

INV-REAL-4
    Realization consumes scroll state. It never produces scroll state.

INV-VIRTUAL-IDENTITY-1
    Logical item identity MUST remain stable across realization and unrealization
    cycles.

    Virtualization MUST NOT create, destroy, or remap logical identities.
    Identity continuity is required for anchors, selection, focus, and state
    persistence.

────────────────────────────────────────────────────────────────────────────────
SECTION C: Non-Virtualized Subsumption
────────────────────────────────────────────────────────────────────────────────

DEFINITION: Non-virtualized equivalence

    A non-virtualized container is defined as:
    RealizationWindow.logical_range == 0..DatasetView.len()

INV-SUBSUME-1
    No separate code paths for "if virtualized".

INV-SUBSUME-2
    Non-virtualized is the degenerate case where window equals full dataset.

INV-SUBSUME-3
    All invariants in this SPEC apply equally to non-virtualized content.

INV-SUBSUME-BOUND
    Non-virtualized equivalence is conceptual.
    Implementations MAY refuse full realization when DatasetView.len exceeds configured bounds.

INV-SUBSUME-MATH
    Scroll math and invariants remain identical regardless of realization strategy.

────────────────────────────────────────────────────────────────────────────────
SECTION D: Realization Invariants
────────────────────────────────────────────────────────────────────────────────

INV-VIRT-1
    Virtualization operates on DatasetView, not on widgets.

INV-VIRT-2
    visible_index_range is computed solely from:
    - scroll_offset (from SPEC-HAL-131A)
    - viewport_size (from SPEC-HAL-131A)
    - item metrics (heights/widths)

INV-VIRT-3
    Visible range computation is deterministic and side-effect free.

INV-VIRT-4
    Visible range computation must not depend on realized widgets.

INV-VIRT-5
    Realization pool size is fixed. Growth requires explicit reconfiguration.

INV-PROJECTION-SNAPSHOT-1
    Virtualization projection is computed exclusively during layout_tree().
    After layout_tree() completes, the projection is treated as a read-only snapshot
    for the remainder of the frame.

    Post-layout phases (anchor resolution, hit-testing, paint) may read projection
    data but MUST NOT mutate, recompute, or extend it.

────────────────────────────────────────────────────────────────────────────────

D.2 Scroll-Driven Visible Range Invariants

INV-VIRT-SCROLL-1
    visible_range must change ONLY as a function of scroll offset, never layout
    side effects.
    Layout passes, widget binding, slot reuse, and realization MUST NOT mutate
    visible_range. Only scroll offset changes (from user input or programmatic
    scroll) may cause visible_range to change.

INV-VIRT-SCROLL-2
    For a fixed scroll offset, visible_range must be deterministic across frames.
    Given identical scroll_offset, viewport_size, and item metrics, visible_range
    computation MUST produce identical results on every invocation.

INV-VIRT-SCROLL-3
    visible_range changes must be monotonic with scroll direction (no oscillation).
    Scrolling down MUST NOT decrease visible_range.start.
    Scrolling up MUST NOT increase visible_range.start.
    No backward jumps or oscillation permitted during continuous scroll in one
    direction.

INV-VIRT-SCROLL-4
    update_visible_range_from_scroll(scroll_y) must be a pure function of its
    inputs and immutable list config.
    It must not read/derive from realized children count, slot pool state, or
    any layout-time transient.
    Reason: kills "layout side effects" by construction.

────────────────────────────────────────────────────────────────────────────────

D.3 Constraint-Driven Viewport (SPEC-HAL-NNN Addition)

INV-LIST-VIEWPORT-1
    List MUST derive viewport_height from LayoutConstraints.max_height during layout().
    External setters (set_viewport_height) are FORBIDDEN.

    Source: constraints.max_height in layout()
    Forbidden: Storing viewport height via setters or events

INV-LIST-VIRTUALIZATION-1
    Visible slot count MUST be computed inside layout() as:

        ceil(viewport_height / item_height) + overscan

    Where viewport_height comes from constraints.max_height.
    This computation happens once per layout pass, not externally.

INV-LIST-STATE-1
    List MUST NOT store viewport geometry as mutable runtime state.

    Forbidden:
    - set_viewport_height() as externally callable mutation
    - resize listeners that mutate viewport state
    - engine sync hooks pushing geometry into the widget

    Viewport geometry is ephemeral: derived from constraints, used in layout, discarded.

INV-LAYOUT-AUTHORITY-1
    Layout constraints are authoritative for viewport geometry.

    If constraints change (e.g., window resize):
    - Layout recomputes with new constraints
    - visible_range recomputes automatically
    - Virtualization updates without explicit invalidation

    No explicit invalidation APIs are allowed for viewport-triggered updates.
    The layout system's constraint propagation IS the invalidation mechanism.

INV-CONSTRAINT-FLOW-1
    ScrollArea MUST pass its viewport_height as constraints.max_height to content.

    This enables constraint-driven virtualization:
    - ScrollArea receives constraints from parent
    - ScrollArea determines viewport_size from constraints
    - ScrollArea passes viewport_height as max_height to List
    - List derives visible_range from constraints.max_height

    The constraint chain is: Window → Layout → ScrollArea → List → visible_range.

────────────────────────────────────────────────────────────────────────────────
SECTION E: Slot Pool Contract
────────────────────────────────────────────────────────────────────────────────

INV-SLOT-1
    Slots are reused freely. Slot identity is meaningless outside the pool.

INV-SLOT-2
    Binding a slot assigns exactly one RowId at a time.

INV-SLOT-3
    Unbinding a slot discards all widget-local state.

INV-SLOT-4
    Binding is the only moment data enters a widget.

INV-SLOT-5
    Widgets render purely from bound data + visual state snapshot.

INV-SLOT-6
    Slot widgets must not persist visual state across reuse.

────────────────────────────────────────────────────────────────────────────────
SECTION F: Prefetch Policy (SPEC-HAL-134 Phase 6)
────────────────────────────────────────────────────────────────────────────────

F.1 Authority Registration

**Prefetch Authority**: `compute_realized_range_from_visible()`
- Location: `src/widgets/list.rs`
- Policy: Symmetric expansion by PREFETCH_ITEMS (currently 2)
- See SPEC-HAL-134 Section 16 for full specification

F.2 Base Invariants

INV-OVERSCAN-1
    Overscan is declarative. It specifies extra items beyond visible range.

INV-OVERSCAN-2
    Overscan is symmetric (before and after visible range) unless configured otherwise.

INV-OVERSCAN-3
    Overscan does not affect content_extent or scroll bounds.

INV-OVERSCAN-4
    Overscan reduces binding churn during small scrolls.

────────────────────────────────────────────────────────────────────────────────

F.3 Prefetch Window Invariants (Formalized in SPEC-HAL-134 Phase 6)

INV-PREFETCH-1 (Phase 6)
    realized_range is derived ONLY from visible_range + fixed prefetch parameters.
    No scroll velocity, frame history, or adaptive behavior permitted.

INV-PREFETCH-2 (Phase 6)
    Prefetch expansion is symmetric: (visible.start - P) .. (visible.end + P).

INV-PREFETCH-3 (Phase 6)
    realized_range is hard-clamped to [0, item_count]. No overflow, no wrap.

INV-PREFETCH-4
    Prefetch window must NOT affect scroll offset calculation directly.
    Prefetch extends realization only; scroll position remains derived from
    visible_range and item metrics without prefetch influence.

INV-PREFETCH-5
    Prefetch window must NOT trigger additional layout passes.
    Prefetch bounds are computed once during layout, alongside visible_range.
    No separate pass or deferred expansion is permitted.

INV-PREFETCH-6
    Prefetch window must NOT change realized_range ordering stability across
    frames if visible_range unchanged.
    Given identical visible_range between frames, realized_range must be
    identical. No reordering, no churn, no creation/destruction of widgets.

────────────────────────────────────────────────────────────────────────────────

F.4 Secondary Expansion Prohibition (SPEC-HAL-134 Phase 6)

INV-PREFETCH-NO-SECONDARY-1
    Consumers of realized_range MUST NOT re-expand the range.
    The range returned by compute_realized_range_from_visible() is final.

INV-PREFETCH-NO-SECONDARY-2
    Consumers MUST NOT apply additional overscan, padding, or "safety margin".

INV-PREFETCH-NO-SECONDARY-3
    If a consumer appears to need additional expansion, the correct action is:
    1. Propose an SPEC amendment to increase PREFETCH_ITEMS
    2. Move the logic into compute_realized_range_from_visible()
    Secondary expansion is architecturally illegitimate.

────────────────────────────────────────────────────────────────────────────────

F.5 Decoupling Invariant

INV-VIRT-DECOUPLE-1
    realized_range MUST be a pure function of:
    - visible_range
    - item_count
    - prefetch policy (PREFETCH_ITEMS constant)
    It MUST NOT read:
    - scroll offset directly
    - scroll deltas
    - scroll velocity
    - input events
    - item metrics (height removed in Phase 6 - symmetric item-count prefetch)
    Scroll influences realization ONLY via visible_range.

INV-VIRT-COMPLETE-1
    Virtualization is considered complete iff all of the following hold:
    - visible_range depends ONLY on scroll offset + immutable geometry.
    - realized_range depends ONLY on visible_range + metrics + prefetch.
    - Realization does NOT read input events, scroll deltas, velocity, or layout state.
    - Re-running layout with identical inputs MUST NOT change:
      - visible_range
      - realized_range
      - widget identity ordering.
    This invariant is a *closure condition* — no new behavior may violate it
    without a new SPEC.

INV-VIRT-CONSUMER-1 (SPEC-HAL-134 Phase 3)
    Downstream consumers MUST treat realized_range as authoritative.
    Consumers MUST NOT:
    - Recompute visible_range or realized_range
    - Clamp, extend, or modify the range
    - Apply correction or compensation
    - Reinterpret the range based on external state

    Enforcement: Debug assertion in `post_layout_populate_input_projections()`.
    File: src/core/app.rs

────────────────────────────────────────────────────────────────────────────────
SECTION G: Variable Item Height
────────────────────────────────────────────────────────────────────────────────

INV-HEIGHT-1
    Each RowId may have an associated measured height.

INV-HEIGHT-2
    Unknown heights use an estimate until measured.

INV-HEIGHT-3
    Prefix-sum structure maps index ↔ cumulative height.

INV-HEIGHT-4
    scroll_offset → index mapping uses binary search over prefix sums.

INV-HEIGHT-5
    Height changes require anchor stabilization:
    The anchor RowId's screen Y must remain invariant.

INV-HEIGHT-6
    Height changes must not cause scroll jumps.

INV-HEIGHT-7
    Measurement is lazy. Items are measured on first realization.

────────────────────────────────────────────────────────────────────────────────

B.5 HeightModel

HeightModel MUST provide:

    known_height(RowId) -> Option<f32>   // Measured height if available
    estimated_height(RowId) -> f32       // Must be > 0, monotonic per RowId
    total_extent                         // = sum(known + estimated for all items)

INV-HEIGHT-MODEL-1
    Unknown heights MUST use estimation.

INV-HEIGHT-MODEL-2
    Updating a measured height MUST preserve anchor screen position.

INV-HEIGHT-MODEL-3
    Estimation convergence MUST NOT cause visible scroll jumps.

────────────────────────────────────────────────────────────────────────────────
SECTION H: Layout & Spatial Invariants
────────────────────────────────────────────────────────────────────────────────

INV-LAY-1
    Virtualized content reports content_extent representing full logical size.

INV-LAY-2
    For fixed-height: content_height = item_count × item_height

INV-LAY-3
    For variable-height: content_height = sum(all logical item heights)

INV-LAY-4
    Widget Y position is derived from logical index, not slot index.

INV-LAY-5
    Scroll position changes visible index range, not widget binding order.

INV-LAY-6
    No widget may be positioned relative to another widget's rect.

INV-LAY-PLACEMENT-1
    Each realized slot MUST be laid out at its logical position in content space
    (derived from index → cumulative extent).

INV-LAY-PLACEMENT-2
    ScrollContainer applies the sole translation by -logical_offset.
    Content MUST NOT counter-translate or stack slots at origin.

────────────────────────────────────────────────────────────────────────────────

H.2 Axis Generalization

INV-AXIS-GENERAL
    All virtualization and scroll invariants apply independently per axis.
    Vertical and horizontal virtualization MUST compose.

INV-AXIS-NO-VERTICAL-ONLY
    No invariant, data structure, or algorithm in this SPEC assumes vertical-only scrolling.
    Horizontal virtualization uses identical logic with width substituted for height.

────────────────────────────────────────────────────────────────────────────────
SECTION I: Dataset Mutation Handling
────────────────────────────────────────────────────────────────────────────────

| Mutation               | Anchor Exists        | Expected Result                    |
|------------------------|----------------------|------------------------------------|
| Insert before viewport | Yes                  | Anchor preserved, offset adjusts   |
| Insert before viewport | No                   | Offset increases by item height    |
| Insert in viewport     | Yes (not at anchor)  | Anchor preserved, visible shifts   |
| Insert in viewport     | Yes (at anchor)      | Anchor preserved, new item below   |
| Insert after viewport  | Any                  | No scroll change                   |
| Remove before viewport | Yes                  | Anchor preserved, offset adjusts   |
| Remove before viewport | No                   | Offset decreases by item height    |
| Remove in viewport     | Yes (not anchor)     | Anchor preserved, visible shifts   |
| Remove in viewport     | Yes (anchor removed) | Fallback to nearest, then absolute |
| Remove after viewport  | Any                  | No scroll change                   |

INV-MUTATE-1
    Mutations increment ViewEpoch before any realization.

INV-MUTATE-2
    Realization re-computes visible range after mutation.

INV-MUTATE-3
    Anchor fallback is deterministic: prefer next item, then previous, then absolute.

────────────────────────────────────────────────────────────────────────────────
SECTION J: Selection & Focus Invariants
────────────────────────────────────────────────────────────────────────────────

INV-SEL-1
    Selection is a set of RowId.

INV-SEL-2
    Focus is a RowId or None.

INV-SEL-3
    No selection or focus state stored in widgets.

INV-SEL-4
    Visual selection state is pure projection: is_selected = selection.contains(row_id)

INV-SEL-5
    Selection persists across scroll, pagination, filter, sort (unless explicitly cleared).

INV-SEL-6
    Index-based operations (shift-selection) resolve indices at action time using DatasetView.

AUTHORITY: "Selection is logical, not visual"
    Selection operates on RowId. Visual representation follows.

────────────────────────────────────────────────────────────────────────────────
SECTION K: Pagination Invariants
────────────────────────────────────────────────────────────────────────────────

INV-PAGE-1
    Pagination is a DatasetView window, not a layout mode.

INV-PAGE-2
    Page size and page number affect only DatasetView, not ScrollContainer.

INV-PAGE-3
    "Show all" = page window equals full dataset.

INV-PAGE-4
    Pagination does not destroy selection unless explicitly configured.

INV-PAGE-5
    Changing page parameters increments ViewEpoch.

AUTHORITY: "Pagination compatibility"
    Pagination and virtualization compose without special cases.

────────────────────────────────────────────────────────────────────────────────
SECTION L: Filtering & Sorting Semantics
────────────────────────────────────────────────────────────────────────────────

| Operation       | ViewEpoch | Selection | Scroll Behavior                     |
|-----------------|-----------|-----------|-------------------------------------|
| Apply filter    | Advances  | Preserved | Reset to top OR anchor if preserved |
| Remove filter   | Advances  | Preserved | Reset to top OR anchor if preserved |
| Change sort     | Advances  | Preserved | Anchor to focused item if exists    |
| Clear sort      | Advances  | Preserved | Anchor to focused item if exists    |

INV-FILTER-1
    Filtering removes items from DatasetView. It does not destroy them.

INV-FILTER-2
    Selection of filtered-out items is preserved but invisible.

INV-FILTER-3
    Removing filter restores visibility of preserved selection.

INV-SORT-1
    Sorting reorders DatasetView. RowId stability is preserved.

INV-SORT-2
    Focused item remains focused after sort (identity, not position).

────────────────────────────────────────────────────────────────────────────────
SECTION M: Drag & Drop Stability
────────────────────────────────────────────────────────────────────────────────

INV-DND-1
    Drag is an explicit session: start → update* → end | cancel

INV-DND-2
    Drag payload is captured at start and immutable.

INV-DND-3
    Drag payload is a collection of RowId.

INV-DND-4
    Dragging works with off-screen items.

INV-DND-5
    Virtualization updates visible range as scroll changes during drag.

INV-DND-6
    Drop resolution uses DatasetView at drop time, committing via RowId.

────────────────────────────────────────────────────────────────────────────────
SECTION N: Partial Visibility & Extremes
────────────────────────────────────────────────────────────────────────────────

INV-PARTIAL-1
    Partially visible items at viewport edges are fully realized.

INV-PARTIAL-2
    Clipping is visual only. Partial items receive full layout bounds.

INV-PARTIAL-3
    Zero items in DatasetView: content_extent = 0, no slots bound.

INV-PARTIAL-4
    Sparse dataset (few items, large viewport): realize all items, no virtualization overhead.

────────────────────────────────────────────────────────────────────────────────
SECTION O: Performance Invariants
────────────────────────────────────────────────────────────────────────────────

INV-PERF-1
    Scrolling must not allocate.

INV-PERF-2
    Binding must be O(visible_count), never O(item_count).

INV-PERF-3
    No per-scroll tree rebuilds.

INV-PERF-4
    No per-scroll text shaping without caching.

INV-PERF-5
    All scroll-induced changes must be frame-stable.

────────────────────────────────────────────────────────────────────────────────
SECTION P: Forbidden Patterns
────────────────────────────────────────────────────────────────────────────────

FORBID-VIRT-1
    Realization computing content_extent.
    Correct: Content extent is sum of all item heights, computed from metrics.

FORBID-VIRT-2
    Slot widgets persisting visual state across reuse.
    Correct: Visual state is recomputed from bound data on each bind.

FORBID-VIRT-3
    Layout-time dataset mutation.
    Correct: Dataset mutations happen before layout, increment ViewEpoch.

FORBID-VIRT-4
    Visible range depending on realized widgets.
    Correct: Visible range depends only on scroll state and metrics.

FORBID-VIRT-5
    Index used as identity.
    Correct: RowId is identity. Index is position in current view.

FORBID-VIRT-6
    Separate code paths for virtualized vs non-virtualized.
    Correct: Non-virtualized is window == full dataset.

FORBID-VIRT-7
    Coupling virtualization to specific widget type.
    Correct: Virtualization is generic over any slot widget.

FORBID-VIRT-SCROLL-MUTATE
    Realization logic writing to scroll state (logical_offset, anchor, content_extent).
    Correct: Realization reads scroll state, never writes it.

FORBID-VIRT-VERTICAL-ONLY
    Any code path, data structure, or algorithm that assumes vertical-only scrolling.
    Correct: All virtualization logic is axis-agnostic or dual-axis.

────────────────────────────────────────────────────────────────────────────────
SECTION Q: Authoritative Statements (referenced elsewhere by name)
────────────────────────────────────────────────────────────────────────────────

AUTHORITY-1: "Virtualization consumes scroll, never produces it"
    Realization reads scroll state from SPEC-HAL-131A. It never writes it.

AUTHORITY-2: "Non-virtualized is a degenerate case"
    All code paths handle non-virtualized as window == full dataset.

AUTHORITY-3: "Selection is logical, not visual"
    Selection operates on RowId. Widgets project selection state.

AUTHORITY-4: "Bounded realization pool"
    Slot count is fixed. It does not scale with dataset size.

────────────────────────────────────────────────────────────────────────────────
SECTION R: Text Editor Substrate
────────────────────────────────────────────────────────────────────────────────

Text editors use the same scroll + virtualization substrate with no exceptions.

R.1 Identity Mapping

    RowId ⇢ LineId                        // One line = one RowId
    HeightModel ⇢ LineHeightModel         // Per-line height (wrap-dependent)

R.2 Vertical Virtualization

INV-TEXT-VERT-1
    Vertical virtualization = line virtualization.
    visible_line_range computed from scroll_offset_y and viewport_height.

INV-TEXT-VERT-2
    Line heights may vary (soft-wrapped lines, code folding).
    HeightModel applies: known_height(LineId) or estimated_height(LineId).

INV-TEXT-VERT-3
    Prefix-sum structure: cumulative_y[line_index] = sum(line_heights[0..line_index]).

R.3 Horizontal Virtualization

INV-TEXT-HORIZ-1
    Horizontal extent = max(line_widths) OR explicit content_width from layout.

INV-TEXT-HORIZ-2
    For non-wrapping mode: content_width = max(rendered_line_width) across all lines.

INV-TEXT-HORIZ-3
    For soft-wrap mode: content_width = viewport_width (no horizontal scroll).

R.4 Scale Requirements

INV-TEXT-SCALE-1
    MUST support millions of lines without full realization.

INV-TEXT-SCALE-2
    MUST support extremely long lines (megabytes per line) via horizontal virtualization.

INV-TEXT-SCALE-3
    Simultaneous vertical + horizontal scrolling uses composed invariants.

R.5 Required Data Structures

    LineHeightModel:
        known_height(LineId) -> Option<f32>
        estimated_height(LineId) -> f32
        prefix_sum: Vec<f32> or lazy structure
        total_height: f32

    LineWidthCache (non-wrapping):
        max_known_width: f32
        width_at(LineId) -> Option<f32>

R.6 Anchor Stability

INV-TEXT-ANCHOR-1
    Anchor is LineId + offset_within_line + viewport_y.

INV-TEXT-ANCHOR-2
    Typing, deletion, or insertion preserves anchor screen position when possible.

────────────────────────────────────────────────────────────────────────────────
SECTION R2: What This SPEC Enables
────────────────────────────────────────────────────────────────────────────────

- Lists with millions of items
- Pagination + virtualization without conflict
- Multi-selection across pages and filtered views
- Drag & drop across virtualized ranges
- Smooth scrolling with fixed or variable heights
- Text editors with millions of lines
- Simultaneous vertical + horizontal virtualization
- Zero hacks, zero per-frame syncing tricks

────────────────────────────────────────────────────────────────────────────────
SECTION S: Acceptance Criteria
────────────────────────────────────────────────────────────────────────────────

SPEC-HAL-131B is complete when:

[ ] visible_index_range(scroll_offset, viewport_size, heights) is pure function
[ ] RealizationWindow.slot_count is constant regardless of DatasetView.len()
[ ] Slot binding test: bind(slot, row_id) → unbind(slot) → bind(slot, different_row_id) produces no state leak
[ ] Height estimation test: update measured height → anchor screen position unchanged
[ ] Mutation table test: each row produces expected scroll behavior per anchor/no-anchor
[ ] Non-virtualized test: logical_range == 0..len() produces identical scroll math
[ ] DatasetView abstraction test: works with mock that returns id_at() lazily
[ ] Axis independence test: horizontal virtualization uses same logic as vertical
[ ] Content extent test: content_extent = sum(all item heights), not sum(realized heights)
[ ] Slot position test: slot[i].y == cumulative_height[logical_index], not slot_index * fixed_height
[ ] No write to ScrollState from any realization code path
[ ] Text editor mode: LineId as RowId, LineHeightModel as HeightModel passes all invariants

────────────────────────────────────────────────────────────────────────────────
SECTION T: Consumer Audit Rules (SPEC-HAL-134 Phase 4)
────────────────────────────────────────────────────────────────────────────────

T.1 Audit Trigger

    grep "realized_range()" src/

is a manual audit trigger. Any new hit outside of:

    1. The trait definition (src/core/input_projection.rs)
    2. The implementation (src/widgets/list.rs)
    3. Existing documented consumers (src/core/app.rs)
    4. Test files (tests/*.rs)

requires:

    - Consumer review against INV-VIRT-CONSUMER-1
    - Addition of debug_assert! in the new consumer
    - SPEC update documenting the new consumption site

T.2 Current Consumer Registry

| File                  | Function                              | Added    |
|-----------------------|---------------------------------------|----------|
| src/core/app.rs       | post_layout_populate_input_projections| Phase 3  |

T.3 Forbidden Consumer Patterns

New consumers MUST NOT:

    - Recompute visible_range or realized_range
    - Clamp, extend, or modify the range
    - Apply correction or compensation
    - Reinterpret the range based on input, scroll, or layout state

Violations require a new SPEC to justify the deviation.

T.4 No Shadow Logic Rule (SPEC-HAL-134 Phase 5)

INV-NO-SHADOW-LOGIC-1
    Consumers MUST NOT introduce parallel logic that derives the same concept.

    One concept → one authority → one implementation.

Forbidden patterns:
    - Recomputing visible_range from scroll_offset (authority: List)
    - Re-deriving realized_range from scroll state (authority: visible_range + prefetch)
    - Computing bounds or eligibility from raw scroll data
    - Caching "corrected" or "adjusted" versions of authoritative ranges
    - Maintaining shadow state that duplicates the authority chain

If a consumer appears to need any of these, the correct action is to:
    1. Propose an SPEC amendment
    2. Move the logic into the authoritative source
    3. Expose additional query methods from the authority

Shadow derivation is architecturally illegitimate, not merely discouraged.

T.5 Slot–Index Mapping Authority (SPEC-HAL-134 Phase 7)

INV-SLOT-AUTHORITY-1
    The mapping between logical indices and slot indices has ONE authoritative source:
    - `slot_index_from_logical()` for logical → slot
    - `logical_index_from_slot()` for slot → logical

    The canonical formula is:
    ```
    slot_index = logical_index - realized_range.start
    ```

INV-SLOT-AUTHORITY-2
    Consumers MUST NOT:
    - Compute slot_index using alternative formulas
    - Cache slot mappings independently
    - Infer slot assignments from widget order
    - Maintain shadow slot state

Forbidden patterns:
    - `index - realized.start` inline (use helper)
    - `realized.start + widget_idx` inline (use helper)
    - Storing slot ↔ logical maps in consumer state
    - Deriving slot from children array position assumptions

If a consumer needs slot information, it MUST call the authoritative helpers.
Direct computation is forbidden regardless of correctness.

T.6 Widget Identity Authority (SPEC-HAL-134 Phase 8)

INV-ID-AUTHORITY-1
    Widget identity is determined by **logical index**, not by:
    - slot index
    - widget pointer
    - pool position
    - creation order
    - reuse history

INV-ID-AUTHORITY-2
    When a widget is reused for a different logical index:
    - `on_rebind(new_logical_index)` MUST be called
    - All transient state MUST be cleared
    - Silent reuse is forbidden

    The check is:
    ```
    needs_rebind = widget.last_logical_index() != Some(new_logical_index)
    ```

INV-ID-AUTHORITY-3
    State preservation is permitted **if and only if**:
    ```
    previous_logical_index == current_logical_index
    ```

    No slot-based, pool-based, or heuristic preservation is permitted.

Forbidden patterns:
    - Preserving widget state when logical index changes
    - Using slot index to determine identity
    - Skipping rebind for "optimization"
    - Implicit state carryover across reuse
    - Pool-history–based identity decisions

If a widget needs to preserve state across logical index changes, that state
must be stored externally (e.g., in ListState) using stable identifiers.
Widget-internal transient state resets on rebind.

T.7 Selection & Focus Externalization (SPEC-HAL-134 Phase 9)

INV-SEL-EXTERN-1
    Slot widgets (e.g., ListItem) MUST NOT contain internal fields for:
    - selection (`is_selected: bool`)
    - focus (`is_focused: bool`)
    - hover (`is_hovered: bool`)
    - pressed/active state

INV-SEL-EXTERN-2
    Selection and focus authority MUST live in the parent's state model:
    - `ListState.selected_ids: HashSet<LogicalItemId>`
    - `ListState.focused_id: Option<LogicalItemId>`

    These are the ONLY authoritative sources for selection/focus.

INV-SEL-EXTERN-3
    Slot widgets derive visual state via provider closure:
    ```rust
    fn visual_state(&self) -> ListItemVisualState {
        self.visual_state_provider.map(|p| p(self.index)).unwrap_or_default()
    }
    ```
    The provider queries ListState. No caching within the widget.

Audit Trigger:

    grep "is_selected\|is_focused\|is_hovered" src/widgets/list_item.rs

If any such fields exist, this is a violation. Selection/focus MUST survive
widget reuse, which is only possible with external authority.

Forbidden patterns:
    - `struct ListItem { is_selected: bool, ... }`
    - Widget caching selection state
    - Selection lost on on_rebind()
    - Selection derived from slot or pool position
    - Any selection state in the widget rather than ListState

T.8 Input Projection Stability (SPEC-HAL-134 Phase 10)

INV-INPUT-PROJ-1
    Input dispatch ALWAYS targets LogicalItemId, never widget identity.

    The routing path is:
    ```
    pointer event → hit_test_projected() → ProjectedHitResult
                  → dispatch_projected_input() → handle_projected_action()
                  → container handles via logical_index
    ```

    Widget identity (TreeWidgetId) is an OPTIONAL optimization hint,
    not a routing key.

INV-INPUT-PROJ-2
    Slot reuse does NOT affect click targeting.

    When slots are reused after scroll:
    - Old: Slot B renders item 1
    - New: Slot B renders item 3

    A click at the same viewport position targets item 3,
    not "whatever was in slot B before".

INV-INPUT-PROJ-3
    Hit testing is stateless and pure.

    `InputProjection::hit_test(x, y)` returns the same result for the same
    inputs regardless of previous calls. There is no cached target,
    no "last hit" memory, no sticky binding.

INV-INPUT-PROJ-4
    Focus survives widget reuse.

    Because focus is stored in `ListState.focused_id` (a LogicalItemId),
    it survives scroll that causes widget reuse. The focus index doesn't
    change just because the widget slot changed.

INV-INPUT-PROJ-5
    Pointer capture does NOT bind to virtualized item widgets.

    ListItem handles clicks as immediate Down→Up sequences.
    It does NOT capture the pointer for drag operations.
    Container (List) may capture for scroll, but that targets the
    container, not individual items.

Audit Triggers:

    grep "pointer_capture\|PointerCapture" src/widgets/list_item.rs

If ListItem captures pointer, this is a potential violation.
Pointer capture + reuse = stale capture target.

    grep "widget_id\|TreeWidgetId" src/core/event_router.rs | grep dispatch

If dispatch uses widget_id as a PRIMARY routing key (not just
optimization/realization lookup), review for INV-INPUT-PROJ-1 violation.

Forbidden patterns:
    - Routing input by widget identity instead of logical identity
    - Caching hit-test results across frames
    - "Sticky" click targets that persist after scroll
    - ListItem capturing pointer
    - Selection/focus dispatch targeting widget instead of container
    - Input decisions based on slot index or realization order

────────────────────────────────────────────────────────────────────────────────
