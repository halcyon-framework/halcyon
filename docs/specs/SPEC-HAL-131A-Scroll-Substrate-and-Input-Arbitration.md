SPEC-HAL-131A
Scroll Substrate & Input Arbitration

Status: Binding / Locked
Locked: 2025-12-20
Note: Any future behavior change requires a new SPEC.
Scope: Scroll state, input arbitration, scrollbar semantics, resize behavior
Layer: Core infrastructure (widget-agnostic, virtualization-agnostic)
Companion: SPEC-HAL-131B (Realization & Virtualization)
Depends on: WidgetTree, Layout, EventRouter
Explicit goal: Define scroll as pure math + state before any widget concerns

────────────────────────────────────────────────────────────────────────────────
SECTION A: Problem Statement
────────────────────────────────────────────────────────────────────────────────

Scroll has historically been conflated with:
- Widget layout
- Content realization
- Virtualization logic
- Input handling

This SPEC separates scroll substrate from all consumers.

ScrollContainer owns scroll. Nothing else does.

────────────────────────────────────────────────────────────────────────────────
SECTION B: Formal Objects
────────────────────────────────────────────────────────────────────────────────

B.1 ScrollState

    struct ScrollState {
        logical_offset: Vec2,        // Current scroll position
        anchor: Option<ScrollAnchor>, // Exclusive with absolute offset
        content_extent: Size,        // Total scrollable content size
        viewport_extent: Size,       // Visible region size
    }

INV-STATE-1
    Exactly one of (anchor, absolute offset) is authoritative at any moment.
    Anchor mode: logical_offset derived from anchor + offset.
    Absolute mode: anchor is None.

INV-STATE-2
    logical_offset is always clamped:
    0 ≤ logical_offset.x ≤ max(0, content_extent.width - viewport_extent.width)
    0 ≤ logical_offset.y ≤ max(0, content_extent.height - viewport_extent.height)

INV-STATE-3
    content_extent is reported by content, not computed by ScrollContainer.

INV-STATE-4
    viewport_extent is determined by ScrollContainer's layout constraints.

────────────────────────────────────────────────────────────────────────────────

B.2 ScrollAnchor

    struct ScrollAnchor {
        target_id: AnchorId,         // Stable identity (RowId, NodeId, etc.)
        target_offset: f32,          // Offset within target
        viewport_offset: f32,        // Target's position in viewport
    }

INV-ANCHOR-1
    Anchor survives content mutations that preserve target_id.

INV-ANCHOR-2
    If anchor target is removed, anchor falls back to nearest valid target or absolute mode.

INV-ANCHOR-RESOLVE-1
    ScrollAnchor MUST be resolved to logical_offset before layout for the frame.

INV-ANCHOR-RESOLVE-2
    Paint MUST consume resolved logical_offset only and MUST NOT resolve anchors.

────────────────────────────────────────────────────────────────────────────────

B.3 InputGesture

    struct InputGesture {
        axis: Axis,                  // Horizontal, Vertical, or Both
        source: GestureSource,       // Wheel, Trackpad, Thumb, Keyboard, Programmatic
        lifetime: GestureLifetime,   // Discrete, Continuous, Inertial
        priority: u8,                // Arbitration priority
    }

INV-GESTURE-1
    At most one active gesture per axis at any time.

INV-GESTURE-2
    Higher priority gesture preempts lower priority on same axis.

INV-GESTURE-3
    Gesture produces scroll intent, never layout changes.

────────────────────────────────────────────────────────────────────────────────
SECTION C: Scroll Core Invariants
────────────────────────────────────────────────────────────────────────────────

INV-SCROLL-CORE-1
    ScrollContainer is sole owner of: logical_offset, viewport_extent, scroll clipping, coordinate transforms.

INV-SCROLL-CORE-2
    ScrollContainer exposes to content during layout:
    - viewport_width, viewport_height
    - scroll_offset_x, scroll_offset_y

INV-SCROLL-CORE-3
    ScrollContainer applies scroll translation and clipping exactly once during painting.

INV-SCROLL-CORE-4
    ScrollContainer routes pointer events in scrolled coordinates before hit-testing content.

INV-SCROLL-CORE-5
    ScrollContainer never inspects, modifies, or interprets content children layout.

INV-SCROLL-CORE-6
    Programmatic scroll (scroll_to, scroll_by) mutates logical_offset only, never content.

INV-SCROLL-CORE-7
    Scroll bounds are hard clamped. Overscroll is visual effect only, never actual state.

INV-SCROLL-BOUNDARY-STRICT
    ScrollViewportInfo (viewport_extent, logical_offset) is delivered only via LayoutContext
    to the immediate scroll content.
    No widget may query parent nodes, widget tree, or global state to infer scroll position.

────────────────────────────────────────────────────────────────────────────────
SECTION D: Content Extent Contract
────────────────────────────────────────────────────────────────────────────────

INV-EXTENT-1
    Content reports content_extent. ScrollContainer accepts it without question.

INV-EXTENT-2
    content_extent may differ from realized widget bounds.

INV-EXTENT-3
    content_extent >> realized content size is explicitly supported.

INV-EXTENT-4
    ScrollContainer does not infer content_extent from children.

────────────────────────────────────────────────────────────────────────────────
SECTION E: Scrollbar Semantics
────────────────────────────────────────────────────────────────────────────────

INV-THUMB-1
    Thumb position is pure function of: logical_offset, content_extent, viewport_extent.

INV-THUMB-2
    Thumb size = viewport_extent / content_extent (clamped to minimum).

INV-THUMB-3
    Thumb drag produces scroll intent proportional to track position.

INV-THUMB-4
    Virtualization is invisible to scrollbar. Scrollbar sees only ScrollState.

INV-THUMB-5
    Scrollbar MUST NOT reference: realization state, slot counts, realized widget bounds,
    or any data structure from SPEC-HAL-131B.

────────────────────────────────────────────────────────────────────────────────
SECTION F: Resize Behavior
────────────────────────────────────────────────────────────────────────────────

Resize behavior is deterministic per configuration.

| Resize Type        | Anchor Mode                  | Absolute Mode              |
|--------------------|------------------------------|----------------------------|
| Viewport shrinks   | Anchor preserved             | Offset clamped to max      |
| Viewport grows     | Anchor preserved             | Offset unchanged           |
| Content shrinks    | Anchor recalculated          | Offset clamped to max      |
| Content grows      | Anchor preserved             | Offset unchanged           |

INV-RESIZE-1
    Resize never causes content jump unless clamping is triggered.

INV-RESIZE-2
    Anchor mode prioritizes visual stability over offset stability.

INV-RESIZE-3
    Resize is processed before next layout, not during.

────────────────────────────────────────────────────────────────────────────────
SECTION G: Input Arbitration
────────────────────────────────────────────────────────────────────────────────

G.1 Source Priority (highest to lowest)

| Priority | Source                      | Lifetime              | Notes                              |
|----------|-----------------------------|-----------------------|------------------------------------|
| 1        | Thumb drag / direct manip   | Continuous            | Scrollbar thumb interaction        |
| 2        | Touch / Trackpad drag       | Continuous / Inertial | Two-finger scroll with momentum    |
| 3        | Wheel                       | Discrete              | Mouse wheel steps                  |
| 4        | Keyboard                    | Discrete              | Page Up/Down, Home/End, Arrows     |
| 5        | Programmatic                | Discrete (advisory)   | scroll_to, scroll_by, reveal       |

INV-INPUT-PRIORITY-STRICT
    Programmatic scroll requests MUST NOT preempt any active user gesture.
    If a higher-priority gesture is active, programmatic requests MUST be deferred or rejected.

────────────────────────────────────────────────────────────────────────────────

G.2 Keyboard Navigation Mapping

| Key         | Action                                              |
|-------------|-----------------------------------------------------|
| Up/Down     | Scroll by line height (or item height if known)     |
| Left/Right  | Scroll by character width (horizontal scroll only)  |
| Page Up     | Scroll by viewport_height - overlap                 |
| Page Down   | Scroll by viewport_height - overlap                 |
| Home        | Scroll to offset 0                                  |
| End         | Scroll to max offset                                |

INV-INPUT-1
    Keyboard scroll is consumed by innermost focused scrollable.

INV-INPUT-2
    Unfocused scroll containers do not consume keyboard input.

────────────────────────────────────────────────────────────────────────────────

G.3 Nested Scroll Chaining

INV-CHAIN-1
    Scroll event propagates outward only when inner container reaches boundary.

INV-CHAIN-2
    Boundary = logical_offset at min or max for scroll direction.

INV-CHAIN-3
    Partial consumption is forbidden. Container either consumes fully or propagates fully.

INV-CHAIN-4
    Inertial scroll transfers momentum to outer container on boundary hit (configurable).

────────────────────────────────────────────────────────────────────────────────

G.4 Gesture Lifecycle

INV-LIFECYCLE-1
    Discrete gestures: single intent per event.

INV-LIFECYCLE-2
    Continuous gestures: start → update* → end.

INV-LIFECYCLE-3
    Inertial gestures: start → update* → coast → end.

INV-LIFECYCLE-4
    Cancellation returns to pre-gesture state (for continuous/inertial).

────────────────────────────────────────────────────────────────────────────────
SECTION G.5: Scroll Intent, Capture, and Arbitration — Binding Law
────────────────────────────────────────────────────────────────────────────────

This section establishes binding architectural law for scroll intent, capture,
and arbitration. These invariants are engine-grade, survive refactors, and
override widget preferences. Violations are engine bugs, not UX issues.

────────────────────────────────────────────────────────────────────────────────

G.5.1 Scroll Intent

INV-SCROLL-INTENT-1 — Scroll intent is singular

    At any moment, exactly ONE scroll target may consume scroll input.

    Scroll intent must not be implicitly reinterpreted.
    Hover and focus do NOT establish scroll authority.

    ALLOWED:
    - Explicit capture establishes scroll target
    - Captured widget exclusively receives scroll deltas

    FORBIDDEN:
    - Implicit scroll redirection
    - Scroll bubbling between widgets during active capture
    - Automatic reassignment of scroll target

────────────────────────────────────────────────────────────────────────────────

INV-SCROLL-INTENT-2 — Scroll capture is explicit

    A widget may capture scroll input ONLY after an explicit user action
    indicating intent to interact with that widget.

    Explicit actions include:
    - Pointer down inside the widget
    - Scrollbar drag initiation
    - Explicit "enter / activate" gesture (keyboard Enter on focused scrollable)

    FORBIDDEN:
    - Capture on hover
    - Capture on focus alone
    - Capture on pointer movement without button press
    - Capture solely because a widget is scrollable

────────────────────────────────────────────────────────────────────────────────

INV-SCROLL-INTENT-3 — Captured scroll is exclusive

    While scroll is captured by a widget:

    - All scroll deltas route exclusively to that widget
    - No bubbling to parent or ancestor scroll areas
    - No fallback behavior

    If scroll bounds are reached, NO other scroll target moves.
    The capturing widget "swallows" all scroll intent until release.

────────────────────────────────────────────────────────────────────────────────

INV-SCROLL-INTENT-4 — Scroll release is explicit

    Scroll capture is released ONLY by an explicit release action:

    - Pointer up
    - Gesture end (trackpad lift, touch end)
    - Explicit exit from interaction mode (Escape, focus loss by user action)

    FORBIDDEN:
    - Reaching scroll limits does NOT release capture
    - Timeout-based release
    - Implicit release on any non-user-initiated condition

────────────────────────────────────────────────────────────────────────────────

G.5.2 User Scroll Dominance

INV-SCROLL-ARBITRATION-1 — User scroll dominates programmatic scroll

    Programmatic scroll (including focus-related reveal, anchor resolution,
    scroll_to, scroll_by) must NOT preempt an active user scroll interaction.

    If a scroll gesture is active (drag, wheel momentum, trackpad inertia):
    - Programmatic scroll requests MUST be skipped or deferred
    - No exception for "urgent" or "critical" reveals

    User physical input has absolute priority over engine-initiated scroll.

────────────────────────────────────────────────────────────────────────────────
SECTION H: IME/Text Safety
────────────────────────────────────────────────────────────────────────────────

INV-IME-1
    Scroll input MAY occur during IME composition.

INV-IME-2
    Scroll MUST NOT invalidate IME composition anchor or caret geometry.

INV-IME-3
    Programmatic reveal-caret scroll is permitted during IME.

INV-IME-4
    Gestures that would mutate caret/selection are restricted, not scrolling itself.

────────────────────────────────────────────────────────────────────────────────
SECTION I: Forbidden Patterns
────────────────────────────────────────────────────────────────────────────────

FORBID-SCROLL-1
    Widgets reading scroll_offset directly.
    Correct: Widgets receive position from layout.

FORBID-SCROLL-2
    Layout mutating scroll state.
    Correct: Scroll state is input to layout, never output.

FORBID-SCROLL-3
    ScrollArea inferring content_extent from children bounds.
    Correct: Content explicitly reports extent.

FORBID-SCROLL-4
    Frame-time dependent scroll math.
    Correct: Scroll math is deterministic given inputs.

FORBID-SCROLL-5
    Scroll callbacks mutating layout mid-pass.
    Correct: Scroll changes schedule next layout pass.

FORBID-SCROLL-6
    Coupling scroll logic to specific widget types.
    Correct: Scroll substrate is widget-agnostic.

FORBID-SCROLL-QUERY
    Any widget or wrapper querying scroll_offset via tree traversal or parent inspection.
    Correct: Scroll info is delivered via LayoutContext to immediate content only.

FORBID-SCROLL-REALIZATION
    Scroll math depending on realization state, slot counts, or realized widget bounds.
    Correct: Scroll math depends only on logical_offset, content_extent, viewport_extent.

FORBID-SCROLL-LATE-RESOLVE
    Anchor resolution occurring at paint time or after layout.
    Correct: Anchor MUST be resolved to logical_offset before layout begins.

FORBID-SCROLL-FOCUS-ENTRY
    Scroll mutations originating from focus entry.
    Scroll state MUST NOT change when focus transitions from None → Some(widget).
    Cross-reference: SPEC-HAL-038 / INV-FOCUS-ENTRY-SCROLL-NEUTRALITY.

────────────────────────────────────────────────────────────────────────────────

I.2 Explicitly Forbidden UX Behaviors (Binding)

The following behaviors are explicitly forbidden. These are not suggestions
or preferences — they are architectural violations. Code exhibiting these
behaviors is defective and must be fixed.

FORBID-SCROLL-HIJACK-HOVER
    Scroll hijacking on hover.
    A widget must not capture scroll input simply because the pointer is over it.
    Scroll capture requires explicit user action (pointer down, drag start).

FORBID-VALUE-CHANGE-WHEEL
    Accidental value changes via mouse wheel.
    Widgets with adjustable values (sliders, spinboxes, dropdowns) must not
    change their value on wheel input unless they have explicit scroll capture.
    Hovering over a widget and scrolling must not mutate widget state.

FORBID-NESTED-SCROLL-INTERCEPT
    Nested scroll interception without capture.
    Inner scrollable widgets must not intercept scroll deltas intended for
    outer containers unless the inner widget has explicit scroll capture.

FORBID-SCROLL-LEAK-AT-BOUNDS
    Scroll leaking when inner widget reaches bounds.
    When a captured widget reaches its scroll limits, scroll deltas must NOT
    leak to parent containers. The capturing widget swallows all scroll until
    capture is explicitly released.

FORBID-CONTAINER-SELF-REVEAL
    Containers scrolling themselves into view on focus.
    Container widgets (ScrollArea, List, FlowGrid, panels) must NOT trigger
    focus-follow-scroll for their own rect. Only leaf interactive widgets
    may request scroll reveal. Container focus is navigational only.

FORBID-TIMING-HACKS
    Timing-based scroll hacks.
    First-tick masking, delayed release, frame-count guards, and similar
    temporal workarounds are forbidden. Scroll behavior must be deterministic
    and frame-timing-independent.

────────────────────────────────────────────────────────────────────────────────
SECTION J: Global Architectural Invariants
────────────────────────────────────────────────────────────────────────────────

INV-LAYOUT-CONTEXT-PURITY

LayoutContext is layout-semantic only.

LayoutContext MUST NOT:
- Carry feature-specific queues, schedulers, registries, or state
- Carry post-layout or post-paint responsibilities
- Carry widget identity for non-layout purposes
- Be extended to support anchor resolution or any non-layout subsystem

Anchor resolution is a post-layout, pre-paint concern and MUST NOT be
implemented by modifying LayoutContext, layout traversal, or layout
call signatures.

Any design that requires LayoutContext modification for anchor
resolution is invalid by definition, regardless of test coverage.

────────────────────────────────────────────────────────────────────────────────

Stage Constraints:

Stage 13 — Anchor Resolution Placement
    Constrained by INV-LAYOUT-CONTEXT-PURITY
    Status: IMPLEMENTED (post_layout_resolve_anchors declared on ScrollArea)

Stage 14 — Frame Wiring
    Constrained by INV-LAYOUT-CONTEXT-PURITY
    Status: IMPLEMENTED (post_layout_resolve_scroll_anchors wired in build_scene)

Stage 15 — Virtualization-Realization-Scroll Coupling
    Constrained by INV-REALIZE-ORDER-1 through INV-REALIZE-ORDER-4
    Status: IMPLEMENTED (ordering verified, realization completes before anchor resolution)

Stage 16 — Scroll Dirty Semantics
    Constrained by INV-SCROLL-DIRTY-1 through INV-SCROLL-DIRTY-3
    Status: IMPLEMENTED (scroll write paths audited, no realization triggers found)

────────────────────────────────────────────────────────────────────────────────

Stage 14 Implementation Notes:

    Entry Point:
        post_layout_resolve_scroll_anchors() in src/core/app.rs
        Called from build_scene() between layout_tree() and paint_tree()

    Mechanism:
        - Engine-owned: No Widget trait hooks, no layout-time resolution
        - Uses WidgetTree::node_and_node_mut for safe disjoint borrows
        - ScrollArea::post_layout_resolve_anchors delegates to internal resolver
        - No unsafe code in anchor resolution path

    Invariants:
        - Single authoritative entry point (engine phase)
        - Tree topology stable during resolution
        - Anchor → logical_offset before paint
        - Paint never accesses anchor directly

────────────────────────────────────────────────────────────────────────────────

Stage 15 — Virtualization-Realization-Scroll Coupling

    Establishes ordering between virtualization realization and scroll anchor resolution.

    Invariants:

    INV-REALIZE-ORDER-1
        Virtualization realization MUST complete before post_layout_resolve_scroll_anchors.
        Realization occurs DURING layout_tree(), not after.

    INV-REALIZE-ORDER-2
        Scroll anchor resolution MUST NOT trigger realization.
        Anchor resolution reads realized geometry; it never instantiates widgets.

    INV-REALIZE-ORDER-3
        Realization changes MAY mark anchors dirty, but MUST NOT resolve them.
        Anchor resolution is deferred to the post-layout phase.

    INV-REALIZE-ORDER-4
        Paint phase MUST NOT observe virtual (unrealized) geometry.
        All widgets painted must have been laid out during layout_tree().

    Frame Pipeline:

        1. layout_tree()                        — widget layout + realization
           └─ List::layout()                    — height updates, visible range, child layout
        2. post_layout_resolve_scroll_anchors() — anchor resolution (reads realized geometry)
        3. paint_tree()                         — scene graph (only realized widgets)

    Key Functions:

        Realization:
            - List::layout() in src/widgets/list.rs:586
            - Processes pending height updates (Stage 0)
            - Computes visible range (Stage 5)
            - Lays out children at logical positions (Stage 9)

        Anchor Resolution:
            - post_layout_resolve_scroll_anchors() in src/core/app.rs:234
            - Iterates ScrollArea nodes
            - Calls content.as_anchor_resolver() to get resolver
            - Resolves anchors to logical_offset

    Status: IMPLEMENTED (ordering verified, no violations found)

────────────────────────────────────────────────────────────────────────────────

Stage 16 — Scroll Dirty Semantics

    Locks how scroll offset changes affect virtualization without
    pulling realization or layout prematurely.

    Invariants:

    INV-SCROLL-DIRTY-1
        Scroll offset changes MAY mark virtualization state dirty.
        Scroll offset changes MUST NOT trigger realization immediately.
        Realization is deferred to the next layout_tree() pass.

    INV-SCROLL-DIRTY-2
        Realization only occurs during layout_tree().
        No scroll mutation path may invoke List::layout() or similar.

    INV-SCROLL-DIRTY-3
        Anchor resolution MUST NOT mark virtualization dirty.
        Anchor resolution reads realized geometry; it never instantiates.

    Audit Results:

        All scroll write paths in scroll_state.rs and scroll_area.rs
        only mutate scroll state (offset, anchor, dirty flags, extents).
        None trigger realization. Verified paths:

        scroll_state.rs:
            - set_content_extent, set_viewport_extent
            - set_offset, set_offset_x, set_offset_y
            - scroll_by, scroll_by_x, scroll_by_y
            - extend_content_height/width
            - prepend_content_height/width
            - trim_content_top/bottom/left/right
            - set_anchor, clear_anchor, mark_anchor_dirty
            - resolve_anchor, jump_to_anchor

        scroll_area.rs:
            - All public APIs delegate to ScrollState
            - handle_event scroll handling: state.scroll_by()
            - handle_scrollbar_drag: state.set_offset_y/x()

        Realization occurs ONLY in List::layout() during layout_tree().

    Status: IMPLEMENTED (audit complete, no violations found)

────────────────────────────────────────────────────────────────────────────────
SECTION K: Authoritative Statements (referenced elsewhere by name)
────────────────────────────────────────────────────────────────────────────────

AUTHORITY-1: "Scroll owns space"
    ScrollContainer is the sole authority on scroll position and viewport geometry.
    No widget, layout, or realization logic may override this.

AUTHORITY-2: "Widgets must not read scroll state"
    Widgets receive their position via layout. They never query scroll offset.

AUTHORITY-3: "Virtualization is transparent to scrollbar"
    Scrollbar math depends only on ScrollState. Virtualization details are invisible.

────────────────────────────────────────────────────────────────────────────────
SECTION L: Pre-Layout Scroll Synchronization
────────────────────────────────────────────────────────────────────────────────

Added: 2025-12-23 (virtualization black gap fix)

This section formalizes the pre-layout scroll sync mechanism that prevents
1-frame lag between scroll offset and layout, which causes virtualization
corruption (black gaps, overlapping items).

────────────────────────────────────────────────────────────────────────────────

L.1 Problem Statement

The post-layout scroll sync pattern (Stage 15) creates a 1-frame lag:

    Frame N:
        layout_tree()                       — uses OLD visible_range
        post_layout_sync_scroll_to_lists()  — updates visible_range for NEXT frame
        paint_tree()                        — scroll transform applied to WRONG positions

Result: Children positioned at old visible_range, scroll transform moves them
off-screen → black gaps, overlap, visual corruption.

Fix: Scroll→List sync MUST occur BEFORE layout so layout uses CURRENT scroll_y.

────────────────────────────────────────────────────────────────────────────────

L.2 Pre-Layout Scroll Sync Invariants

INV-PRE-SYNC-1 — Scroll Observation Before Layout
    Any widget whose layout depends on scroll offset MUST observe the current
    scroll offset BEFORE layout_tree() executes.

    Applies to:
    - List.visible_range
    - Any future virtualized container
    - Any widget performing realization windows

    Enforcement:
    - pre_layout_sync_scroll_to_lists() called BEFORE layout_tree()
    - Uses current scroll_y from ScrollArea.target_scroll_y()

INV-PRE-SYNC-2 — Phase Correctness
    Pre-layout sync MUST occur while Lists are in ScrollCompute phase
    (default state between frames).

    Forbidden:
    - Calling update_visible_range_from_scroll() during LayoutApply phase

INV-PRE-SYNC-3 — Layout Dirty Propagation
    If pre-layout sync changes visible_range, the List MUST be marked
    layout-dirty in the same frame.

    Enforcement:
    - update_visible_range_from_scroll() returns change flag
    - Caller calls tree.mark_layout_dirty(list_id) when changed

────────────────────────────────────────────────────────────────────────────────

L.3 ScrollArea Content Coverage Invariant

INV-SA-GEOM-1 — Content Rect Coverage
    ScrollArea MUST assign its content child a rect height >= viewport_height.

    Rationale:
    In virtualized mode, the realized content height may be smaller than
    the viewport (e.g., only 25 items realized when viewport shows 30).
    The child rect MUST still cover the viewport to prevent black gaps.

    child_rect_height = max(content_layout_result.size.height, viewport_height)
    child_rect_width  = max(content_layout_result.size.width, viewport_width)

    Clarification:
    - This is VISUAL coverage, not scroll extent
    - content_size still represents full logical content (e.g., 100k items)
    - child_rect is allowed to be larger than realized content

────────────────────────────────────────────────────────────────────────────────

L.4 Frame Pipeline Ordering (Tightened)

    The frame pipeline order is a STABLE INVARIANT. Reordering is forbidden.

    0. pre_layout_sync_scroll_to_lists()       — sync scroll offset BEFORE layout
    1. layout_tree()                           — compute widget geometry
    2. post_layout_populate_input_projections()— build input projection snapshots
    3. post_layout_resolve_scroll_anchors()    — resolve scroll anchors
    4. post_layout_sync_scroll_to_lists()      — sync for anchor changes
    5. paint_tree()                            — build scene graph

    INV-PIPELINE-PRE-SYNC
        Pre-layout sync (step 0) handles scroll input (wheel, drag).
        Ensures layout sees current scroll_y.

    INV-PIPELINE-POST-SYNC
        Post-layout sync (step 4) handles anchor resolution effects.
        Scroll anchors may change offset, which needs re-sync.

    INV-PIPELINE-ORDER
        The ordering is fixed. Reordering any step is forbidden.
        Each step depends on outputs from previous steps.

────────────────────────────────────────────────────────────────────────────────

L.5 Scroll Sync Scope Isolation

INV-SCROLL-SYNC-SCOPE-1 — Nested ScrollArea Isolation
    Scroll synchronization applies ONLY to descendants of a ScrollArea's
    content subtree and MUST NOT cross into nested ScrollAreas.

    Rationale:
    Each ScrollArea owns its own scroll offset. A child ScrollArea has
    independent scroll state that MUST NOT be overwritten by parent sync.

    Enforcement:
    - Traversal explicitly stops at nested ScrollAreas
    - This behavior is intentional, not incidental

    INV-SCROLL-SYNC-5 (from post_layout implementation) applies equally
    to pre_layout_sync_scroll_to_lists().

────────────────────────────────────────────────────────────────────────────────
L.6 Retained Vertical Position Across Reconstruction
────────────────────────────────────────────────────────────────────────────────

SPEC-HAL-366 defines explicitly opted-in sequential horizontal continuity using
HorizontalScrollLink. It does not change the live-only meaning of the pre-existing
horizontal attachment methods or the retained-Y invariants in L.6.

`VerticalScrollLink` is the owner-supported continuity primitive for replacing one
`ScrollArea` with another while retaining the presented vertical coordinate.
It is not a second scroll owner and is not a live multi-widget synchronization
mechanism.

INV-RETAINED-Y-1 — Sequential Single Writer
    At most one mounted `ScrollArea` may write a `VerticalScrollLink`.
    The link carries one mechanical scalar coordinate only. Simultaneously mounted
    linked scrolling continues to use `ScrollGroupId`.

INV-RETAINED-Y-2 — Deferred Final-Bounds Restore
    Attaching a `VerticalScrollLink` creates a pending one-shot restore.
    Measurement passes MUST NOT clamp or consume it. The first tight layout reads
    the retained coordinate only after final viewport and content extents exist,
    clamps it through `ScrollState`, and sets logical, target, and visual offsets
    to the same result before paint.

INV-RETAINED-Y-3 — Virtualization Sees First-Frame Position
    While restoration is pending, pre-layout scroll synchronization observes the
    retained coordinate. ListView, TreeView, and other virtualized content
    therefore realize the retained region during the replacement's first layout,
    not one frame after it.

INV-RETAINED-Y-4 — Fresh Intent Wins
    Any explicit programmatic, pointer, scrollbar, proximity, anchor, or linked
    scroll intent received by the replacement before tight layout cancels the
    pending restore. A stale retained coordinate MUST NOT overwrite newer intent.

INV-RETAINED-Y-5 — First-Paint Geometry
    Restoration is an immediate discontinuity, not smooth motion. The first paint
    after reconstruction uses the restored visual offset, and scrollbar thumb
    geometry is derived from that same clamped coordinate under INV-THUMB-1.

────────────────────────────────────────────────────────────────────────────────
SECTION K: Acceptance Criteria
────────────────────────────────────────────────────────────────────────────────

SPEC-HAL-131A is complete when:

[ ] ScrollState clamping passes: offset = clamp(offset, 0, max(0, content - viewport))
[ ] Anchor resolution test: anchor → logical_offset computed before layout callback fires
[ ] Scrollbar thumb position = logical_offset / (content_extent - viewport_extent)
[ ] Scrollbar thumb size = viewport_extent / content_extent (clamped to min)
[ ] Gesture priority test: programmatic scroll rejected while thumb drag active
[ ] Resize table: each row produces expected offset per anchor/absolute mode
[ ] Nested chaining: inner at boundary → full propagation; inner not at boundary → full consumption
[ ] IME test: scroll permitted, caret geometry unchanged
[ ] No ScrollState fields reference realization, slots, or widget bounds
[ ] LayoutContext is sole delivery mechanism for scroll info to content
[ ] Paint receives only resolved logical_offset, never anchor
[ ] Reconstructed ScrollArea restores retained Y on first tight layout and first paint
[ ] Fresh pre-layout scroll intent cancels a pending retained-Y restore

────────────────────────────────────────────────────────────────────────────────
