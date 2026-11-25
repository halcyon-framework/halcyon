// SPEC-HAL-131A: Scroll Substrate & Input Arbitration
//
// This module defines the canonical scroll state model.
// Scroll is pure math + state - widget-agnostic, virtualization-agnostic.
//
// ============================================================================
// SPEC-HAL-142: State Classification
// ============================================================================
//
// ScrollState is GEOMETRIC STATE - a third category distinct from visual/interaction.
// It is pure mathematics: offsets, extents, anchors for viewport positioning.
//
// Geometric state is consumed by BOTH visual and interaction systems:
// - Visual: paint transforms use logical_offset for scroll positioning
// - Interaction: hit testing uses geometry to determine item under pointer
//
// SPEC-HAL-142 compliance:
// - ScrollState contains no visual state (no hover, no animation)
// - ScrollState contains no interaction state (no selection, no focus)
// - It is a neutral foundation both systems can read
// - Mutations to ScrollState do not conflate visual and interaction concerns
//
// ============================================================================
//
// Key invariants from SPEC-HAL-131A:
// - INV-STATE-1: Exactly one of (anchor, absolute offset) is authoritative
// - INV-STATE-2: logical_offset is always clamped to [0, max(0, content - viewport)]
// - INV-STATE-3: content_extent is reported by content, not computed by ScrollContainer
// - INV-STATE-4: viewport_extent is determined by ScrollContainer's layout constraints
// - INV-ANCHOR-RESOLVE-1: Anchor MUST be resolved to logical_offset before layout
// - INV-ANCHOR-RESOLVE-2: Paint MUST consume resolved logical_offset only
// - INV-ANCHOR-CALL-SITE: Anchor resolution MUST be invoked by ScrollContainer before layout.
//                         It MUST NEVER be triggered from layout or paint code.
//
// Forbidden patterns (FORBID-SCROLL-*):
// - No frame-time dependent scroll math
// - No inertia, momentum, decay, easing, snapping, timers
// - No realization -> scroll mutation
// - No widget querying scroll state via tree traversal
//
// ============================================================================
// VIRTUALIZATION RENDER & MOTION MODEL (BINDING)
// ============================================================================
//
// Virtualized scrolling MUST behave identically to native scrolling:
// - Content moves smoothly
// - Scroll offset is continuous
// - Only *instantiation* is virtualized, not motion
//
// INV-VIRT-SCROLL-1: Scroll Is Real, Not Simulated
//   - logical_offset always changes continuously.
//   - No snapping, no replacement-based illusion.
//   - Render transform uses `-logical_offset` exactly like non-virtualized content.
//
// INV-VIRT-SCROLL-2: Virtualization Affects Existence, Not Position
//   - Widgets outside the visible + overscan range are NOT instantiated.
//   - Widgets inside range are positioned at their TRUE ABSOLUTE content coordinates.
//   - No widget is ever "pinned" to viewport space.
//
// INV-VIRT-SCROLL-3: Anchor Resolution Is Pre-Layout Only
//   - Anchor resolution may adjust logical_offset.
//   - After resolution, scrolling is purely offset-based.
//   - Anchor logic NEVER affects paint-time transforms.
//
// INV-VIRT-SCROLL-4: Stable Coordinate Space
//   - Content space is infinite/large and monotonic.
//   - Item y = index * item_height (or resolver-defined).
//   - Virtualization never alters this mapping.
//
// INV-VIRT-SCROLL-5: Zero Web-Style Recycling
//   - No DOM-style "reuse same node at different offsets".
//   - Widgets may be reused internally, but their logical position MUST change.
//   - Visual continuity is non-negotiable.
//
// FORBIDDEN BEHAVIORS:
// - FORBID-VIRT-1: Replacing widgets without moving content.
// - FORBID-VIRT-2: Jumping content on scroll.
// - FORBID-VIRT-3: Anchors modifying paint transforms.
// - FORBID-VIRT-4: Viewport-relative positioning for list items.
// - FORBID-VIRT-5: Any behavior that would feel different from native OS scrolling.

use crate::core::math::{Point, Size};

// ============================================================================
// SPEC-HAL-133: Linked Scrolling Support
// ============================================================================

/// Scroll group identifier for linked/synchronized scrolling.
///
/// SPEC-HAL-133: Multiple ScrollArea instances may share a ScrollGroupId
/// to synchronize their scroll offsets.
///
/// INV-LINK-MODE-OFFSET-1: Follower offset is clamped to its own scroll bounds.
/// INV-LINK-MODE-OFFSET-2: Offset sync does not assume identical content extents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScrollGroupId(pub u64);

impl ScrollGroupId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Stable identity for anchor targets.
///
/// SPEC-HAL-131A B.2: Opaque identifier for anchored scroll targets.
/// May represent RowId, LineId, NodeId, or any stable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnchorId(pub u64);

impl AnchorId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Scroll anchor for visual stability.
///
/// SPEC-HAL-131A B.2: Anchors survive content mutations that preserve target_id.
/// Anchor resolution converts this to logical_offset before layout.
///
/// INV-ANCHOR-1: Anchor survives content mutations that preserve target_id.
/// INV-ANCHOR-2: If anchor target is removed, fallback to nearest or absolute mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollAnchor {
    /// Stable identity of the anchor target
    pub target_id: AnchorId,
    /// Offset within the target (Point for axis-agnostic resolution)
    pub target_offset: Point,
    /// Target's desired position in viewport (Point for axis-agnostic resolution)
    pub viewport_offset: Point,
}

impl ScrollAnchor {
    pub const fn new(target_id: AnchorId, target_offset: Point, viewport_offset: Point) -> Self {
        Self {
            target_id,
            target_offset,
            viewport_offset,
        }
    }
}

/// Anchor resolver trait for content that supports anchor-based scrolling.
///
/// SPEC-HAL-131A INV-ANCHOR-RESOLVE-1: Anchor resolution runs before layout.
/// Content widgets implement this to resolve anchor targets to positions.
///
/// CALL SITE INVARIANT (INV-ANCHOR-CALL-SITE):
/// - Anchor resolution MUST be invoked by ScrollContainer before layout begins.
/// - Anchor resolution MUST NEVER be triggered from layout or paint code.
/// - This is enforced by design: only ScrollContainer holds the resolver reference.
pub trait AnchorResolver {
    /// Resolve an anchor to a logical position in content space.
    ///
    /// Returns the target's position as a Point (axis-agnostic).
    /// Returns None if the anchor target no longer exists.
    fn resolve_anchor(&self, anchor: &ScrollAnchor) -> Option<Point>;

    /// Find the nearest valid anchor when the current anchor is invalid.
    ///
    /// Returns None if no valid anchor can be found (fall back to absolute mode).
    fn find_nearest_anchor(&self, anchor: &ScrollAnchor) -> Option<ScrollAnchor>;
}

/// Canonical scroll state.
///
/// SPEC-HAL-131A B.1: ScrollContainer is sole owner of scroll position and viewport geometry.
/// No widget, layout, or realization logic may override this.
///
/// This is the single source of truth for scroll position. All scroll consumers
/// (scrollbars, virtualization, paint) read from this state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollState {
    /// Current scroll position (resolved, clamped).
    ///
    /// INV-STATE-2: Always clamped to [0, max(0, content - viewport)] per axis.
    pub logical_offset: Point,

    /// Optional anchor for visual stability.
    ///
    /// INV-STATE-1: If Some, anchor mode is authoritative. logical_offset is derived.
    /// INV-STATE-1: If None, absolute mode - logical_offset is authoritative.
    pub anchor: Option<ScrollAnchor>,

    /// SPEC-HAL-131A Stage 11: Anchor dirty flag for state-driven resolution.
    ///
    /// When true, anchor needs resolution. When false, anchor is clean and
    /// resolution can be skipped.
    ///
    /// INV-ANCHOR-RESOLVE-SCHED-2: Anchors are resolved at most once per dirtied cycle.
    anchor_dirty: bool,

    /// Total scrollable content size.
    ///
    /// INV-STATE-3: Reported by content, not computed by ScrollContainer.
    /// INV-EXTENT-2: May differ from realized widget bounds (virtualization).
    pub content_extent: Size,

    /// Visible viewport size.
    ///
    /// INV-STATE-4: Determined by ScrollContainer's layout constraints.
    pub viewport_extent: Size,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    /// Create a new scroll state at origin with zero extents.
    pub const fn new() -> Self {
        Self {
            logical_offset: Point { x: 0.0, y: 0.0 },
            anchor: None,
            anchor_dirty: false,
            content_extent: Size::zero(),
            viewport_extent: Size::zero(),
        }
    }

    /// Maximum scroll offset per axis.
    ///
    /// INV-STATE-2: max = max(0, content - viewport)
    #[inline]
    pub fn max_scroll(&self) -> Point {
        Point::new(
            (self.content_extent.width - self.viewport_extent.width).max(0.0),
            (self.content_extent.height - self.viewport_extent.height).max(0.0),
        )
    }

    /// Clamp logical_offset to valid range.
    ///
    /// INV-STATE-2: Ensures offset stays in [0, max_scroll] per axis.
    #[inline]
    fn clamp_offset(&mut self) {
        let max = self.max_scroll();
        self.logical_offset.x = self.logical_offset.x.clamp(0.0, max.x);
        self.logical_offset.y = self.logical_offset.y.clamp(0.0, max.y);
    }

    /// Set content extent (total scrollable size).
    ///
    /// INV-STATE-3: Content reports this, ScrollContainer accepts without question.
    /// INV-RESIZE-3: Resize is processed before next layout, not during.
    /// SPEC-HAL-131A Stage 11: Marks anchor dirty if content size changed.
    ///
    /// NOTE: Does NOT clamp offset during layout. Clamping happens in scroll input
    /// handlers to avoid premature clamping during Flex measurement pass.
    pub fn set_content_extent(&mut self, extent: Size) {
        // SPEC-HAL-131A Stage 11: Mark anchor dirty on content size change
        if self.content_extent != extent {
            self.mark_anchor_dirty();
        }
        self.content_extent = extent;
        // Removed: self.clamp_offset() - clamping now happens only in scroll input handlers
    }

    // =========================================================================
    // CONTENT EXTENSION CONTRACT (Model 1: Finite Illusion)
    // =========================================================================
    //
    // For virtualized, unbounded feeds that grow over time (e.g. Twitter-style).
    // Content may extend downward without disturbing viewport or anchors.
    //
    // INV-SCROLL-EXTEND-1: Bottom Extension Only
    //   - Content extent may only grow downward (height += Δ)
    //   - Top (0.0) is immutable
    //
    // INV-SCROLL-EXTEND-2: Viewport Stability
    //   - Extending content MUST NOT change:
    //     * logical_offset
    //     * visible content
    //     * anchor resolution result
    //   - Viewport position is invariant under extension
    //
    // INV-SCROLL-EXTEND-3: No Re-layout Cascade
    //   - Extension MUST NOT:
    //     * trigger anchor resolution
    //     * recompute visible range
    //     * instantiate new items
    //   - Effect is purely on scroll bounds (max_scroll_y)
    //
    // INV-SCROLL-EXTEND-4: Immediate Scrollbar Update
    //   - Scrollbar thumb range reflects new content_extent in the same frame
    //   - No easing, animation, or deferred update
    //
    // INV-SCROLL-EXTEND-5: Jump Safety
    //   - Programmatic jumps after extension:
    //     * use final offset only
    //     * do not realize intermediate items
    // =========================================================================

    /// Extend content height by delta (downward growth only).
    ///
    /// INV-SCROLL-EXTEND-1: delta MUST be >= 0 (bottom extension only).
    /// INV-SCROLL-EXTEND-2: Viewport position unchanged - no offset clamping.
    /// INV-SCROLL-EXTEND-3: No anchor mutation, no visible range recompute.
    /// INV-SCROLL-EXTEND-4: max_scroll_y updated immediately.
    ///
    /// Use case: Virtualized infinite feeds where new content appears at bottom.
    /// After extension, scrollbar reflects new content extent; viewport stays put.
    #[inline]
    pub fn extend_content_height(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-EXTEND-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        // INV-SCROLL-EXTEND-2: Only content extent changes.
        // No offset clamping - viewport position must remain unchanged.
        // INV-SCROLL-EXTEND-3: No anchor mutation here.
        self.content_extent.height += delta.max(0.0);
        // INV-SCROLL-EXTEND-4: max_scroll() now returns updated value.
        // Scrollbar consumers call thumb_fraction()/scroll_fraction() which
        // read content_extent directly - no additional update needed.
    }

    /// Extend content width by delta (rightward growth only).
    ///
    /// INV-SCROLL-EXTEND-1: delta MUST be >= 0 (right extension only).
    /// INV-SCROLL-EXTEND-2: Viewport position unchanged - no offset clamping.
    #[inline]
    pub fn extend_content_width(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-EXTEND-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        self.content_extent.width += delta.max(0.0);
    }

    // =========================================================================
    // CONTENT PREPEND CONTRACT (Model 2: Top Extension / History Prepend)
    // =========================================================================
    //
    // For virtualized feeds where older content is loaded above the viewport
    // (e.g., chat history loading, infinite scroll upward).
    // Content grows upward; offset is compensated to maintain visual stability.
    //
    // INV-SCROLL-PREPEND-1: Top Extension Only
    //   - Delta ≥ 0
    //   - Content grows upward conceptually (prepend)
    //
    // INV-SCROLL-PREPEND-2: Viewport Stability
    //   - Visual content must not move
    //   - Logical offset MUST increase by delta
    //   - Items previously at offset N are now at offset N + delta
    //
    // INV-SCROLL-PREPEND-3: Anchor Safety
    //   - No anchor creation, resolution, clearing, or mutation
    //   - Existing anchor (if any) is preserved untouched
    //
    // INV-SCROLL-PREPEND-4: Immediate Geometry Update
    //   - max_scroll updates same frame
    //   - Scrollbar updates immediately
    //
    // INV-SCROLL-PREPEND-5: Jump Safety
    //   - Subsequent programmatic jumps use final offset only
    //   - No intermediate states
    // =========================================================================

    /// Prepend content height by delta (upward growth with offset compensation).
    ///
    /// INV-SCROLL-PREPEND-1: delta MUST be >= 0 (top extension only).
    /// INV-SCROLL-PREPEND-2: Viewport stability - offset increased by delta.
    /// INV-SCROLL-PREPEND-3: No anchor mutation.
    /// INV-SCROLL-PREPEND-4: max_scroll updated immediately.
    ///
    /// Use case: Chat history loading, infinite scroll upward.
    /// After prepend, viewport shows same content; old items shifted down logically.
    #[inline]
    pub fn prepend_content_height(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-PREPEND-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        let delta = delta.max(0.0);

        // INV-SCROLL-PREPEND-2: Offset compensation for viewport stability.
        // Items at old offset Y are now at Y + delta.
        // To keep viewing the same items, offset must also increase by delta.
        self.logical_offset.y += delta;

        // Content extent grows.
        self.content_extent.height += delta;

        // INV-SCROLL-PREPEND-3: Anchor is not touched.
        // INV-SCROLL-PREPEND-4: max_scroll() now returns updated value.
    }

    /// Prepend content width by delta (leftward growth with offset compensation).
    ///
    /// INV-SCROLL-PREPEND-1: delta MUST be >= 0 (left extension only).
    /// INV-SCROLL-PREPEND-2: Viewport stability - offset increased by delta.
    #[inline]
    pub fn prepend_content_width(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-PREPEND-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        let delta = delta.max(0.0);
        self.logical_offset.x += delta;
        self.content_extent.width += delta;
    }

    // =========================================================================
    // CONTENT TRIM CONTRACT (Model 3: Bidirectional Removal)
    // =========================================================================
    //
    // For memory-bounded virtualized feeds that discard off-screen content.
    // Required for:
    //   - Memory-bounded infinite feeds
    //   - Chat history truncation
    //   - Long-running sessions
    //
    // INV-SCROLL-TRIM-1: Directional Legality
    //   - Delta >= 0
    //   - Top trim removes content above current content origin
    //   - Bottom trim removes content below current content end
    //
    // INV-SCROLL-TRIM-2: Offset Compensation
    //   - Top trim: logical_offset.y = max(0, logical_offset.y - delta)
    //   - Bottom trim: logical_offset.y unchanged unless clamped by new max
    //
    // INV-SCROLL-TRIM-3: Viewport Stability
    //   - If trimmed region is fully off-screen: visible content must not move
    //   - If trim intersects viewport: clamp offset only (no animation, no easing)
    //
    // INV-SCROLL-TRIM-4: Anchor Safety
    //   - No anchor resolution
    //   - No anchor mutation
    //   - Anchor may be invalidated ONLY if its target is removed (caller responsibility)
    //
    // INV-SCROLL-TRIM-5: Geometry Correctness
    //   - content_extent shrinks immediately
    //   - max_scroll() updates same frame
    //   - Scrollbar reflects new geometry immediately
    //
    // INV-SCROLL-TRIM-6: No Hidden Side Effects
    //   - No visible-range recompute
    //   - No instantiation
    //   - No resolver calls
    // =========================================================================

    /// Trim content from the top by delta (remove content above origin).
    ///
    /// INV-SCROLL-TRIM-1: delta MUST be >= 0.
    /// INV-SCROLL-TRIM-2: Offset decreased by delta (clamped to 0).
    /// INV-SCROLL-TRIM-3: If trim is off-screen, viewport stable.
    /// INV-SCROLL-TRIM-4: Anchor not mutated (caller handles invalidation).
    /// INV-SCROLL-TRIM-5: content_extent and max_scroll updated immediately.
    ///
    /// Use case: Discarding old chat history above viewport.
    /// Items that were at offset Y are now at offset Y - delta.
    /// Viewport offset is compensated to maintain visual stability.
    #[inline]
    pub fn trim_content_top(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-TRIM-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        let delta = delta.max(0.0);

        // Don't trim more than we have
        let delta = delta.min(self.content_extent.height);

        // INV-SCROLL-TRIM-2: Offset compensation.
        // Content above is removed, so items shift up logically.
        // To maintain visual stability, offset decreases by delta.
        self.logical_offset.y = (self.logical_offset.y - delta).max(0.0);

        // INV-SCROLL-TRIM-5: Content extent shrinks.
        self.content_extent.height -= delta;

        // INV-SCROLL-TRIM-4: Anchor is not touched (caller responsibility).
        // INV-SCROLL-TRIM-6: No side effects.
    }

    /// Trim content from the bottom by delta (remove content below end).
    ///
    /// INV-SCROLL-TRIM-1: delta MUST be >= 0.
    /// INV-SCROLL-TRIM-2: Offset unchanged unless clamped by new max.
    /// INV-SCROLL-TRIM-3: If trim is off-screen, viewport stable.
    /// INV-SCROLL-TRIM-4: Anchor not mutated (caller handles invalidation).
    /// INV-SCROLL-TRIM-5: content_extent and max_scroll updated immediately.
    ///
    /// Use case: Discarding future content below viewport.
    /// Only clamps offset if viewport was viewing the trimmed region.
    #[inline]
    pub fn trim_content_bottom(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-TRIM-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        let delta = delta.max(0.0);

        // Don't trim more than we have
        let delta = delta.min(self.content_extent.height);

        // INV-SCROLL-TRIM-5: Content extent shrinks.
        self.content_extent.height -= delta;

        // INV-SCROLL-TRIM-2: Offset unchanged, but clamp to new max.
        // INV-SCROLL-TRIM-3: If trim intersects viewport, clamp only.
        let max = self.max_scroll();
        if self.logical_offset.y > max.y {
            self.logical_offset.y = max.y;
        }

        // INV-SCROLL-TRIM-4: Anchor is not touched (caller responsibility).
        // INV-SCROLL-TRIM-6: No side effects.
    }

    /// Trim content from the left by delta (remove content left of origin).
    ///
    /// INV-SCROLL-TRIM-1: delta MUST be >= 0.
    /// INV-SCROLL-TRIM-2: Offset decreased by delta (clamped to 0).
    #[inline]
    pub fn trim_content_left(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-TRIM-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        let delta = delta.max(0.0).min(self.content_extent.width);
        self.logical_offset.x = (self.logical_offset.x - delta).max(0.0);
        self.content_extent.width -= delta;
    }

    /// Trim content from the right by delta (remove content right of end).
    ///
    /// INV-SCROLL-TRIM-1: delta MUST be >= 0.
    /// INV-SCROLL-TRIM-2: Offset unchanged unless clamped by new max.
    #[inline]
    pub fn trim_content_right(&mut self, delta: f32) {
        debug_assert!(
            delta >= 0.0,
            "INV-SCROLL-TRIM-1 VIOLATION: delta must be >= 0, got {}",
            delta
        );
        let delta = delta.max(0.0).min(self.content_extent.width);
        self.content_extent.width -= delta;
        let max = self.max_scroll();
        if self.logical_offset.x > max.x {
            self.logical_offset.x = max.x;
        }
    }

    // =========================================================================
    // ANCHOR INVALIDATION CONTRACT (Model 4: Explicit Invalidation)
    // =========================================================================
    //
    // After trim operations, anchors may silently point to removed content.
    // This contract provides an explicit, mechanical anchor invalidation rule.
    //
    // INV-ANCHOR-INV-1: Explicit Invalidation Only
    //   - Anchor is removed ONLY via this API
    //   - No trim/extend/prepend code may touch anchors directly
    //   - All other content mutation APIs preserve anchor
    //
    // INV-ANCHOR-INV-2: Predicate-Based
    //   - Caller provides a predicate closure
    //   - Predicate returns true  → anchor cleared
    //   - Predicate returns false → anchor preserved
    //
    // INV-ANCHOR-INV-3: No Side Effects
    //   - No offset mutation
    //   - No resolver calls
    //   - No layout/visible-range recompute
    //   - Pure anchor state mutation only
    //
    // INV-ANCHOR-INV-4: Idempotent
    //   - Calling on None anchor is a no-op
    //   - Calling twice is safe
    // =========================================================================

    /// Invalidate anchor if predicate returns true.
    ///
    /// INV-ANCHOR-INV-1: This is the ONLY API that removes anchors due to content mutation.
    /// INV-ANCHOR-INV-2: Predicate-based - true clears, false preserves.
    /// INV-ANCHOR-INV-3: No side effects - offset unchanged, no resolver calls.
    /// INV-ANCHOR-INV-4: Idempotent - safe on None, safe to call multiple times.
    ///
    /// Use case: After trim_content_top, invalidate if anchor target was removed.
    /// Example: `state.invalidate_anchor_if(|a| a.target_id.0 < trimmed_item_count)`
    #[inline]
    pub fn invalidate_anchor_if<F>(&mut self, predicate: F)
    where
        F: FnOnce(&ScrollAnchor) -> bool,
    {
        // INV-ANCHOR-INV-4: No anchor = no-op
        if let Some(anchor) = &self.anchor {
            // INV-ANCHOR-INV-2: Predicate decides
            if predicate(anchor) {
                // INV-ANCHOR-INV-3: Only anchor is mutated, nothing else
                self.anchor = None;
            }
        }
        // INV-ANCHOR-INV-3: No offset mutation, no resolver calls
    }

    // =========================================================================
    // ANCHOR RESOLUTION CONTRACT (Stage 5: Read-Only Path)
    // =========================================================================
    //
    // Pure read-only anchor resolution without state mutation.
    // Returns the resolved content-space position for display/query purposes.
    //
    // INV-ANCHOR-RESOLVE-RO-1: Pure Function
    //   - No offset mutation
    //   - No anchor mutation
    //   - No geometry mutation
    //   - No caching of resolved position
    //
    // INV-ANCHOR-RESOLVE-RO-2: Single Resolver Call
    //   - Resolver called exactly once when anchor exists
    //   - Resolver not called when anchor is None
    //
    // INV-ANCHOR-RESOLVE-RO-3: No Auto-Scroll
    //   - Does not adjust viewport
    //   - Does not clamp result
    //   - Returns raw content-space position
    //
    // INV-ANCHOR-RESOLVE-RO-4: No Invalidation
    //   - Does not invalidate anchor on resolution failure
    //   - Returns None if resolver returns None
    // =========================================================================

    /// Resolve anchor to content-space position (read-only, no mutation).
    ///
    /// INV-ANCHOR-RESOLVE-RO-1: Pure function - no state mutation.
    /// INV-ANCHOR-RESOLVE-RO-2: Resolver called exactly once when anchor exists.
    /// INV-ANCHOR-RESOLVE-RO-3: No auto-scroll, no clamping.
    /// INV-ANCHOR-RESOLVE-RO-4: No anchor invalidation on failure.
    ///
    /// Returns the resolved content-space position of the anchor target,
    /// or None if no anchor is set or resolution fails.
    ///
    /// Use case: Query where an anchor would resolve to without modifying state.
    #[inline]
    pub fn resolve_anchor_position(&self, resolver: &dyn AnchorResolver) -> Option<Point> {
        // INV-ANCHOR-RESOLVE-RO-2: No anchor = no resolver call
        let anchor = self.anchor.as_ref()?;

        // INV-ANCHOR-RESOLVE-RO-2: Call resolver exactly once
        // INV-ANCHOR-RESOLVE-RO-1: No mutation - just return the result
        // INV-ANCHOR-RESOLVE-RO-4: If None, return None (no invalidation)
        resolver.resolve_anchor(anchor)
    }

    // =========================================================================
    // ANCHOR-BASED SCROLL JUMP CONTRACT (Stage 6: Explicit, Single-Shot)
    // =========================================================================
    //
    // One-shot, explicit jump API that scrolls to an anchor using the
    // already-resolved position. First mutating consumer of anchor system.
    //
    // INV-ANCHOR-JUMP-1: Explicit Call Only
    //   - No automatic jumps anywhere else
    //   - Jump only occurs when this method is called
    //
    // INV-ANCHOR-JUMP-2: Single Resolver Call
    //   - Must reuse read-only resolution logic (resolve_anchor_position)
    //   - Resolver called exactly once
    //
    // INV-ANCHOR-JUMP-3: Single Mutation
    //   - Offset written exactly once
    //   - No intermediate states
    //
    // INV-ANCHOR-JUMP-4: Anchor Preserved
    //   - Jump does NOT clear or mutate anchor
    //   - Anchor remains available for subsequent operations
    //
    // INV-ANCHOR-JUMP-5: No Side Effects
    //   - No trimming, no extending, no invalidation
    //   - Only offset mutation
    // =========================================================================

    /// Jump to anchor position (explicit, single-shot).
    ///
    /// INV-ANCHOR-JUMP-1: This is the ONLY place where anchor-based jump occurs.
    /// INV-ANCHOR-JUMP-2: Resolver called exactly once via resolve_anchor_position.
    /// INV-ANCHOR-JUMP-3: Offset written exactly once.
    /// INV-ANCHOR-JUMP-4: Anchor preserved after jump.
    /// INV-ANCHOR-JUMP-5: No side effects - only offset mutation.
    ///
    /// Returns true if jump occurred, false if no anchor or resolution failed.
    ///
    /// Computation:
    /// - target_offset = resolved_point - anchor.viewport_offset
    /// - Clamped to [0, max_scroll]
    #[inline]
    pub fn jump_to_anchor(&mut self, resolver: &dyn AnchorResolver) -> bool {
        // INV-ANCHOR-JUMP-1: No anchor = no jump
        let anchor = match &self.anchor {
            Some(a) => a,
            None => return false,
        };

        // INV-ANCHOR-JUMP-2: Single resolver call via read-only path
        let resolved = match resolver.resolve_anchor(anchor) {
            Some(p) => p,
            None => return false,
        };

        // Capture viewport_offset before any mutation
        let viewport_offset = anchor.viewport_offset;

        // Compute target offset: resolved_point - viewport_offset
        let target_x = resolved.x - viewport_offset.x;
        let target_y = resolved.y - viewport_offset.y;

        // INV-ANCHOR-JUMP-3: Single mutation - clamp to [0, max_scroll]
        let max = self.max_scroll();
        self.logical_offset.x = target_x.clamp(0.0, max.x);
        self.logical_offset.y = target_y.clamp(0.0, max.y);

        // INV-ANCHOR-JUMP-4: Anchor NOT cleared
        // INV-ANCHOR-JUMP-5: No side effects - we only touched logical_offset

        true
    }

    // =========================================================================
    // ANCHOR RESOLUTION INTEGRATION (Stage 7: Write Path, Explicit Commit)
    // =========================================================================
    //
    // Committed anchor resolution path that:
    // - resolves an anchor
    // - applies the jump
    // - optionally clears the anchor explicitly when the caller decides
    //
    // This separates:
    // - jumping (Stage 6)
    // - from lifecycle ownership of the anchor
    //
    // INV-ANCHOR-COMMIT-1: Composition Only
    //   - Must reuse jump_to_anchor, no duplicated logic.
    //
    // INV-ANCHOR-COMMIT-2: Explicit Clearing
    //   - Anchor cleared ONLY if clear_after == true.
    //
    // INV-ANCHOR-COMMIT-3: No Hidden Invalidation
    //   - Resolver failure does not clear anchor.
    //
    // INV-ANCHOR-COMMIT-4: Single Resolver Call
    //   - Resolver still called exactly once (via jump).
    // =========================================================================

    /// Resolve anchor and jump, optionally clearing anchor after.
    ///
    /// INV-ANCHOR-COMMIT-1: Reuses jump_to_anchor (no duplicated logic).
    /// INV-ANCHOR-COMMIT-2: Anchor cleared only if clear_after == true.
    /// INV-ANCHOR-COMMIT-3: Resolver failure does not clear anchor.
    /// INV-ANCHOR-COMMIT-4: Resolver called exactly once (via jump).
    ///
    /// Returns true if jump occurred, false if no anchor or resolution failed.
    ///
    /// This completes the anchor lifecycle loop: create → resolve → jump → optionally retire.
    #[inline]
    pub fn resolve_and_jump_anchor(
        &mut self,
        resolver: &dyn AnchorResolver,
        clear_after: bool,
    ) -> bool {
        // INV-ANCHOR-COMMIT-1: No anchor = no jump (handled by jump_to_anchor)
        if self.anchor.is_none() {
            return false;
        }

        // INV-ANCHOR-COMMIT-1: Reuse jump_to_anchor
        // INV-ANCHOR-COMMIT-4: Resolver called exactly once via this call
        let jumped = self.jump_to_anchor(resolver);

        // INV-ANCHOR-COMMIT-3: If jump failed (resolver returned None), do not clear
        if !jumped {
            return false;
        }

        // INV-ANCHOR-COMMIT-2: Clear only if requested
        if clear_after {
            self.anchor = None;
        }

        true
    }

    /// Set viewport extent (visible region size).
    ///
    /// INV-STATE-4: Determined by ScrollContainer's layout constraints.
    ///
    /// NOTE: Does NOT clamp offset. Clamping happens in scroll input handlers
    /// (scroll_by, set_offset) to avoid premature clamping during Flex measurement pass.
    /// The measurement pass may temporarily set a larger viewport, and we must not
    /// clamp offset based on that temporary value.
    pub fn set_viewport_extent(&mut self, extent: Size) {
        self.viewport_extent = extent;
        // Removed: self.clamp_offset() - clamping now happens only in scroll input handlers
    }

    /// Clamp logical_offset to current valid range without clearing anchor.
    ///
    /// Stage 2.5: Scroll Metrics Revalidation on Resize.
    ///
    /// This method is specifically for resize scenarios where:
    /// - Viewport or content size has changed
    /// - max_scroll may have decreased
    /// - logical_offset may now exceed the valid range
    /// - Anchor should be preserved (not cleared)
    ///
    /// INV-STATE-2: Ensures offset stays in [0, max_scroll] per axis.
    ///
    /// Call this after setting new viewport/content extents when the scroll
    /// position must be validated immediately (not deferred to next input).
    #[inline]
    pub fn clamp_offset_on_resize(&mut self) {
        self.clamp_offset();
    }

    /// Set logical offset directly (absolute mode).
    ///
    /// INV-STATE-1: Clears anchor - offset becomes authoritative.
    /// INV-STATE-2: Value is clamped to valid range.
    /// INV-SCROLL-CORE-6: Programmatic scroll mutates offset only.
    pub fn set_offset(&mut self, offset: Point) {
        self.anchor = None;
        self.logical_offset = offset;
        self.clamp_offset();
    }

    /// Set X offset only (absolute mode).
    pub fn set_offset_x(&mut self, x: f32) {
        self.anchor = None;
        let max = self.max_scroll();
        self.logical_offset.x = x.clamp(0.0, max.x);
    }

    /// Set Y offset only (absolute mode).
    pub fn set_offset_y(&mut self, y: f32) {
        self.anchor = None;
        let max = self.max_scroll();
        self.logical_offset.y = y.clamp(0.0, max.y);
    }

    /// Scroll by delta (absolute mode).
    ///
    /// INV-STATE-1: Clears anchor - offset becomes authoritative.
    /// INV-STATE-2: Result is clamped to valid range.
    pub fn scroll_by(&mut self, delta: Point) {
        self.anchor = None;
        self.logical_offset.x += delta.x;
        self.logical_offset.y += delta.y;
        self.clamp_offset();
    }

    /// Scroll by delta on X axis only.
    pub fn scroll_by_x(&mut self, dx: f32) {
        self.anchor = None;
        self.logical_offset.x += dx;
        let max = self.max_scroll();
        self.logical_offset.x = self.logical_offset.x.clamp(0.0, max.x);
    }

    /// Scroll by delta on Y axis only.
    pub fn scroll_by_y(&mut self, dy: f32) {
        self.anchor = None;
        self.logical_offset.y += dy;
        let max = self.max_scroll();
        self.logical_offset.y = self.logical_offset.y.clamp(0.0, max.y);
    }

    // =========================================================================
    // SPEC-HAL-133: Linked Scrolling Offset Receiver
    // =========================================================================
    //
    // Receives offset from linked scrolling without clearing anchor.
    // Used by ScrollLinkCoordinator to propagate offsets between group members.
    //
    // INV-LINK-MODE-OFFSET-1: Offset clamped to this surface's scroll bounds.
    // INV-LINK-MODE-ANCHOR-1: Anchors remain per-surface (not copied).
    // =========================================================================

    /// Receive linked scroll offset (does not clear anchor).
    ///
    /// SPEC-HAL-133: Used for linked scrolling propagation.
    /// Unlike set_offset(), this does NOT clear the anchor.
    ///
    /// INV-LINK-MODE-OFFSET-1: Offset clamped to this surface's bounds.
    /// INV-LINK-MODE-ANCHOR-1: Anchor remains unchanged.
    ///
    /// Returns true if offset changed.
    #[inline]
    pub fn receive_linked_offset(&mut self, offset: Point) -> bool {
        let max = self.max_scroll();
        let new_x = offset.x.clamp(0.0, max.x);
        let new_y = offset.y.clamp(0.0, max.y);

        let changed = (self.logical_offset.x - new_x).abs() > 0.001
            || (self.logical_offset.y - new_y).abs() > 0.001;

        if changed {
            self.logical_offset.x = new_x;
            self.logical_offset.y = new_y;
        }

        changed
    }

    /// Receive linked scroll offset for Y axis only.
    ///
    /// SPEC-HAL-133: Used for vertical linked scrolling.
    /// Returns true if offset changed.
    #[inline]
    pub fn receive_linked_offset_y(&mut self, y: f32) -> bool {
        let max = self.max_scroll();
        let new_y = y.clamp(0.0, max.y);
        let changed = (self.logical_offset.y - new_y).abs() > 0.001;
        if changed {
            self.logical_offset.y = new_y;
        }
        changed
    }

    /// Receive linked scroll offset for X axis only.
    ///
    /// SPEC-HAL-133: Used for horizontal linked scrolling.
    /// Returns true if offset changed.
    #[inline]
    pub fn receive_linked_offset_x(&mut self, x: f32) -> bool {
        let max = self.max_scroll();
        let new_x = x.clamp(0.0, max.x);
        let changed = (self.logical_offset.x - new_x).abs() > 0.001;
        if changed {
            self.logical_offset.x = new_x;
        }
        changed
    }

    /// Set anchor (anchor mode).
    ///
    /// INV-STATE-1: Anchor becomes authoritative until next absolute operation.
    /// INV-ANCHOR-RESOLVE-1: Must call resolve_anchor() before layout.
    ///
    /// SPEC-HAL-131A Stage 9: Internal API. Use ScrollArea::set_anchor_at_viewport_y
    /// for intent-named anchor creation.
    ///
    /// SPEC-HAL-131A Stage 11: Sets anchor_dirty = true to trigger resolution.
    #[doc(hidden)]
    pub fn set_anchor(&mut self, anchor: ScrollAnchor) {
        self.anchor = Some(anchor);
        self.anchor_dirty = true;
    }

    /// Clear anchor, keeping current logical_offset.
    ///
    /// INV-STATE-1: Switches to absolute mode.
    /// SPEC-HAL-131A Stage 11: Clears both anchor and anchor_dirty.
    pub fn clear_anchor(&mut self) {
        self.anchor = None;
        self.anchor_dirty = false;
    }

    // =========================================================================
    // SPEC-HAL-131A Stage 11: Anchor Dirtying & Re-arm Semantics
    // =========================================================================
    //
    // State-driven anchor resolution. Anchors persist across frames without
    // being re-resolved unless explicitly dirtied.
    //
    // INV-ANCHOR-RESOLVE-SCHED-2: Anchors are resolved at most once per dirtied cycle.
    //   - Resolution is skipped if anchor is clean.
    //   - After successful resolution, anchor_dirty = false.
    // =========================================================================

    /// Check if anchor needs resolution.
    ///
    /// SPEC-HAL-131A Stage 11: Returns true if anchor exists AND is dirty.
    #[inline]
    pub fn is_anchor_dirty(&self) -> bool {
        self.anchor.is_some() && self.anchor_dirty
    }

    /// Mark anchor as dirty, triggering resolution on next cycle.
    ///
    /// SPEC-HAL-131A Stage 11: Public invalidation hook.
    /// No-op if no anchor is set.
    ///
    /// Use case: Content mutations (insert, delete, resize) that require
    /// anchor re-resolution.
    #[inline]
    pub fn mark_anchor_dirty(&mut self) {
        if self.anchor.is_some() {
            self.anchor_dirty = true;
        }
    }

    /// Clear anchor dirty flag after successful resolution.
    ///
    /// SPEC-HAL-131A Stage 11: Internal API for resolution scheduling.
    /// Called after anchor is successfully resolved.
    #[inline]
    pub(crate) fn clear_anchor_dirty(&mut self) {
        self.anchor_dirty = false;
    }

    /// Resolve anchor to logical_offset before layout.
    ///
    /// SPEC-HAL-131A INV-ANCHOR-RESOLVE-1: This MUST be called before layout begins.
    /// SPEC-HAL-131A INV-ANCHOR-RESOLVE-2: After this, paint uses logical_offset only.
    ///
    /// INV-ANCHOR-CALL-SITE: This method MUST be called ONLY by ScrollContainer,
    /// BEFORE layout begins. It MUST NEVER be invoked from layout or paint code.
    ///
    /// If anchor resolution fails, falls back to absolute mode with current offset.
    ///
    /// # SPEC-HAL-133 Phase 2: Origin-Agnostic Resolution Guarantee
    ///
    /// This function is origin-agnostic by construction. The resolver receives only
    /// `ScrollAnchor` (target_id, target_offset, viewport_offset) and returns geometry.
    /// There is no field, flag, or branch for anchor origin (user vs linked).
    /// Linked anchors are indistinguishable from user anchors at resolution time.
    pub fn resolve_anchor(&mut self, resolver: &dyn AnchorResolver) {
        // SPEC-HAL-133 Phase 2 INV: Anchor resolution is origin-agnostic.
        // ScrollAnchor contains only: target_id, target_offset, viewport_offset.
        // There is NO origin metadata (is_linked, from_link, source, token, etc.).
        // Resolution depends solely on anchor geometry + resolver layout data.
        debug_assert!(
            std::mem::size_of::<ScrollAnchor>()
                == std::mem::size_of::<AnchorId>() + 2 * std::mem::size_of::<Point>(),
            "SPEC-HAL-133: ScrollAnchor must contain only target_id + two Points (no origin metadata)"
        );

        let Some(anchor) = self.anchor else { return };

        if let Some(target_pos) = resolver.resolve_anchor(&anchor) {
            // Convert anchor position to scroll offset (axis-agnostic)
            // target_pos is the content-space position of the anchor
            // viewport_offset is where we want it in the viewport
            self.logical_offset.x = target_pos.x - anchor.viewport_offset.x;
            self.logical_offset.y = target_pos.y - anchor.viewport_offset.y;
            self.clamp_offset();
        } else {
            // Anchor target not found - try nearest
            if let Some(new_anchor) = resolver.find_nearest_anchor(&anchor) {
                if let Some(target_pos) = resolver.resolve_anchor(&new_anchor) {
                    self.logical_offset.x = target_pos.x - new_anchor.viewport_offset.x;
                    self.logical_offset.y = target_pos.y - new_anchor.viewport_offset.y;
                    self.anchor = Some(new_anchor);
                    self.clamp_offset();
                    return;
                }
            }
            // INV-ANCHOR-2: Fall back to absolute mode
            self.anchor = None;
            self.clamp_offset();
        }
    }

    /// Check if at start boundary (offset = 0).
    ///
    /// INV-CHAIN-2: Boundary for nested scroll chaining.
    #[inline]
    pub fn at_start(&self) -> (bool, bool) {
        (self.logical_offset.x <= 0.0, self.logical_offset.y <= 0.0)
    }

    /// Check if at end boundary (offset = max).
    ///
    /// INV-CHAIN-2: Boundary for nested scroll chaining.
    #[inline]
    pub fn at_end(&self) -> (bool, bool) {
        let max = self.max_scroll();
        (
            self.logical_offset.x >= max.x,
            self.logical_offset.y >= max.y,
        )
    }

    /// Check if scrolling is needed (content exceeds viewport).
    #[inline]
    pub fn can_scroll(&self) -> (bool, bool) {
        (
            self.content_extent.width > self.viewport_extent.width,
            self.content_extent.height > self.viewport_extent.height,
        )
    }

    /// Get scroll fraction for scrollbar positioning.
    ///
    /// INV-THUMB-1: Thumb position is pure function of logical_offset, content_extent, viewport_extent.
    #[inline]
    pub fn scroll_fraction(&self) -> (f32, f32) {
        let max = self.max_scroll();
        let fx = if max.x > 0.0 {
            self.logical_offset.x / max.x
        } else {
            0.0
        };
        let fy = if max.y > 0.0 {
            self.logical_offset.y / max.y
        } else {
            0.0
        };
        (fx, fy)
    }

    /// Get thumb size fraction for scrollbar sizing.
    ///
    /// INV-THUMB-2: Thumb size = viewport_extent / content_extent (clamped to minimum).
    #[inline]
    pub fn thumb_fraction(&self) -> (f32, f32) {
        let fx = if self.content_extent.width > 0.0 {
            (self.viewport_extent.width / self.content_extent.width).min(1.0)
        } else {
            1.0
        };
        let fy = if self.content_extent.height > 0.0 {
            (self.viewport_extent.height / self.content_extent.height).min(1.0)
        } else {
            1.0
        };
        (fx, fy)
    }
}

/// Viewport information passed to scroll content via LayoutContext.
///
/// SPEC-HAL-131A INV-SCROLL-BOUNDARY-STRICT:
/// This is the ONLY mechanism by which scroll info reaches content.
/// No widget may query parent nodes or global state to infer scroll position.
///
/// FORBID-SCROLL-QUERY: Any widget querying scroll via tree traversal is forbidden.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollViewportInfo {
    /// Viewport width available to content
    pub viewport_width: f32,
    /// Viewport height available to content
    pub viewport_height: f32,
    /// Current horizontal scroll offset
    pub scroll_offset_x: f32,
    /// Current vertical scroll offset
    pub scroll_offset_y: f32,
}

impl ScrollViewportInfo {
    pub const fn new(
        viewport_width: f32,
        viewport_height: f32,
        scroll_offset_x: f32,
        scroll_offset_y: f32,
    ) -> Self {
        Self {
            viewport_width,
            viewport_height,
            scroll_offset_x,
            scroll_offset_y,
        }
    }

    /// Create from ScrollState
    pub fn from_scroll_state(state: &ScrollState) -> Self {
        Self {
            viewport_width: state.viewport_extent.width,
            viewport_height: state.viewport_extent.height,
            scroll_offset_x: state.logical_offset.x,
            scroll_offset_y: state.logical_offset.y,
        }
    }
}

/// Input gesture classification.
///
/// SPEC-HAL-131A B.3: Gesture metadata for input arbitration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureSource {
    /// Mouse wheel steps
    Wheel,
    /// Trackpad two-finger scroll
    Trackpad,
    /// Scrollbar thumb drag
    Thumb,
    /// Keyboard (Page Up/Down, arrows, Home/End)
    Keyboard,
    /// Programmatic (scroll_to, scroll_by, reveal)
    Programmatic,
}

/// Gesture lifetime.
///
/// SPEC-HAL-131A B.3: Discrete vs continuous gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureLifetime {
    /// Single intent per event (wheel, keyboard)
    Discrete,
    /// start -> update* -> end (drag)
    Continuous,
}

/// Input gesture tracking for arbitration.
///
/// SPEC-HAL-131A G.1: At most one active gesture per axis at any time.
/// Higher priority gesture preempts lower priority on same axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputGesture {
    pub source: GestureSource,
    pub lifetime: GestureLifetime,
}

impl InputGesture {
    /// Get priority (higher number = higher priority).
    ///
    /// SPEC-HAL-131A G.1 Priority Table:
    /// 1. Thumb drag (priority 5)
    /// 2. Touch/Trackpad (priority 4)
    /// 3. Wheel (priority 3)
    /// 4. Keyboard (priority 2)
    /// 5. Programmatic (priority 1)
    pub fn priority(&self) -> u8 {
        match self.source {
            GestureSource::Thumb => 5,
            GestureSource::Trackpad => 4,
            GestureSource::Wheel => 3,
            GestureSource::Keyboard => 2,
            GestureSource::Programmatic => 1,
        }
    }
}

/// Active gesture state for input arbitration.
///
/// SPEC-HAL-131A INV-GESTURE-1: At most one active gesture per axis.
/// SPEC-HAL-131A INV-INPUT-PRIORITY-STRICT: Programmatic scroll MUST NOT preempt user gesture.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GestureState {
    /// Active gesture on horizontal axis
    pub horizontal: Option<InputGesture>,
    /// Active gesture on vertical axis
    pub vertical: Option<InputGesture>,
}

impl GestureState {
    pub const fn new() -> Self {
        Self {
            horizontal: None,
            vertical: None,
        }
    }

    /// Try to start a gesture. Returns per-axis acceptance.
    ///
    /// SPEC-HAL-131A INV-GESTURE-2: Higher priority preempts lower priority.
    ///
    /// Returns (accepted_horizontal, accepted_vertical) for explicit per-axis semantics.
    /// No silent partial acceptance: caller knows exactly which axes were accepted.
    pub fn try_start(
        &mut self,
        gesture: InputGesture,
        horizontal: bool,
        vertical: bool,
    ) -> (bool, bool) {
        let mut accepted_h = false;
        let mut accepted_v = false;

        if horizontal {
            if let Some(active) = &self.horizontal {
                if gesture.priority() > active.priority() {
                    self.horizontal = Some(gesture);
                    accepted_h = true;
                }
            } else {
                self.horizontal = Some(gesture);
                accepted_h = true;
            }
        }

        if vertical {
            if let Some(active) = &self.vertical {
                if gesture.priority() > active.priority() {
                    self.vertical = Some(gesture);
                    accepted_v = true;
                }
            } else {
                self.vertical = Some(gesture);
                accepted_v = true;
            }
        }

        (accepted_h, accepted_v)
    }

    /// End a gesture of the given source.
    pub fn end(&mut self, source: GestureSource) {
        if matches!(self.horizontal, Some(g) if g.source == source) {
            self.horizontal = None;
        }
        if matches!(self.vertical, Some(g) if g.source == source) {
            self.vertical = None;
        }
    }

    /// Check if programmatic scroll is allowed per axis.
    ///
    /// SPEC-HAL-131A INV-INPUT-PRIORITY-STRICT:
    /// Programmatic requests MUST be deferred or rejected if any non-programmatic
    /// gesture is active on that axis.
    ///
    /// Returns (horizontal_allowed, vertical_allowed).
    pub fn allows_programmatic(&self) -> (bool, bool) {
        // Programmatic is allowed if no gesture is active, or if the active gesture is also Programmatic
        let h = self
            .horizontal
            .map_or(true, |g| g.source == GestureSource::Programmatic);
        let v = self
            .vertical
            .map_or(true, |g| g.source == GestureSource::Programmatic);
        (h, v)
    }

    /// Check if any user gesture is active.
    pub fn has_active_gesture(&self) -> bool {
        self.horizontal.is_some() || self.vertical.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_new() {
        let state = ScrollState::new();
        assert_eq!(state.logical_offset, Point::new(0.0, 0.0));
        assert!(state.anchor.is_none());
        assert_eq!(state.content_extent, Size::zero());
        assert_eq!(state.viewport_extent, Size::zero());
    }

    #[test]
    fn test_scroll_state_clamping() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(500.0, 1000.0));
        state.set_viewport_extent(Size::new(200.0, 300.0));

        // max = (300, 700)
        assert_eq!(state.max_scroll(), Point::new(300.0, 700.0));

        // Set within bounds
        state.set_offset(Point::new(100.0, 200.0));
        assert_eq!(state.logical_offset, Point::new(100.0, 200.0));

        // Set beyond max - should clamp
        state.set_offset(Point::new(1000.0, 2000.0));
        assert_eq!(state.logical_offset, Point::new(300.0, 700.0));

        // Set below min - should clamp
        state.set_offset(Point::new(-100.0, -200.0));
        assert_eq!(state.logical_offset, Point::new(0.0, 0.0));
    }

    #[test]
    fn test_scroll_by() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(500.0, 1000.0));
        state.set_viewport_extent(Size::new(200.0, 300.0));

        state.scroll_by(Point::new(50.0, 100.0));
        assert_eq!(state.logical_offset, Point::new(50.0, 100.0));

        state.scroll_by(Point::new(-25.0, 50.0));
        assert_eq!(state.logical_offset, Point::new(25.0, 150.0));

        // Scroll beyond bounds - should clamp
        state.scroll_by(Point::new(1000.0, 1000.0));
        assert_eq!(state.logical_offset, Point::new(300.0, 700.0));
    }

    #[test]
    fn test_anchor_mode_clears_on_absolute() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(500.0, 1000.0));
        state.set_viewport_extent(Size::new(200.0, 300.0));

        // Set anchor (using Point for axis-agnostic offsets)
        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        );
        state.set_anchor(anchor);
        assert!(state.anchor.is_some());

        // Absolute scroll clears anchor
        state.set_offset(Point::new(100.0, 200.0));
        assert!(state.anchor.is_none());
    }

    #[test]
    fn test_scroll_fraction() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(500.0, 1000.0));
        state.set_viewport_extent(Size::new(200.0, 300.0));

        // At origin
        assert_eq!(state.scroll_fraction(), (0.0, 0.0));

        // At max
        state.set_offset(Point::new(300.0, 700.0));
        assert_eq!(state.scroll_fraction(), (1.0, 1.0));

        // At half
        state.set_offset(Point::new(150.0, 350.0));
        assert_eq!(state.scroll_fraction(), (0.5, 0.5));
    }

    #[test]
    fn test_thumb_fraction() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(500.0, 1000.0));
        state.set_viewport_extent(Size::new(200.0, 300.0));

        let (fx, fy) = state.thumb_fraction();
        assert!((fx - 0.4).abs() < 0.001); // 200/500
        assert!((fy - 0.3).abs() < 0.001); // 300/1000
    }

    #[test]
    fn test_boundary_detection() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(500.0, 1000.0));
        state.set_viewport_extent(Size::new(200.0, 300.0));

        // At start
        assert_eq!(state.at_start(), (true, true));
        assert_eq!(state.at_end(), (false, false));

        // At end
        state.set_offset(Point::new(300.0, 700.0));
        assert_eq!(state.at_start(), (false, false));
        assert_eq!(state.at_end(), (true, true));

        // In middle
        state.set_offset(Point::new(150.0, 350.0));
        assert_eq!(state.at_start(), (false, false));
        assert_eq!(state.at_end(), (false, false));
    }

    #[test]
    fn test_gesture_state_priority() {
        let mut gs = GestureState::new();

        // Start wheel gesture (vertical only)
        let wheel = InputGesture {
            source: GestureSource::Wheel,
            lifetime: GestureLifetime::Discrete,
        };
        // try_start returns (accepted_h, accepted_v)
        assert_eq!(gs.try_start(wheel, false, true), (false, true));
        assert!(gs.vertical.is_some());

        // Programmatic should fail (lower priority)
        let prog = InputGesture {
            source: GestureSource::Programmatic,
            lifetime: GestureLifetime::Discrete,
        };
        assert_eq!(gs.try_start(prog, false, true), (false, false));
        assert_eq!(gs.vertical.unwrap().source, GestureSource::Wheel);

        // Thumb should succeed (higher priority)
        let thumb = InputGesture {
            source: GestureSource::Thumb,
            lifetime: GestureLifetime::Continuous,
        };
        assert_eq!(gs.try_start(thumb, false, true), (false, true));
        assert_eq!(gs.vertical.unwrap().source, GestureSource::Thumb);
    }

    #[test]
    fn test_allows_programmatic() {
        let mut gs = GestureState::new();
        assert_eq!(gs.allows_programmatic(), (true, true));

        let wheel = InputGesture {
            source: GestureSource::Wheel,
            lifetime: GestureLifetime::Discrete,
        };
        gs.try_start(wheel, false, true);

        // Programmatic not allowed on Y axis while wheel active
        assert_eq!(gs.allows_programmatic(), (true, false));
    }

    // =========================================================================
    // Content Extension Contract Tests (INV-SCROLL-EXTEND-*)
    // =========================================================================

    #[test]
    fn test_extend_content_height_preserves_offset() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(400.0);

        let offset_before = state.logical_offset.y;
        state.extend_content_height(500.0);

        // INV-SCROLL-EXTEND-2: Offset unchanged
        assert_eq!(state.logical_offset.y, offset_before);
        // Content extent increased
        assert_eq!(state.content_extent.height, 1500.0);
        // max_scroll increased
        assert_eq!(state.max_scroll().y, 1300.0); // 1500 - 200
    }

    #[test]
    fn test_extend_content_height_does_not_touch_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        state.extend_content_height(500.0);

        // INV-SCROLL-EXTEND-3: Anchor not cleared
        assert!(state.anchor.is_some());
        assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(42));
    }

    #[test]
    fn test_extend_content_height_zero_is_noop() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        state.extend_content_height(0.0);

        assert_eq!(state.content_extent.height, 1000.0);
    }

    #[test]
    fn test_extend_content_width_preserves_offset() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(1000.0, 400.0));
        state.set_viewport_extent(Size::new(200.0, 400.0));
        state.set_offset_x(400.0);

        let offset_before = state.logical_offset.x;
        state.extend_content_width(500.0);

        assert_eq!(state.logical_offset.x, offset_before);
        assert_eq!(state.content_extent.width, 1500.0);
    }

    // =========================================================================
    // Content Prepend Contract Tests (INV-SCROLL-PREPEND-*)
    // =========================================================================

    #[test]
    fn test_prepend_content_height_offset_compensation() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(400.0);

        let offset_before = state.logical_offset.y;
        state.prepend_content_height(500.0);

        // INV-SCROLL-PREPEND-2: Offset increased by delta
        assert_eq!(state.logical_offset.y, offset_before + 500.0);
        // Content extent increased
        assert_eq!(state.content_extent.height, 1500.0);
        // max_scroll = 1500 - 200 = 1300
        assert_eq!(state.max_scroll().y, 1300.0);
    }

    #[test]
    fn test_prepend_content_height_does_not_touch_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        state.prepend_content_height(500.0);

        // INV-SCROLL-PREPEND-3: Anchor not cleared
        assert!(state.anchor.is_some());
        assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(42));
    }

    #[test]
    fn test_prepend_content_height_zero_is_noop() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(300.0);

        let offset_before = state.logical_offset.y;
        state.prepend_content_height(0.0);

        assert_eq!(state.content_extent.height, 1000.0);
        assert_eq!(state.logical_offset.y, offset_before);
    }

    #[test]
    fn test_prepend_content_width_offset_compensation() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(1000.0, 400.0));
        state.set_viewport_extent(Size::new(200.0, 400.0));
        state.set_offset_x(400.0);

        let offset_before = state.logical_offset.x;
        state.prepend_content_width(500.0);

        assert_eq!(state.logical_offset.x, offset_before + 500.0);
        assert_eq!(state.content_extent.width, 1500.0);
    }

    // =========================================================================
    // Content Trim Contract Tests (INV-SCROLL-TRIM-*)
    // =========================================================================

    #[test]
    fn test_trim_content_top_offset_compensation() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(400.0);

        state.trim_content_top(100.0);

        // INV-SCROLL-TRIM-2: Offset decreased by delta
        assert_eq!(state.logical_offset.y, 300.0);
        // Content extent shrinks
        assert_eq!(state.content_extent.height, 900.0);
        // max_scroll = 900 - 200 = 700
        assert_eq!(state.max_scroll().y, 700.0);
    }

    #[test]
    fn test_trim_content_top_clamps_to_zero() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(50.0);

        // Trim more than current offset
        state.trim_content_top(100.0);

        // INV-SCROLL-TRIM-2: Offset clamped to 0
        assert_eq!(state.logical_offset.y, 0.0);
        assert_eq!(state.content_extent.height, 900.0);
    }

    #[test]
    fn test_trim_content_bottom_offset_unchanged() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(300.0);

        state.trim_content_bottom(200.0);

        // INV-SCROLL-TRIM-2: Offset unchanged (not near bottom)
        assert_eq!(state.logical_offset.y, 300.0);
        // Content extent shrinks
        assert_eq!(state.content_extent.height, 800.0);
        // max_scroll = 800 - 200 = 600
        assert_eq!(state.max_scroll().y, 600.0);
    }

    #[test]
    fn test_trim_content_bottom_clamps_offset() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        // Scroll to max (800)
        state.set_offset_y(800.0);

        // Trim 300 from bottom
        state.trim_content_bottom(300.0);

        // New max = 700 - 200 = 500, offset must clamp
        assert_eq!(state.logical_offset.y, 500.0);
        assert_eq!(state.content_extent.height, 700.0);
    }

    #[test]
    fn test_trim_content_top_preserves_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        state.trim_content_top(100.0);

        // INV-SCROLL-TRIM-4: Anchor not mutated
        assert!(state.anchor.is_some());
        assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(42));
    }

    #[test]
    fn test_trim_content_bottom_preserves_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(99),
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
        );
        state.set_anchor(anchor);

        state.trim_content_bottom(200.0);

        // INV-SCROLL-TRIM-4: Anchor not mutated
        assert!(state.anchor.is_some());
        assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(99));
    }

    #[test]
    fn test_trim_content_zero_is_noop() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(300.0);

        let offset_before = state.logical_offset.y;
        let content_before = state.content_extent.height;

        state.trim_content_top(0.0);
        assert_eq!(state.logical_offset.y, offset_before);
        assert_eq!(state.content_extent.height, content_before);

        state.trim_content_bottom(0.0);
        assert_eq!(state.logical_offset.y, offset_before);
        assert_eq!(state.content_extent.height, content_before);
    }

    // =========================================================================
    // Anchor Invalidation Contract Tests (INV-ANCHOR-INV-*)
    // =========================================================================

    #[test]
    fn test_invalidate_anchor_predicate_matches() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);
        assert!(state.anchor.is_some());

        // INV-ANCHOR-INV-2: Predicate returns true → anchor cleared
        state.invalidate_anchor_if(|a| a.target_id.0 == 42);
        assert!(state.anchor.is_none());
    }

    #[test]
    fn test_invalidate_anchor_predicate_fails() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        // INV-ANCHOR-INV-2: Predicate returns false → anchor preserved
        state.invalidate_anchor_if(|a| a.target_id.0 == 999);
        assert!(state.anchor.is_some());
        assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(42));
    }

    #[test]
    fn test_invalidate_anchor_offset_unchanged() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(300.0);

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let offset_before = state.logical_offset;

        // INV-ANCHOR-INV-3: No offset mutation
        state.invalidate_anchor_if(|_| true);

        assert_eq!(state.logical_offset, offset_before);
        assert!(state.anchor.is_none());
    }

    #[test]
    fn test_invalidate_anchor_no_anchor_is_noop() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(300.0);

        assert!(state.anchor.is_none());

        let offset_before = state.logical_offset;
        let content_before = state.content_extent;

        // INV-ANCHOR-INV-4: No anchor = no-op (predicate should not even be called)
        let mut predicate_called = false;
        state.invalidate_anchor_if(|_| {
            predicate_called = true;
            true
        });

        assert!(
            !predicate_called,
            "Predicate should not be called when no anchor"
        );
        assert!(state.anchor.is_none());
        assert_eq!(state.logical_offset, offset_before);
        assert_eq!(state.content_extent, content_before);
    }

    #[test]
    fn test_invalidate_anchor_idempotent() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        // First call clears
        state.invalidate_anchor_if(|_| true);
        assert!(state.anchor.is_none());

        // INV-ANCHOR-INV-4: Second call is safe (no-op since anchor already gone)
        state.invalidate_anchor_if(|_| true);
        assert!(state.anchor.is_none());
    }

    #[test]
    fn test_invalidate_anchor_typical_trim_scenario() {
        let mut state = ScrollState::new();
        let item_height = 40.0;
        let initial_items = 100;
        state.set_content_extent(Size::new(400.0, initial_items as f32 * item_height));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(2000.0); // Viewing items 50-55

        // Anchor on item 30
        let anchor = ScrollAnchor::new(
            AnchorId::new(30),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        );
        state.set_anchor(anchor);

        // Trim top 20 items (items 0-19 removed)
        let trimmed_items = 20u64;
        state.trim_content_top(trimmed_items as f32 * item_height);

        // Anchor at item 30 is still valid (30 >= 20), but now its ID should be
        // remapped conceptually. For this test we check that it survives trim.
        assert!(state.anchor.is_some());

        // Now invalidate if anchor was in trimmed range
        state.invalidate_anchor_if(|a| a.target_id.0 < trimmed_items);
        // Item 30 >= 20, so anchor should be preserved
        assert!(state.anchor.is_some());

        // Trim more (items 20-39 now removed, total 40 items gone)
        let more_trimmed = 20u64;
        state.trim_content_top(more_trimmed as f32 * item_height);
        let total_trimmed = trimmed_items + more_trimmed;

        // Now anchor at item 30 is in trimmed range (30 < 40)
        state.invalidate_anchor_if(|a| a.target_id.0 < total_trimmed);
        assert!(
            state.anchor.is_none(),
            "Anchor should be cleared when target is trimmed"
        );
    }

    // =========================================================================
    // Anchor Resolution Read-Only Path Tests (INV-ANCHOR-RESOLVE-RO-*)
    // =========================================================================

    /// Test resolver that tracks call count and returns a fixed position
    struct CountingResolver {
        call_count: std::cell::Cell<usize>,
        result: Option<Point>,
    }

    impl CountingResolver {
        fn new(result: Option<Point>) -> Self {
            Self {
                call_count: std::cell::Cell::new(0),
                result,
            }
        }

        fn calls(&self) -> usize {
            self.call_count.get()
        }
    }

    impl AnchorResolver for CountingResolver {
        fn resolve_anchor(&self, _anchor: &ScrollAnchor) -> Option<Point> {
            self.call_count.set(self.call_count.get() + 1);
            self.result
        }

        fn find_nearest_anchor(&self, _anchor: &ScrollAnchor) -> Option<ScrollAnchor> {
            None
        }
    }

    #[test]
    fn test_resolve_anchor_position_calls_resolver_once() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(100.0, 500.0)));

        // INV-ANCHOR-RESOLVE-RO-2: Resolver called exactly once
        let result = state.resolve_anchor_position(&resolver);

        assert_eq!(resolver.calls(), 1, "Resolver must be called exactly once");
        assert_eq!(result, Some(Point::new(100.0, 500.0)));
    }

    #[test]
    fn test_resolve_anchor_position_no_call_when_no_anchor() {
        let state = ScrollState::new();

        let resolver = CountingResolver::new(Some(Point::new(100.0, 500.0)));

        // INV-ANCHOR-RESOLVE-RO-2: Resolver not called when no anchor
        let result = state.resolve_anchor_position(&resolver);

        assert_eq!(
            resolver.calls(),
            0,
            "Resolver must not be called when no anchor"
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_anchor_position_offset_unchanged() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(300.0);

        let offset_before = state.logical_offset;

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(100.0, 500.0)));

        // INV-ANCHOR-RESOLVE-RO-1: No offset mutation
        let _ = state.resolve_anchor_position(&resolver);

        assert_eq!(
            state.logical_offset, offset_before,
            "Offset must not change"
        );
    }

    #[test]
    fn test_resolve_anchor_position_anchor_unchanged() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(10.0, 20.0),
            Point::new(30.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(100.0, 500.0)));

        // INV-ANCHOR-RESOLVE-RO-1: No anchor mutation
        let _ = state.resolve_anchor_position(&resolver);

        assert!(state.anchor.is_some(), "Anchor must still exist");
        let a = state.anchor.unwrap();
        assert_eq!(a.target_id, AnchorId::new(42));
        assert_eq!(a.target_offset, Point::new(10.0, 20.0));
        assert_eq!(a.viewport_offset, Point::new(30.0, 50.0));
    }

    #[test]
    fn test_resolve_anchor_position_geometry_unchanged() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let content_before = state.content_extent;
        let viewport_before = state.viewport_extent;

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(100.0, 500.0)));

        // INV-ANCHOR-RESOLVE-RO-1: No geometry mutation
        let _ = state.resolve_anchor_position(&resolver);

        assert_eq!(
            state.content_extent, content_before,
            "Content extent must not change"
        );
        assert_eq!(
            state.viewport_extent, viewport_before,
            "Viewport extent must not change"
        );
    }

    #[test]
    fn test_resolve_anchor_position_returns_none_on_resolution_failure() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        // Resolver returns None (target not found)
        let resolver = CountingResolver::new(None);

        // INV-ANCHOR-RESOLVE-RO-4: Returns None, no invalidation
        let result = state.resolve_anchor_position(&resolver);

        assert!(result.is_none());
        assert!(
            state.anchor.is_some(),
            "Anchor must not be invalidated on resolution failure"
        );
    }

    // =========================================================================
    // Anchor-Based Scroll Jump Tests (INV-ANCHOR-JUMP-*)
    // =========================================================================

    #[test]
    fn test_jump_to_anchor_succeeds_with_valid_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        // Anchor with viewport_offset = (0, 50) means target should appear 50px from top
        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        // Resolver returns target at (0, 500)
        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        let result = state.jump_to_anchor(&resolver);

        // Jump should succeed
        assert!(result, "Jump should succeed with valid anchor");

        // INV-ANCHOR-JUMP-3: Offset = resolved - viewport_offset = 500 - 50 = 450
        assert_eq!(state.logical_offset.y, 450.0);
    }

    #[test]
    fn test_jump_to_anchor_returns_false_when_no_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(100.0);

        assert!(state.anchor.is_none());

        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        // INV-ANCHOR-JUMP-1: No anchor = no jump
        let result = state.jump_to_anchor(&resolver);

        assert!(!result, "Jump should fail when no anchor");
        // Offset unchanged
        assert_eq!(state.logical_offset.y, 100.0);
        // Resolver not called
        assert_eq!(resolver.calls(), 0);
    }

    #[test]
    fn test_jump_to_anchor_returns_false_when_resolver_fails() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(100.0);

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        // Resolver returns None (target not found)
        let resolver = CountingResolver::new(None);

        let result = state.jump_to_anchor(&resolver);

        assert!(!result, "Jump should fail when resolver returns None");
        // Offset unchanged
        assert_eq!(state.logical_offset.y, 100.0);
    }

    #[test]
    fn test_jump_to_anchor_offset_computation() {
        let mut state = ScrollState::new();
        // Content larger than viewport in both dimensions to allow scrolling
        state.set_content_extent(Size::new(800.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        // max_scroll = (400, 1800)

        // Target at (30, 100) in content space, should appear at (10, 25) in viewport
        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(10.0, 25.0), // viewport_offset
        );
        state.set_anchor(anchor);

        // Resolver returns target position
        let resolver = CountingResolver::new(Some(Point::new(30.0, 100.0)));

        state.jump_to_anchor(&resolver);

        // offset = resolved - viewport_offset
        // x = 30 - 10 = 20
        // y = 100 - 25 = 75
        assert_eq!(state.logical_offset.x, 20.0);
        assert_eq!(state.logical_offset.y, 75.0);
    }

    #[test]
    fn test_jump_to_anchor_clamps_correctly() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 1000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        // max_scroll = (0, 800)

        // Anchor that would result in negative offset
        let anchor = ScrollAnchor::new(
            AnchorId::new(1),
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0), // viewport_offset
        );
        state.set_anchor(anchor);

        // Resolver returns target at (0, 50) - offset would be 50 - 100 = -50
        let resolver = CountingResolver::new(Some(Point::new(0.0, 50.0)));

        state.jump_to_anchor(&resolver);

        // Clamped to 0
        assert_eq!(state.logical_offset.y, 0.0);

        // Now test clamping to max
        let anchor2 = ScrollAnchor::new(
            AnchorId::new(2),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0), // viewport_offset
        );
        state.set_anchor(anchor2);

        // Resolver returns target at (0, 900) - would exceed max (800)
        let resolver2 = CountingResolver::new(Some(Point::new(0.0, 900.0)));

        state.jump_to_anchor(&resolver2);

        // Clamped to max
        assert_eq!(state.logical_offset.y, 800.0);
    }

    #[test]
    fn test_jump_to_anchor_preserves_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(10.0, 20.0),
            Point::new(30.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(100.0, 500.0)));

        state.jump_to_anchor(&resolver);

        // INV-ANCHOR-JUMP-4: Anchor must still exist with same values
        assert!(
            state.anchor.is_some(),
            "Anchor must be preserved after jump"
        );
        let a = state.anchor.unwrap();
        assert_eq!(a.target_id, AnchorId::new(42));
        assert_eq!(a.target_offset, Point::new(10.0, 20.0));
        assert_eq!(a.viewport_offset, Point::new(30.0, 50.0));
    }

    #[test]
    fn test_jump_to_anchor_calls_resolver_once() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        state.jump_to_anchor(&resolver);

        // INV-ANCHOR-JUMP-2: Resolver called exactly once
        assert_eq!(resolver.calls(), 1, "Resolver must be called exactly once");
    }

    #[test]
    fn test_jump_to_anchor_no_geometry_side_effects() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let content_before = state.content_extent;
        let viewport_before = state.viewport_extent;
        let max_before = state.max_scroll();

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        state.jump_to_anchor(&resolver);

        // INV-ANCHOR-JUMP-5: No side effects - geometry unchanged
        assert_eq!(state.content_extent, content_before);
        assert_eq!(state.viewport_extent, viewport_before);
        assert_eq!(state.max_scroll(), max_before);
    }

    // =========================================================================
    // Anchor Resolution Integration Tests (INV-ANCHOR-COMMIT-*)
    // =========================================================================

    #[test]
    fn test_resolve_and_jump_anchor_jump_preserve_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        // INV-ANCHOR-COMMIT-2: clear_after = false → anchor preserved
        let result = state.resolve_and_jump_anchor(&resolver, false);

        assert!(result, "Jump should succeed");
        assert!(
            state.anchor.is_some(),
            "Anchor must be preserved when clear_after = false"
        );
        assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(42));
    }

    #[test]
    fn test_resolve_and_jump_anchor_jump_clear_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        // INV-ANCHOR-COMMIT-2: clear_after = true → anchor cleared
        let result = state.resolve_and_jump_anchor(&resolver, true);

        assert!(result, "Jump should succeed");
        assert!(
            state.anchor.is_none(),
            "Anchor must be cleared when clear_after = true"
        );
    }

    #[test]
    fn test_resolve_and_jump_anchor_resolver_failure_preserves_anchor() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(100.0);

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let offset_before = state.logical_offset.y;

        // Resolver returns None (target not found)
        let resolver = CountingResolver::new(None);

        // INV-ANCHOR-COMMIT-3: Resolver failure does not clear anchor
        let result = state.resolve_and_jump_anchor(&resolver, true);

        assert!(!result, "Jump should fail when resolver returns None");
        assert!(
            state.anchor.is_some(),
            "Anchor must be preserved on resolver failure"
        );
        assert_eq!(
            state.logical_offset.y, offset_before,
            "Offset must not change on failure"
        );
    }

    #[test]
    fn test_resolve_and_jump_anchor_no_anchor_noop() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));
        state.set_offset_y(100.0);

        assert!(state.anchor.is_none());

        let offset_before = state.logical_offset.y;
        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        // No anchor = no jump
        let result = state.resolve_and_jump_anchor(&resolver, true);

        assert!(!result, "Should return false when no anchor");
        assert_eq!(
            state.logical_offset.y, offset_before,
            "Offset must not change"
        );
        assert_eq!(
            resolver.calls(),
            0,
            "Resolver must not be called when no anchor"
        );
    }

    #[test]
    fn test_resolve_and_jump_anchor_resolver_called_once() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        state.resolve_and_jump_anchor(&resolver, true);

        // INV-ANCHOR-COMMIT-4: Resolver called exactly once
        assert_eq!(resolver.calls(), 1, "Resolver must be called exactly once");
    }

    #[test]
    fn test_resolve_and_jump_anchor_offset_correct() {
        let mut state = ScrollState::new();
        state.set_content_extent(Size::new(400.0, 2000.0));
        state.set_viewport_extent(Size::new(400.0, 200.0));

        // Anchor with viewport_offset = (0, 50) means target should appear 50px from top
        let anchor = ScrollAnchor::new(
            AnchorId::new(42),
            Point::new(0.0, 0.0),
            Point::new(0.0, 50.0),
        );
        state.set_anchor(anchor);

        // Resolver returns target at (0, 500)
        let resolver = CountingResolver::new(Some(Point::new(0.0, 500.0)));

        state.resolve_and_jump_anchor(&resolver, false);

        // Offset = resolved - viewport_offset = 500 - 50 = 450
        assert_eq!(state.logical_offset.y, 450.0);
    }
}
