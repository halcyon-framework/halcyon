// SPEC-HAL-011: ScrollArea (Scrolling Container)
// SPEC-HAL-131A: ScrollArea owns canonical ScrollState. No smoothing.
//
// ============================================================================
// VIRTUALIZATION RENDER & MOTION MODEL ENFORCEMENT
// ============================================================================
//
// ScrollArea is the SOLE consumer of logical_offset for paint transforms.
// See scroll_state.rs for the full invariant set (INV-VIRT-SCROLL-*).
//
// KEY ENFORCEMENT POINTS:
//
// 1. scroll_offset() returns the effective visual offset for paint transform.
//    - This is the ONLY source of scroll transform in the render pipeline.
//    - INV-VIRT-SCROLL-1: Continuous offset, no snapping.
//
// 2. paint() applies no special virtualization logic.
//    - Children are painted at their layout-assigned positions.
//    - Scroll transform is applied by the tree traversal using scroll_offset().
//    - INV-VIRT-SCROLL-2: Widgets at true absolute coordinates.
//
// 3. Anchor resolution happens in the engine post-layout phase before paint.
//    - Ordinary anchors resolve into the base logical offset.
//    - SPEC-HAL-352 tail slack augments only the effective visual offset.
//
// VALIDATION CHECKLIST:
// - [ ] Scroll with anchor active → content moves smoothly
// - [ ] Insert/remove above viewport → anchored content stays visually fixed
// - [ ] Fast scroll → no popping, no snapping
// - [ ] Disable virtualization → identical visual behavior

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use web_time::{Duration, Instant};

use crate::core::deferred_wiring::{Deferred, DeferredWiring};
use crate::core::event::{
    Event, EventPhase, GestureAxis, GestureKind, PointerEvent, PointerEventKind, PointerType,
    QualifiedTouchMotion, ScrollIntentAdmission, TouchPressIdentity, TouchScrollMotion,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{Color, LAYER_SCROLLBAR};
use crate::core::scroll_container::{HorizontalScrollLink, ScrollContainer, VerticalScrollLink};
use crate::core::scroll_link::ScrollAxis;
use crate::core::scroll_state::{
    AnchorId, AnchorResolver, ScrollAnchor, ScrollGroupId, ScrollState,
};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::{NodeId, OverlayHitRegion};
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::scrollbar::{
    OVERLAY_SCROLLBAR_HIDE_DELAY_SECS, ScrollbarH, ScrollbarLayout, ScrollbarState, ScrollbarV,
};

const SCROLLBAR_FADE_SECS: f32 = 0.3;

/// Half-width of each retained separator band in content-space logical pixels.
const SEPARATOR_HIT_HALF_WIDTH: f32 = 3.0;

/// SPEC-HAL-268: Proximity scroll state
#[derive(Debug, Clone)]
struct ProximityScrollState {
    /// Whether proximity scrolling is currently active
    active: bool,
    /// Cursor position at moment of activation (local coords) — displacement anchor
    anchor: (f32, f32),
    /// Scroll offset at moment of activation — for future peek/return mode
    scroll_origin: (f32, f32),
    /// Last known cursor position in local coordinates
    cursor_pos: (f32, f32),
    /// Activation source
    source: ProximityScrollSource,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProximityScrollSource {
    None,
    CtrlDoubleTap,
    ModifierCombo,
    MiddleMouse,
}

impl Default for ProximityScrollState {
    fn default() -> Self {
        Self {
            active: false,
            anchor: (0.0, 0.0),
            scroll_origin: (0.0, 0.0),
            cursor_pos: (0.0, 0.0),
            source: ProximityScrollSource::None,
        }
    }
}

/// SPEC-HAL-352: Intent for an explicit vertical layout-change anchor transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutScrollAnchorIntent {
    /// Preserve the anchor widget's viewport Y, using transient tail slack if needed.
    CollapsePreserveHeader,
    /// Preserve the anchor when possible and minimally reveal the affected region.
    ExpandRevealRegion,
}

/// Engine-owned pre-layout snapshot for one layout-change anchor transaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayoutScrollAnchorTransaction {
    pub anchor: TreeWidgetId,
    pub affected_region: TreeWidgetId,
    pub intent: LayoutScrollAnchorIntent,
    pub pre_anchor_viewport_y: f32,
    pub scrollport_size: Size,
}

/// Engine-owned post-layout geometry used to resolve one transaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayoutScrollAnchorResolution {
    pub anchor_content_y: f32,
    pub affected_region_bottom_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ActiveLayoutAnchorSlack {
    preserved_effective_y: f32,
    tail_slack_y: f32,
}

/// Scrolling container for large content.
///
/// SPEC-HAL-131A: ScrollArea owns canonical ScrollState. No smoothing; logical_offset is visual.
///
/// # Invariant: ScrollArea Requires Tight Constraints Final Pass
///
/// ScrollArea only finalizes scroll metrics (max_scroll, content extent, etc.) when it
/// receives a **final/tight** layout pass (constraints.min == constraints.max). When
/// ScrollArea is mounted as a screen root or otherwise receives only loose constraints,
/// it will **never** hit final-pass → `max_scroll` stays `0` even when `content_h > viewport_h`.
///
/// **Embedders must structure layout so ScrollArea gets at least one tight pass per frame.**
///
/// Example fix: Wrap ScrollArea in a Flex container with `FlexParams::flexible(1.0)`.
/// Controls the visibility of this ScrollArea's existing overlay scrollbars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAreaScrollbarRevealPolicy {
    /// Use the existing initial visibility and thumb-hover behavior.
    #[default]
    DefaultLegacy,
    /// Start hidden and reveal on actual scrolling or pointer proximity to the rail.
    AutoHideOnScrollOrRailProximity,
}

struct TouchScroll {
    press: TouchPressIdentity,
    axis: GestureAxis,
    original_offset: f32,
    original_anchor: Option<ScrollAnchor>,
    samples: TouchScrollSamples,
}

const TOUCH_SAMPLE_HISTORY: Duration = Duration::from_millis(80);
const TOUCH_SAMPLE_MIN_SPAN: Duration = Duration::from_millis(8);
const TOUCH_RELEASE_MAX_GAP: Duration = Duration::from_millis(100);
const TOUCH_KINETIC_DECAY: f64 = 4.0;
const TOUCH_KINETIC_MAX_SPEED: f32 = 4000.0;
const TOUCH_KINETIC_START_SPEED: f32 = 80.0;
const TOUCH_KINETIC_STOP_SPEED: f64 = 8.0;

#[derive(Default)]
struct TouchScrollSamples(VecDeque<(Instant, f32)>);

impl TouchScrollSamples {
    fn axis_position(axis: GestureAxis, position: Point) -> f32 {
        match axis {
            GestureAxis::Horizontal => position.x,
            GestureAxis::Vertical => position.y,
            GestureAxis::Any => unreachable!("HAL-396 locks one scroll axis"),
        }
    }

    fn record(&mut self, axis: GestureAxis, time: Instant, position: Point) {
        let position = Self::axis_position(axis, position);
        if !position.is_finite() {
            return;
        }
        if let Some(&(last_time, last_position)) = self.0.back() {
            if time < last_time || position == last_position {
                return;
            }
            if time == last_time {
                self.0.pop_back();
            }
        }
        while self.0.front().is_some_and(|&(at, _)| time - at > TOUCH_SAMPLE_HISTORY) {
            self.0.pop_front();
        }
        self.0.push_back((time, position));
    }

    fn release_velocity(&self, release: Instant) -> Option<f32> {
        let &(start, first) = self.0.front()?;
        let &(end, last) = self.0.back()?;
        let span = end - start;
        if span < TOUCH_SAMPLE_MIN_SPAN
            || release < end || release - end > TOUCH_RELEASE_MAX_GAP
        {
            return None;
        }
        let velocity = (first - last) / span.as_secs_f32();
        if !velocity.is_finite() {
            return None;
        }
        let velocity = velocity.clamp(-TOUCH_KINETIC_MAX_SPEED, TOUCH_KINETIC_MAX_SPEED);
        (velocity.abs() >= TOUCH_KINETIC_START_SPEED).then_some(velocity)
    }
}

struct KineticTouchScroll {
    press: TouchPressIdentity,
    axis: GestureAxis,
    released_at: Instant,
    origin: f32,
    velocity: f32,
}

impl KineticTouchScroll {
    fn position(&self, now: Instant, maximum: f32) -> (f32, bool) {
        let elapsed = now.saturating_duration_since(self.released_at).as_secs_f64();
        let residual = (-TOUCH_KINETIC_DECAY * elapsed).exp();
        let velocity = f64::from(self.velocity);
        let origin = f64::from(self.origin);
        let terminal = (velocity * residual).abs() < TOUCH_KINETIC_STOP_SPEED;
        let fraction = if terminal { 1.0 } else { 1.0 - residual };
        let position = (origin + (velocity / TOUCH_KINETIC_DECAY) * fraction) as f32;
        let position = position.clamp(0.0, maximum);
        let at_bound = (self.velocity > 0.0 && position >= maximum)
            || (self.velocity < 0.0 && position <= 0.0);
        (position, terminal || at_bound)
    }
}

/// See SPEC-HAL-088 Section 15 for full documentation.
pub struct ScrollArea {
    id: WidgetInternalId,
    /// SPEC-HAL-154: Content NodeId with deferred wiring.
    /// Wired at construction via new(content) or post-mount via wire_content().
    content: Deferred<NodeId>,
    /// SPEC-HAL-154: Sealed after first layout.
    sealed: bool,
    /// SPEC-HAL-131A: Canonical scroll state (sole owner)
    state: ScrollState,
    pub show_scrollbars: bool,
    /// Enable vertical scrollbar (default: true)
    enable_scrollbar_v: bool,
    /// Enable horizontal scrollbar (default: false for now)
    enable_scrollbar_h: bool,
    // Configured viewport size (if set, enforces fixed viewport)
    configured_viewport: Option<Size>,
    /// SPEC-HAL-086 Stage 8: Vertical scrollbar state
    scrollbar_v: ScrollbarV,
    /// SPEC-HAL-086 Stage 8: Horizontal scrollbar state
    scrollbar_h: ScrollbarH,
    /// SPEC-HAL-133: Optional scroll group for linked scrolling
    scroll_group: Option<ScrollGroupId>,
    /// SPEC-HAL-133: Frame token for loop prevention in linked scrolling
    link_frame_token: u64,
    /// Frame-driven smooth scrolling container.
    /// Input events update state.logical_offset (the target), frame loop smooths
    /// visual_offset toward it via update_visual(). scroll_offset() returns visual.
    smooth: ScrollContainer,
    scroll_intent_admission: ScrollIntentAdmission,
    touch_scroll: Option<TouchScroll>,
    kinetic_touch_scroll: Option<KineticTouchScroll>,
    /// SPEC-HAL-086 Stage N: Optional horizontal scroll link for external alignment.
    /// When set, ScrollArea writes its visual horizontal offset to this link.
    /// Header/footer widgets read it to apply the same X translation.
    horizontal_scroll_link: Option<HorizontalScrollLink>,
    /// SPEC-HAL-366: Suppress publication until first tight X restoration.
    pending_horizontal_restore: bool,
    /// Retained vertical coordinate shared with a sequential replacement.
    vertical_scroll_link: Option<VerticalScrollLink>,
    /// True until the first tight layout consumes the retained coordinate.
    pending_vertical_restore: bool,
    /// Debug-only: Track if we've warned about missing tight constraints.
    /// Rate-limits the warning to once per instance until a final pass occurs.
    #[cfg(debug_assertions)]
    warned_no_tight_constraints: bool,
    // =========================================================================
    // SPEC-HAL-110 Section 18: Scrollbar Visibility & Gutter Semantics
    // INV-ROOT-INSET-SCROLLABLE: root_inset is scroll-domain padding.
    // =========================================================================
    //
    // INV-SCROLLBAR-CONTAINER-SPACE: container_size used for scrollbar geometry.
    // INV-VIEWPORT-ORIGIN-ZERO: child clip covers the full container at origin.
    // INV-NO-FROZEN-GUTTER: content_offset positions child, clip is at (0,0).
    //
    // container_size: Full allocation from parent (scrollbar positioning)
    // content_offset: root_inset value (child positioning, NOT clip origin)
    // =========================================================================
    /// Full container size (for scrollbar hit testing).
    /// SPEC-HAL-110 INV-SCROLLBAR-CONTAINER-SPACE: Scrollbars use this, not viewport_extent.
    container_size: Size,
    /// Content offset from container origin (root_inset).
    /// INV-ROOT-INSET-SCROLLABLE: Used to position child at (root_inset, root_inset).
    /// Clip origin is (0, 0), so the inset is visible at scroll=0 and scrolls away.
    content_offset: f32,
    /// Retained separator centerlines relative to the content geometry origin.
    /// Post-layout sync stores this compatibility geometry projection.
    cached_separator_bands: Vec<f32>,

    /// External freeze signal for horizontal scroll (e.g., during column resize).
    /// When set and true, delta_x is zeroed in scroll events.
    /// INV-RESIZE-HSCROLL-2: While dragging, scroll_x is frozen.
    horizontal_freeze: Option<Rc<Cell<bool>>>,
    /// SPEC-HAL-268: Proximity scroll state
    proximity: ProximityScrollState,
    scrollbar_reveal_policy: ScrollAreaScrollbarRevealPolicy,
    /// Scrollbar auto-hide: countdown timer (seconds). Resets on activity.
    scrollbar_hide_timer: f32,
    /// Scrollbar auto-hide: current opacity (0.0–1.0).
    scrollbar_fade_alpha: f32,
    /// Scrollbar auto-hide: true when scroll activity occurred this frame.
    scrollbar_activity: bool,
    /// SPEC-HAL-319: Standalone empty mode for runtime composition.
    /// When true, layout/paint render an empty viewport without requiring
    /// content to be wired. Not produced by `new()` or `new_unwired()`;
    /// only via `empty()`.
    standalone_empty: bool,
    /// SPEC-PM-017 R1b: when true, the wired content widget performs its
    /// own vertical translation (f64, viewport-relative paint) and the
    /// scene Scroll node translates X only — scroll_offset() reports
    /// (visual_x, 0.0). Vertical scroll state, domain, scrollbars,
    /// smoothing, reveal, and sync_scroll dispatch are unchanged.
    content_owns_vertical_translation: bool,
    /// SPEC-HAL-352: One explicit transaction awaiting post-layout resolution.
    pending_layout_anchor: Option<LayoutScrollAnchorTransaction>,
    /// SPEC-HAL-352: Active tail slack preserving an effective vertical offset.
    active_layout_anchor: Option<ActiveLayoutAnchorSlack>,
    /// True only when the most recent layout call finalized tight-pass scroll metrics.
    layout_anchor_metrics_finalized: bool,
    /// Last tight-pass container size. Measurement-pass allocations are not
    /// resize testimony for SPEC-HAL-352 transactions.
    layout_anchor_final_container_size: Size,
}

impl ScrollArea {
    /// Create a ScrollArea with content wired at construction time.
    ///
    /// SPEC-HAL-154: Content is wired immediately. Sealing happens at first layout.
    pub fn new(content: NodeId) -> Self {
        let mut s = Self::new_unwired();
        s.content.set(content);
        s
    }

    /// Create a ScrollArea without content wired.
    ///
    /// SPEC-HAL-154: Content must be wired via `wire_content()` before first layout.
    /// Layout will panic if content is not wired.
    pub fn new_unwired() -> Self {
        Self {
            id: WidgetInternalId::new(),
            content: Deferred::new(),
            sealed: false,
            state: ScrollState::new(),
            show_scrollbars: true,
            enable_scrollbar_v: true,
            enable_scrollbar_h: false,
            configured_viewport: None,
            scrollbar_v: ScrollbarV::new(),
            scrollbar_h: ScrollbarH::new(),
            scroll_group: None,
            link_frame_token: 0,
            smooth: ScrollContainer::new(),
            scroll_intent_admission: ScrollIntentAdmission::default(),
            touch_scroll: None,
            kinetic_touch_scroll: None,
            horizontal_scroll_link: None,
            pending_horizontal_restore: false,
            vertical_scroll_link: None,
            pending_vertical_restore: false,
            #[cfg(debug_assertions)]
            warned_no_tight_constraints: false,
            container_size: Size::zero(),
            content_offset: 0.0,
            cached_separator_bands: Vec::new(),
            horizontal_freeze: None,
            proximity: ProximityScrollState::default(),
            scrollbar_reveal_policy: ScrollAreaScrollbarRevealPolicy::default(),
            scrollbar_hide_timer: OVERLAY_SCROLLBAR_HIDE_DELAY_SECS,
            scrollbar_fade_alpha: 1.0,
            scrollbar_activity: false,
            standalone_empty: false,
            content_owns_vertical_translation: false,
            pending_layout_anchor: None,
            active_layout_anchor: None,
            layout_anchor_metrics_finalized: false,
            layout_anchor_final_container_size: Size::zero(),
        }
    }

    /// SPEC-HAL-319: Standalone empty ScrollArea for runtime composition.
    /// Renders an empty viewport without requiring content wiring.
    /// The Deferred<content> stays unset; layout/paint short-circuit on
    /// the `standalone_empty` flag rather than calling `content.get()`.
    pub fn empty() -> Self {
        let mut s = Self::new_unwired();
        s.standalone_empty = true;
        s
    }

    /// SPEC-PM-017 R1b: content performs its own vertical translation in
    /// f64 (deep-content widgets — TreeView). The scene Scroll node then
    /// translates X only; see `scroll_offset()`.
    pub fn with_content_owned_vertical_translation(mut self) -> Self {
        self.content_owns_vertical_translation = true;
        self
    }

    /// Wire the content NodeId (SPEC-HAL-154: deferred wiring phase only).
    ///
    /// # Panics
    /// Panics if called after seal (SPEC-HAL-154: wiring phase violation).
    /// Panics if called twice (SPEC-HAL-154: single-assignment violation).
    pub fn wire_content(&mut self, content: NodeId) {
        assert!(
            !self.is_sealed(),
            "ScrollArea::wire_content called after seal (SPEC-HAL-154: wiring phase violation)"
        );
        self.clear_layout_anchor();
        self.content.set(content);
    }

    /// Get the content NodeId.
    ///
    /// # Panics
    /// Panics if content was never wired.
    pub fn content(&self) -> NodeId {
        self.content.get()
    }

    pub(crate) fn sealed_wired_content(&self) -> Option<NodeId> {
        (self.is_sealed() && self.content.is_set()).then(|| self.content())
    }

    /// SPEC-HAL-352: Current effective vertical offset used by the active
    /// translation owner. Tail slack is local and never changes true extent.
    pub(crate) fn effective_visual_scroll_y(&self) -> f32 {
        self.smooth.visual_offset().1
            + self
                .active_layout_anchor
                .map_or(0.0, |anchor| anchor.tail_slack_y)
    }

    /// SPEC-HAL-352: Read-only owner-local snapshot used by WidgetTree before
    /// applying a live layout mutation.
    pub(crate) fn layout_anchor_request_snapshot(&self) -> Option<(f32, f32, Size)> {
        let blocked = self.scrollbar_v.dragging
            || self.scrollbar_h.dragging
            || self.proximity.active
            || self.smooth.needs_animation()
            || self.scroll_intent_admission.blocks_competing_scroll();
        let size = self.layout_anchor_final_container_size;
        if blocked
            || !size.width.is_finite()
            || !size.height.is_finite()
            || size.width <= 0.0
            || size.height <= 0.0
        {
            return None;
        }

        Some((
            self.smooth.visual_offset().0,
            self.effective_visual_scroll_y(),
            size,
        ))
    }

    /// SPEC-HAL-352: Install one explicit transaction after WidgetTree has
    /// validated and snapshotted the pre-mutation geometry.
    pub(crate) fn begin_layout_anchor_transaction(
        &mut self,
        transaction: LayoutScrollAnchorTransaction,
    ) {
        self.cancel_pending_retained_restore();
        self.state.clear_anchor();
        self.active_layout_anchor = None;
        self.pending_layout_anchor = Some(transaction);
    }

    pub(crate) fn pending_layout_anchor(&self) -> Option<LayoutScrollAnchorTransaction> {
        self.pending_layout_anchor
    }

    pub(crate) fn cancel_pending_layout_anchor(&mut self) {
        self.pending_layout_anchor = None;
        self.active_layout_anchor = None;
    }

    fn clear_layout_anchor(&mut self) {
        self.pending_layout_anchor = None;
        self.active_layout_anchor = None;
    }

    fn retire_layout_anchor_for_scroll_intent(&mut self) {
        self.scroll_intent_admission.admit_scroll_intent();
        self.clear_layout_anchor();
        self.cancel_pending_retained_restore();
    }

    pub(crate) fn bind_scroll_intent_admission(&mut self, admission: ScrollIntentAdmission) {
        self.scroll_intent_admission = admission;
    }

    /// HAL-396: the winning axis tracks the finger immediately. The existing
    /// scroll state clamps against current metrics, including after relayout.
    fn jump_touch_scroll(
        &mut self,
        cx: &mut EventContext,
        axis: GestureAxis,
        position: f32,
        previous: Point,
    ) {
        self.set_touch_scroll_offset(axis, position);
        let current = self.touch_scroll_visual_offset();
        if current != previous {
            self.scrollbar_activity = true;
            self.publish_vertical_scroll_position();
            cx.invalidate_scroll_projection();
            if current.y != previous.y {
                cx.invalidate_scroll_dependent_ancestor_layout();
            }
        }
    }

    fn set_touch_scroll_offset(&mut self, axis: GestureAxis, position: f32) {
        match axis {
            GestureAxis::Horizontal => {
                self.state.set_offset_x(position);
                self.smooth.jump_to_offset_x(self.state.logical_offset.x);
            }
            GestureAxis::Vertical => {
                self.state.set_offset_y(position);
                self.smooth.jump_to_offset_y(self.state.logical_offset.y);
            }
            GestureAxis::Any => unreachable!("HAL-396 locks the selected axis"),
        }
    }

    pub(crate) fn advance_kinetic_scroll(&mut self, now: Instant) -> Option<GestureAxis> {
        let motion = self.kinetic_touch_scroll.take()?;
        if !self.scroll_intent_admission.owns_kinetic_scroll(motion.press) {
            return None;
        }
        let maximum = TouchScrollSamples::axis_position(motion.axis, self.state.max_scroll());
        let (position, settled) = motion.position(now, maximum);
        let previous = self.touch_scroll_visual_offset();
        self.set_touch_scroll_offset(motion.axis, position);
        let changed = previous != self.touch_scroll_visual_offset();
        if changed {
            self.scrollbar_activity = true;
            self.publish_vertical_scroll_position();
        }
        let axis = motion.axis;
        if settled {
            self.scroll_intent_admission.finish_kinetic_scroll(motion.press);
        } else {
            self.kinetic_touch_scroll = Some(motion);
        }
        changed.then_some(axis)
    }

    fn touch_scroll_visual_offset(&self) -> Point {
        Point::new(
            self.smooth.visual_offset().0,
            self.effective_visual_scroll_y(),
        )
    }

    fn apply_touch_scroll_motion(
        &mut self,
        cx: &mut EventContext,
        motion: TouchScrollMotion,
        previous: Point,
    ) {
        let position = match motion.axis {
            GestureAxis::Horizontal => previous.x - motion.delta.x,
            GestureAxis::Vertical => previous.y - motion.delta.y,
            GestureAxis::Any => unreachable!("HAL-396 locks the selected axis"),
        };
        if position.is_finite() {
            self.jump_touch_scroll(cx, motion.axis, position, previous);
        }
    }

    fn cancel_pending_retained_restore(&mut self) {
        self.pending_horizontal_restore = false;
        self.pending_vertical_restore = false;
    }

    fn pending_retained_horizontal_offset(&self) -> Option<f32> {
        if self.pending_horizontal_restore {
            self.horizontal_scroll_link
                .as_ref()
                .map(HorizontalScrollLink::get)
        } else {
            None
        }
    }

    fn pending_retained_vertical_offset(&self) -> Option<f32> {
        if self.pending_vertical_restore {
            self.vertical_scroll_link
                .as_ref()
                .map(VerticalScrollLink::retained_offset)
        } else {
            None
        }
    }

    fn publish_vertical_scroll_position(&self) {
        if self.pending_vertical_restore {
            return;
        }
        if let Some(link) = &self.vertical_scroll_link {
            link.publish(self.effective_visual_scroll_y());
        }
    }

    fn notify_scroll_input(&mut self, cx: &mut EventContext, previous: Point) {
        let current = self.state.logical_offset;
        if self.auto_hides_scrollbars() && current != previous {
            self.scrollbar_activity = true;
        }
        let horizontal_changed = (current.x - previous.x).abs() > 0.001;
        let vertical_changed = (current.y - previous.y).abs() > 0.001;
        if horizontal_changed || vertical_changed {
            cx.invalidate_paint();
        }
        if vertical_changed {
            cx.invalidate_scroll_dependent_ancestor_layout();
        }
    }

    pub(crate) fn publish_horizontal_scroll_position(&self) -> bool {
        if self.pending_horizontal_restore {
            return false;
        }
        let Some(link) = &self.horizontal_scroll_link else {
            return false;
        };
        let presented_x = self.smooth.visual_offset().0;
        if link.get().to_bits() == presented_x.to_bits() {
            return false;
        }
        link.set(presented_x);
        true
    }

    fn finalize_scroll_geometry(
        &mut self,
        container_size: Size,
        content_size: Size,
        domain_viewport: &mut Size,
        retained_horizontal_offset: Option<f32>,
        retained_vertical_offset: Option<f32>,
    ) -> f32 {
        self.record_final_layout_anchor_container_size(container_size);
        // Opposing overlays extend genuine overflow; they must not create it
        // on a fitting axis. Ordinary layout and restoration use current metrics.
        let bar_thickness = self.scrollbar_v.config.thickness;
        let vbar_occludes =
            self.enable_scrollbar_v && content_size.height > container_size.height + 0.5;
        let hbar_occludes =
            self.enable_scrollbar_h && content_size.width > container_size.width + 0.5;
        *domain_viewport = Size::new(
            (container_size.width
                - if vbar_occludes && content_size.width > container_size.width {
                    bar_thickness
                } else {
                    0.0
                })
            .max(0.0),
            (container_size.height
                - if hbar_occludes && content_size.height > container_size.height {
                    bar_thickness
                } else {
                    0.0
                })
            .max(0.0),
        );

        // =======================================================================
        // INV-ROOT-INSET-SCROLLABLE: root_inset is part of scroll domain.
        // INV-VIEWPORT-ORIGIN-ZERO: viewport_extent = container (no offset).
        // =======================================================================
        //
        // viewport_extent = domain_viewport: the container minus any strip
        // occluded by a visible opposing overlay scrollbar (occlusion-aware
        // INV-SCROLL-DOMAIN-CLOSED). max_scroll = content_extent - viewport_extent
        // therefore lets overflowing content clear the bar; a fitting axis
        // retains the full allocated extent.
        // content_extent = child content + 2*root_inset (padding in scroll domain).
        //
        // The reveal calculation in resolve_reveal_rect() must account for root_inset
        // separately, since the VISIBLE viewport is container - 2*root_inset.
        // =======================================================================
        self.state.set_viewport_extent(*domain_viewport);

        // SPEC-HAL-086 Stage 8 (REVISED): Since we always pass finite viewport width,
        // child should always return finite content_size. Set content_extent directly.
        debug_assert!(
            content_size.width.is_finite() && content_size.height.is_finite(),
            "Child returned INFINITY content_size; non-virtualizing children with finite \
             sub-widgets should return finite content_size even with unbounded constraints"
        );
        self.state.set_content_extent(content_size);

        self.state.clamp_offset_on_resize();
        if let Some(retained_x) = retained_horizontal_offset {
            self.state.set_offset_x(retained_x);
        }
        if let Some(retained_y) = retained_vertical_offset {
            self.state.set_offset_y(retained_y);
        }

        // Sync geometry to smooth container for frame-driven visual smoothing.
        // smooth mirrors state extents so update_visual() clamps correctly.
        // INV-ROOT-INSET-SCROLLABLE: Use same expanded extents as state.
        self.smooth.set_viewport_size(*domain_viewport);
        self.smooth.set_content_size(content_size);

        self.smooth.clamp_offsets_on_resize();
        if retained_horizontal_offset.is_some() {
            self.smooth.jump_to_offset_x(self.state.logical_offset.x);
            self.pending_horizontal_restore = false;
        }
        if retained_vertical_offset.is_some() {
            self.smooth.jump_to_offset_y(self.state.logical_offset.y);
            self.pending_vertical_restore = false;
        }

        // SPEC-HAL-352: Preserve the effective offset while true bounds
        // absorb or release active tail slack.
        self.reconcile_active_layout_anchor();

        // Sync target offset: state.logical_offset is authoritative (input target).
        let offset = self.state.logical_offset;
        self.smooth.set_target_offset((offset.x, offset.y));
        self.publish_vertical_scroll_position();
        self.smooth.visual_offset().1
    }

    fn sync_smooth_geometry_for_layout_anchor(&mut self) {
        self.smooth.set_viewport_size(self.state.viewport_extent);
        self.smooth.set_content_size(self.state.content_extent);
    }

    fn record_final_layout_anchor_container_size(&mut self, container_size: Size) {
        let previous = self.layout_anchor_final_container_size;
        let resized = previous.width > 0.0
            && previous.height > 0.0
            && ((previous.width - container_size.width).abs() > 0.01
                || (previous.height - container_size.height).abs() > 0.01);
        self.layout_anchor_final_container_size = container_size;
        if resized {
            self.clear_layout_anchor();
        }
    }

    fn reconcile_active_layout_anchor(&mut self) {
        let Some(mut active) = self.active_layout_anchor else {
            return;
        };

        if !active.preserved_effective_y.is_finite() {
            self.active_layout_anchor = None;
            return;
        }

        active.preserved_effective_y = active.preserved_effective_y.max(0.0);
        let base_y = active.preserved_effective_y.min(self.state.max_scroll().y);
        self.state.set_offset_y(base_y);
        self.smooth.jump_to_offset_y(base_y);
        active.tail_slack_y = (active.preserved_effective_y - base_y).max(0.0);

        self.active_layout_anchor = if active.tail_slack_y > 0.01 {
            Some(active)
        } else {
            None
        };
    }

    /// SPEC-HAL-352: Resolve one transaction from completed post-layout geometry.
    pub(crate) fn resolve_layout_anchor_transaction(
        &mut self,
        resolution: LayoutScrollAnchorResolution,
    ) {
        let Some(transaction) = self.pending_layout_anchor.take() else {
            return;
        };

        let size_matches = (transaction.scrollport_size.width
            - self.layout_anchor_final_container_size.width)
            .abs()
            <= 0.01
            && (transaction.scrollport_size.height
                - self.layout_anchor_final_container_size.height)
                .abs()
                <= 0.01;
        let finite = resolution.anchor_content_y.is_finite()
            && resolution.affected_region_bottom_y.is_finite()
            && self.state.viewport_extent.height.is_finite();
        if !self.layout_anchor_metrics_finalized || !size_matches || !finite {
            self.active_layout_anchor = None;
            self.state.clamp_offset_on_resize();
            self.smooth.clamp_offsets_on_resize();
            return;
        }

        let preserve_y = resolution.anchor_content_y - transaction.pre_anchor_viewport_y;
        let desired_y = match transaction.intent {
            LayoutScrollAnchorIntent::CollapsePreserveHeader => preserve_y.max(0.0),
            LayoutScrollAnchorIntent::ExpandRevealRegion => {
                let reveal_bottom_y =
                    resolution.affected_region_bottom_y - self.state.viewport_extent.height;
                preserve_y
                    .max(reveal_bottom_y)
                    .min(resolution.anchor_content_y)
                    .max(0.0)
            }
        };
        let base_y = desired_y.min(self.state.max_scroll().y);
        self.state.set_offset_y(base_y);
        self.smooth.jump_to_offset_y(base_y);

        let tail_slack_y = match transaction.intent {
            LayoutScrollAnchorIntent::CollapsePreserveHeader => (desired_y - base_y).max(0.0),
            LayoutScrollAnchorIntent::ExpandRevealRegion => 0.0,
        };
        self.active_layout_anchor = if tail_slack_y > 0.01 {
            Some(ActiveLayoutAnchorSlack {
                preserved_effective_y: desired_y,
                tail_slack_y,
            })
        } else {
            None
        };
    }

    #[cfg(test)]
    pub(crate) fn test_set_final_layout_anchor_metrics(
        &mut self,
        container_size: Size,
        viewport_extent: Size,
        content_extent: Size,
    ) {
        self.container_size = container_size;
        self.record_final_layout_anchor_container_size(container_size);
        self.state.set_viewport_extent(viewport_extent);
        self.state.set_content_extent(content_extent);
        self.state.clamp_offset_on_resize();
        self.sync_smooth_geometry_for_layout_anchor();
        self.smooth.clamp_offsets_on_resize();
        self.layout_anchor_metrics_finalized = true;
        self.reconcile_active_layout_anchor();
    }

    #[cfg(test)]
    pub(crate) fn test_tail_slack_y(&self) -> f32 {
        self.active_layout_anchor
            .map_or(0.0, |anchor| anchor.tail_slack_y)
    }

    #[cfg(test)]
    pub(crate) fn test_max_scroll_y(&self) -> f32 {
        self.state.max_scroll().y
    }

    #[cfg(test)]
    pub(crate) fn test_has_pending_layout_anchor(&self) -> bool {
        self.pending_layout_anchor.is_some()
    }

    #[cfg(test)]
    pub(crate) fn test_has_ordinary_anchor(&self) -> bool {
        self.state.anchor.is_some()
    }

    /// Set fixed viewport size (width, height)
    /// ScrollArea will always be this size, regardless of parent constraints
    pub fn with_viewport_size(mut self, width: f32, height: f32) -> Self {
        self.configured_viewport = Some(Size::new(width, height));
        self
    }

    pub fn with_scrollbars(mut self, show: bool) -> Self {
        self.show_scrollbars = show;
        self
    }

    /// Select scrollbar reveal behavior for this instance before mounting.
    pub fn with_scrollbar_reveal_policy(mut self, policy: ScrollAreaScrollbarRevealPolicy) -> Self {
        if self.scrollbar_reveal_policy != policy {
            self.scrollbar_reveal_policy = policy;
            (self.scrollbar_hide_timer, self.scrollbar_fade_alpha) =
                if self.auto_hides_scrollbars() {
                    (0.0, 0.0)
                } else {
                    (OVERLAY_SCROLLBAR_HIDE_DELAY_SECS, 1.0)
                };
        }
        self
    }

    pub(crate) fn auto_hides_scrollbars(&self) -> bool {
        self.scrollbar_reveal_policy
            == ScrollAreaScrollbarRevealPolicy::AutoHideOnScrollOrRailProximity
    }

    /// SPEC-HAL-086 Stage 8: Enable vertical scrollbar
    pub fn enable_vertical_scrollbar(mut self, enable: bool) -> Self {
        self.enable_scrollbar_v = enable;
        self
    }

    /// SPEC-HAL-086 Stage 8: Enable horizontal scrollbar
    pub fn enable_horizontal_scrollbar(mut self, enable: bool) -> Self {
        self.enable_scrollbar_h = enable;
        self
    }

    /// Set a horizontal scroll link for external alignment.
    ///
    /// SPEC-HAL-086 Stage N: When set, ScrollArea writes its visual horizontal
    /// offset through the engine's projection publisher. Header/footer widgets
    /// read the link to apply the same X translation. This attachment is live-only
    /// and replaces any pending retained-X attachment without affecting Y.
    ///
    /// INV-HSCROLL-LINK-1: ScrollArea is the ONLY writer.
    pub fn with_horizontal_scroll_link(mut self, link: HorizontalScrollLink) -> Self {
        self.set_horizontal_scroll_link(link);
        self
    }

    /// Set horizontal scroll link (mutable setter).
    ///
    /// SPEC-HAL-086 Stage N: For post-construction wiring.
    pub fn set_horizontal_scroll_link(&mut self, link: HorizontalScrollLink) {
        self.horizontal_scroll_link = Some(link);
        self.pending_horizontal_restore = false;
    }

    /// Retain presented X across sequential `ScrollArea` reconstruction.
    ///
    /// SPEC-HAL-366: The first tight layout clamps and settles X before input
    /// projections/paint. Pending Y and independent Y convergence are preserved.
    /// Destroy the previous writer before mounting its replacement. Newer
    /// accepted scroll intent cancels continuity; this method does not run layout.
    pub fn with_retained_horizontal_scroll_link(mut self, link: HorizontalScrollLink) -> Self {
        self.set_retained_horizontal_scroll_link(link);
        self
    }

    /// Attach retained X after construction; schedule layout through the usual owner.
    pub fn set_retained_horizontal_scroll_link(&mut self, link: HorizontalScrollLink) {
        self.state.clear_anchor();
        self.clear_layout_anchor();
        self.horizontal_scroll_link = Some(link);
        self.pending_horizontal_restore = true;
    }

    /// Retain the presented vertical position across `ScrollArea` reconstruction.
    ///
    /// The replacement consumes the retained coordinate once its first tight
    /// layout establishes final content and viewport extents. Restoration sets
    /// logical and visual offsets together before first paint. Any explicit
    /// scroll intent received before that layout cancels the pending restore.
    ///
    /// This is a sequential-lifetime continuity primitive, not a live linked
    /// scrolling mechanism. Use a scroll group to coordinate simultaneously
    /// mounted scroll areas.
    pub fn with_vertical_scroll_link(mut self, link: VerticalScrollLink) -> Self {
        self.state.clear_anchor();
        self.clear_layout_anchor();
        self.vertical_scroll_link = Some(link);
        self.pending_vertical_restore = true;
        self
    }

    /// Set retained vertical continuity after construction.
    pub fn set_vertical_scroll_link(&mut self, link: VerticalScrollLink) {
        self.state.clear_anchor();
        self.clear_layout_anchor();
        self.vertical_scroll_link = Some(link);
        self.pending_vertical_restore = true;
    }

    /// Set horizontal freeze flag for external scroll freezing.
    ///
    /// INV-RESIZE-HSCROLL-2: When true, delta_x is zeroed in scroll events.
    /// Used by ListView to freeze horizontal scroll during column resize.
    pub fn with_horizontal_freeze(mut self, flag: Rc<Cell<bool>>) -> Self {
        self.horizontal_freeze = Some(flag);
        self
    }

    /// Get the current horizontal scroll visual offset.
    ///
    /// Returns the X component of the visual scroll offset.
    pub fn horizontal_scroll_offset(&self) -> f32 {
        self.smooth.visual_offset().0
    }

    /// Store the content widget's separator centerlines in the compatibility cache.
    ///
    /// Post-layout synchronization supplies these X coordinates in content space.
    pub fn set_separator_hit_bands(&mut self, bands: Vec<f32>) {
        self.cached_separator_bands = bands;
    }

    /// Check whether a ScrollArea-local X falls within a cached separator band.
    ///
    /// The query subtracts the content offset and adds the visual horizontal
    /// scroll offset to compare against the centerlines in content coordinates.
    #[allow(
        dead_code,
        reason = "Retained legacy separator-band geometry query; not part of pointer routing"
    )]
    fn is_in_separator_hit_band(&self, local_x: f32) -> bool {
        // Convert from ScrollArea-local to geometry-origin.
        // Bands are in geometry-origin space (body-local). Subtract content_offset
        // to remove the child positioning offset, then add scroll to get content coords.
        let content_x = local_x - self.content_offset + self.horizontal_scroll_offset();

        self.cached_separator_bands
            .iter()
            .any(|&sep_x| (content_x - sep_x).abs() <= SEPARATOR_HIT_HALF_WIDTH)
    }

    // =========================================================================
    // SPEC-HAL-133: Linked Scrolling API
    // =========================================================================

    /// Set the scroll group for linked scrolling.
    ///
    /// SPEC-HAL-133: Multiple ScrollArea instances with the same ScrollGroupId
    /// will have their scroll offsets synchronized.
    ///
    /// INV-LINK-MODE-OFFSET-1: Each member's offset is clamped to its own bounds.
    /// INV-LINK-MODE-ANCHOR-1: Anchors remain per-ScrollArea (not shared).
    pub fn with_scroll_group(mut self, group: ScrollGroupId) -> Self {
        self.scroll_group = Some(group);
        self
    }

    /// Set the scroll group (mutable setter).
    ///
    /// SPEC-HAL-133: For dynamic group assignment.
    pub fn set_scroll_group(&mut self, group: Option<ScrollGroupId>) {
        self.scroll_group = group;
    }

    /// Get the scroll group.
    ///
    /// SPEC-HAL-133: Returns None if not part of any group.
    pub fn scroll_group(&self) -> Option<ScrollGroupId> {
        self.scroll_group
    }

    /// Receive linked scroll offset from group coordinator.
    ///
    /// SPEC-HAL-133: Called by scroll link coordinator during propagation.
    /// Does NOT trigger re-propagation (loop prevention via frame token).
    ///
    /// INV-LINK-LOOP-1: Uses frame token to prevent cycles.
    /// INV-LINK-MODE-OFFSET-1: Offset clamped to this surface's bounds.
    /// INV-LINK-MODE-ANCHOR-1: Anchor remains unchanged.
    ///
    /// Returns true if offset changed.
    pub fn receive_linked_offset(&mut self, offset: Point, frame_token: u64) -> bool {
        // INV-LINK-LOOP-1: Skip if already processed this frame
        if self.link_frame_token == frame_token
            || self.scroll_intent_admission.blocks_competing_scroll()
        {
            return false;
        }
        self.link_frame_token = frame_token;
        self.retire_layout_anchor_for_scroll_intent();
        let changed = self.state.receive_linked_offset(offset);
        if changed {
            // Sync target for frame-driven convergence
            let new_offset = self.state.logical_offset;
            self.smooth.set_target_offset((new_offset.x, new_offset.y));
            self.publish_vertical_scroll_position();
        }
        changed
    }

    /// Mark this ScrollArea as the leader for the current frame.
    ///
    /// SPEC-HAL-133: Called when this ScrollArea initiates a scroll.
    /// The frame token prevents this ScrollArea from receiving its own
    /// propagated offset in the same frame.
    pub fn mark_as_link_leader(&mut self, frame_token: u64) {
        self.link_frame_token = frame_token;
    }

    /// Get the current link frame token.
    ///
    /// SPEC-HAL-133: Used by coordinator to identify leaders.
    pub fn link_frame_token(&self) -> u64 {
        self.link_frame_token
    }

    /// Set the content size (total scrollable area).
    ///
    /// SPEC-HAL-114 Stage 9: For virtualized lists, content_size must be set
    /// to the full logical size (item_count * item_height), not just visible items.
    pub fn with_content_size(mut self, width: f32, height: f32) -> Self {
        self.state.set_content_extent(Size::new(width, height));
        self
    }

    /// Set viewport size (test-only).
    ///
    /// SPEC-HAL-131A: Required for tests that configure ScrollArea after construction.
    ///
    /// SPEC-HAL-158B INV-DIRTY-SCROLL-1: This is a test-only API.
    /// Production code MUST NOT call this method.
    /// This method does NOT propagate dirty to the widget tree.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn set_viewport_size(&mut self, size: Size) {
        self.state.set_viewport_extent(size);
    }

    /// Set content size (test-only).
    ///
    /// SPEC-HAL-131A: Required for tests that configure ScrollArea after construction.
    ///
    /// SPEC-HAL-158B INV-DIRTY-SCROLL-1: This is a test-only API.
    /// Production code MUST NOT call this method.
    /// This method does NOT propagate dirty to the widget tree.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn set_content_size(&mut self, size: Size) {
        self.state.set_content_extent(size);
    }

    /// Extend content height by delta (downward growth only).
    ///
    /// INV-SCROLL-EXTEND-1: delta MUST be >= 0 (bottom extension only).
    /// INV-SCROLL-EXTEND-2: Viewport position unchanged.
    /// INV-SCROLL-EXTEND-3: No anchor mutation, no visible range recompute.
    ///
    /// Use case: Virtualized infinite feeds where new content appears at bottom.
    /// Forwards directly to ScrollState without reinterpretation.
    pub fn extend_content_height(&mut self, delta: f32) {
        self.state.extend_content_height(delta);
        if self.active_layout_anchor.is_some() {
            self.sync_smooth_geometry_for_layout_anchor();
            self.reconcile_active_layout_anchor();
        }
    }

    /// Prepend content height by delta (upward growth with offset compensation).
    ///
    /// INV-SCROLL-PREPEND-1: delta MUST be >= 0 (top extension only).
    /// INV-SCROLL-PREPEND-2: Viewport stability - offset increased by delta.
    /// INV-SCROLL-PREPEND-3: No anchor mutation.
    ///
    /// Use case: Chat history loading, infinite scroll upward.
    /// Forwards directly to ScrollState without reinterpretation.
    pub fn prepend_content_height(&mut self, delta: f32) {
        let delta = delta.max(0.0);
        if let Some(active) = &mut self.active_layout_anchor {
            active.preserved_effective_y += delta;
        }
        self.state.prepend_content_height(delta);
        if self.active_layout_anchor.is_some() {
            self.sync_smooth_geometry_for_layout_anchor();
            self.reconcile_active_layout_anchor();
        }
    }

    /// Trim content from the top by delta (remove content above origin).
    ///
    /// INV-SCROLL-TRIM-1: delta MUST be >= 0.
    /// INV-SCROLL-TRIM-2: Offset decreased by delta (clamped to 0).
    /// INV-SCROLL-TRIM-4: Anchor not mutated (caller handles invalidation).
    ///
    /// Use case: Discarding old chat history above viewport.
    /// Forwards directly to ScrollState without reinterpretation.
    pub fn trim_content_top(&mut self, delta: f32) {
        let applied = delta.max(0.0).min(self.state.content_extent.height);
        if let Some(active) = &mut self.active_layout_anchor {
            active.preserved_effective_y = (active.preserved_effective_y - applied).max(0.0);
        }
        self.state.trim_content_top(delta);
        if self.active_layout_anchor.is_some() {
            self.sync_smooth_geometry_for_layout_anchor();
            self.reconcile_active_layout_anchor();
        }
    }

    /// Trim content from the bottom by delta (remove content below end).
    ///
    /// INV-SCROLL-TRIM-1: delta MUST be >= 0.
    /// INV-SCROLL-TRIM-2: Offset unchanged unless clamped by new max.
    /// INV-SCROLL-TRIM-4: Anchor not mutated (caller handles invalidation).
    ///
    /// Use case: Discarding future content below viewport.
    /// Forwards directly to ScrollState without reinterpretation.
    pub fn trim_content_bottom(&mut self, delta: f32) {
        self.state.trim_content_bottom(delta);
        if self.active_layout_anchor.is_some() {
            self.sync_smooth_geometry_for_layout_anchor();
            self.reconcile_active_layout_anchor();
        }
    }

    // =========================================================================
    // SPEC-HAL-114 Stage 8: Programmatic Scroll API
    // =========================================================================

    /// Scroll to a specific vertical offset.
    ///
    /// SPEC-HAL-114 Stage 8: Programmatic scroll for auto-reveal.
    /// - Clamped to [0, max_scroll_y]
    /// - Idempotent (setting same value is no-op)
    ///
    /// Use this to reveal focused items in virtualized lists.
    pub fn scroll_to_y(&mut self, y: f32) {
        if !self.scroll_intent_admission.admit_scroll_intent() {
            return;
        }
        self.cancel_pending_retained_restore();
        let clamped = y.clamp(0.0, self.state.max_scroll().y);
        if (clamped - self.state.logical_offset.y).abs() > 0.001 {
            self.retire_layout_anchor_for_scroll_intent();
        }
        self.state.set_offset_y(y);
        // Sync target for frame-driven convergence
        let offset = self.state.logical_offset;
        self.smooth.set_target_offset((offset.x, offset.y));
        self.publish_vertical_scroll_position();
    }

    /// Scroll to specific X and Y offsets. Both axes clamped to [0, max_scroll].
    /// Use when resetting both axes (e.g., return to Live mode).
    pub fn scroll_to(&mut self, x: f32, y: f32) {
        if !self.scroll_intent_admission.admit_scroll_intent() {
            return;
        }
        self.cancel_pending_retained_restore();
        let max = self.state.max_scroll();
        let clamped = Point::new(x.clamp(0.0, max.x), y.clamp(0.0, max.y));
        if (clamped.x - self.state.logical_offset.x).abs() > 0.001
            || (clamped.y - self.state.logical_offset.y).abs() > 0.001
        {
            self.retire_layout_anchor_for_scroll_intent();
        }
        self.state.set_offset(Point::new(x, y));
        let offset = self.state.logical_offset;
        self.smooth.set_target_offset((offset.x, offset.y));
        self.smooth.jump_to_offset((offset.x, offset.y));
        self.publish_vertical_scroll_position();
    }

    /// Jump scroll to Y instantly — sets BOTH target and visual.
    /// No smooth animation. Use for auto-follow where entries should
    /// "pop" into view rather than slide.
    pub fn jump_to_y(&mut self, y: f32) {
        if !self.scroll_intent_admission.admit_scroll_intent() {
            return;
        }
        self.cancel_pending_retained_restore();
        let clamped = y.clamp(0.0, self.state.max_scroll().y);
        if (clamped - self.state.logical_offset.y).abs() > 0.001 {
            self.retire_layout_anchor_for_scroll_intent();
        }
        self.state.set_offset_y(y);
        let offset = self.state.logical_offset;
        self.smooth.jump_to_offset((offset.x, offset.y));
        self.publish_vertical_scroll_position();
    }

    /// Get the current target scroll X offset (logical).
    pub fn target_scroll_x(&self) -> f32 {
        self.state.logical_offset.x
    }

    /// Get the current target scroll Y offset.
    ///
    /// SPEC-HAL-114 Stage 8: For integration tests.
    pub fn target_scroll_y(&self) -> f32 {
        self.pending_retained_vertical_offset()
            .unwrap_or(self.state.logical_offset.y)
    }

    /// Get the current visual scroll Y offset.
    ///
    /// Returns the smoothed visual position (converges toward target each frame).
    pub fn visual_scroll_y(&self) -> f32 {
        self.pending_retained_vertical_offset()
            .unwrap_or_else(|| self.effective_visual_scroll_y())
    }

    /// Current content extent height (read-only).
    ///
    /// Used by content widgets to reconcile their reported content extent
    /// against the live ScrollState without forcing a relayout.
    pub fn content_extent_height(&self) -> f32 {
        self.state.content_extent.height
    }

    // =========================================================================
    // Scroll Intent Resolution
    // =========================================================================

    /// Resolve a scroll intent to reveal a rect in content coordinates.
    ///
    /// INV-SCROLL-INTENT-1: All viewport math lives here, not in widgets.
    /// INV-SCROLL-INTENT-2: Widgets never mutate scroll position directly.
    ///
    /// # Arguments
    /// * `rect` - The rect to reveal, in content coordinates
    /// * `align` - How to position the rect within the viewport
    ///
    /// # Alignment Behavior
    /// * `Nearest` - Minimum scroll to make rect fully visible
    /// * `Start` - Align rect.top to viewport.top
    /// * `End` - Align rect.bottom to viewport.bottom
    /// * `Center` - Center rect vertically in viewport
    pub fn resolve_reveal_rect(
        &mut self,
        rect: crate::core::math::RectF,
        align: crate::core::event::ScrollAlign,
    ) {
        let Some(clamped_scroll) = self.reveal_scroll_target(rect, align) else {
            return;
        };
        self.retire_layout_anchor_for_scroll_intent();
        self.state.set_offset_y(clamped_scroll);
        let offset = self.state.logical_offset;
        self.smooth.set_target_offset((offset.x, offset.y));
        self.publish_vertical_scroll_position();
    }

    pub(crate) fn has_geometry_scroll_effect(&self) -> bool {
        self.pending_horizontal_restore
            || self.pending_vertical_restore
            || self.pending_layout_anchor.is_some()
            || self.is_anchor_dirty()
            || self.smooth.visual_offset() != self.smooth.target_offset()
    }

    pub(crate) fn reveal_would_change_geometry(
        &self,
        rect: crate::core::math::RectF,
        align: crate::core::event::ScrollAlign,
    ) -> bool {
        self.reveal_scroll_target(rect, align)
            .is_some_and(|target| target != self.effective_visual_scroll_y())
    }

    fn reveal_scroll_target(
        &self,
        rect: crate::core::math::RectF,
        align: crate::core::event::ScrollAlign,
    ) -> Option<f32> {
        if self.scroll_intent_admission.blocks_competing_scroll() {
            return None;
        }
        use crate::core::event::ScrollAlign;

        // FIX A1: viewport_extent is container_size, but the VISIBLE viewport is
        // container_size - 2*root_inset. Without this adjustment, reveal thinks
        // the viewport is larger than it actually is, causing the last visible
        // item to appear "in viewport" when it's actually clipped by root_inset.
        let visible_viewport_height =
            (self.state.viewport_extent.height - 2.0 * self.content_offset).max(0.0);
        let content_height = self.state.content_extent.height;
        let current_scroll = self.effective_visual_scroll_y();

        // Edge case: viewport can't show content
        if visible_viewport_height <= 0.0 || content_height <= 0.0 {
            return None;
        }

        // INV-REVEAL-FIT: a target at least as tall as the visible viewport cannot
        // be brought into view by scrolling — every alignment just pins the scroll
        // to an edge. This fires when focus lands on a viewport-sized scroll-content
        // host (a List/TreeView that fills its ScrollArea and owns its internal
        // scroll); revealing it would yank the user's scroll position to content
        // top. Preserve the current scroll. (−0.5 absorbs sub-pixel viewport rounding.)
        if rect.height >= visible_viewport_height - 0.5 {
            return None;
        }

        // Compute target scroll based on alignment
        let target_scroll = match align {
            ScrollAlign::Nearest => {
                // Reveal margin: breathing room so the focused widget doesn't
                // sit flush against the viewport edge after auto-scroll.
                let reveal_margin = 4.0;

                let view_top = current_scroll;
                let view_bottom = current_scroll + visible_viewport_height;

                if rect.y < view_top {
                    // Rect is above viewport: align top with margin
                    (rect.y - reveal_margin).max(0.0)
                } else if rect.y + rect.height > view_bottom {
                    // Rect is below viewport: align bottom with margin
                    rect.y + rect.height - visible_viewport_height + reveal_margin
                } else {
                    // Already visible: no change
                    return None;
                }
            }
            ScrollAlign::Start => {
                // Align rect.top to viewport.top
                rect.y
            }
            ScrollAlign::End => {
                // Align rect.bottom to viewport.bottom
                rect.y + rect.height - visible_viewport_height
            }
            ScrollAlign::Center => {
                // Center rect in viewport
                rect.y + (rect.height - visible_viewport_height) / 2.0
            }
        };

        // Clamp to valid scroll range and apply
        // NOTE: max_scroll uses the scroll domain formula (content_extent - viewport_extent)
        // which is different from visible_viewport_height. This is correct because
        // max_scroll defines the scroll RANGE, while visible_viewport_height defines
        // what's actually VISIBLE for reveal purposes.
        let max_scroll = self.state.max_scroll().y;
        let clamped_scroll = target_scroll.clamp(0.0, max_scroll);
        Some(clamped_scroll)
    }

    // =========================================================================
    // SPEC-HAL-131A Stage 9: Anchor Creation Contract (Write Path Lock-In)
    // =========================================================================
    //
    // Centralized anchor creation API. Anchors are created only via intent-named
    // APIs, not via direct ScrollAnchor construction at widget call sites.
    //
    // INV-ANCHOR-CREATE-1: Anchors created only via intent-named APIs.
    //   - Use `set_anchor_at_viewport_y` to create anchors.
    //   - Calling twice replaces the existing anchor (last write wins).
    // =========================================================================

    /// Create an anchor at the specified viewport Y position.
    ///
    /// SPEC-HAL-131A Stage 9: Canonical anchor creation API.
    ///
    /// - `target_id`: Stable identifier for the anchor target (e.g., item index)
    /// - `viewport_y`: Y position within viewport where target should appear
    ///
    /// The anchor ensures the target remains at `viewport_y` after content
    /// mutations (insert, delete, resize). Resolve with `jump_to_anchor_and_keep`
    /// or `jump_to_anchor_and_clear`.
    ///
    /// INV-ANCHOR-CREATE-1: Calling twice replaces the existing anchor.
    pub fn set_anchor_at_viewport_y(&mut self, target_id: u64, viewport_y: f32) {
        if self.scroll_intent_admission.blocks_competing_scroll() {
            return;
        }
        self.retire_layout_anchor_for_scroll_intent();
        let anchor = ScrollAnchor::new(
            AnchorId::new(target_id),
            Point::new(0.0, 0.0),
            Point::new(0.0, viewport_y),
        );
        self.state.set_anchor(anchor);
    }

    /// Set anchor to a specific row for visual stability.
    ///
    /// SPEC-HAL-131A Stage 9: Deprecated. Use `set_anchor_at_viewport_y` instead.
    #[doc(hidden)]
    pub fn set_anchor_row(&mut self, row_id: u64, viewport_offset_y: f32) {
        self.set_anchor_at_viewport_y(row_id, viewport_offset_y);
    }

    /// Set anchor from linked scroll propagation.
    ///
    /// SPEC-HAL-133 Phase 2: Called by the scroll link coordinator when propagating
    /// anchors to linked ScrollArea instances.
    ///
    /// INV-LINK-ANCHOR-2: If this ScrollArea's content doesn't contain
    /// the anchor target, resolution will no-op (handled by resolver).
    /// No fallback, no inference.
    ///
    /// INV-LINK-CONTRACT-1: Requires scroll_group to be set.
    /// INV-LINK-CONTRACT-2: Requires axis to match ScrollArea's supported axis.
    ///
    /// This sets the anchor and marks it dirty so it will be resolved
    /// in the post-layout anchor resolution phase.
    ///
    /// # Axis Filtering
    ///
    /// SPEC-HAL-133 Phase 2: Anchor components are filtered by axis:
    /// - Vertical intent: only vertical components used
    /// - Horizontal intent: only horizontal components used
    /// - Both: requires dual-axis support
    pub fn set_linked_anchor(&mut self, anchor: ScrollAnchor, intent_axis: ScrollAxis) {
        if self.scroll_intent_admission.blocks_competing_scroll() {
            return;
        }
        // INV-LINK-CONTRACT-1: Reject if no scroll group
        if self.scroll_group.is_none() {
            debug_assert!(
                false,
                "SPEC-HAL-133 Phase 2: set_linked_anchor called on ScrollArea without scroll_group"
            );
            return;
        }

        // INV-LINK-CONTRACT-2: Check axis compatibility
        let supported_axis = self.supported_scroll_axis();
        let axis_compatible = match intent_axis {
            ScrollAxis::Vertical => {
                matches!(supported_axis, ScrollAxis::Vertical | ScrollAxis::Both)
            }
            ScrollAxis::Horizontal => {
                matches!(supported_axis, ScrollAxis::Horizontal | ScrollAxis::Both)
            }
            ScrollAxis::Both => matches!(supported_axis, ScrollAxis::Both),
        };

        if !axis_compatible {
            debug_assert!(
                false,
                "SPEC-HAL-133 Phase 2: set_linked_anchor axis mismatch: intent={:?}, supported={:?}",
                intent_axis, supported_axis
            );
            return;
        }

        self.retire_layout_anchor_for_scroll_intent();

        // SPEC-HAL-133 Phase 2: Filter anchor components by axis
        let filtered_anchor = match intent_axis {
            ScrollAxis::Vertical => ScrollAnchor::new(
                anchor.target_id,
                Point::new(0.0, anchor.target_offset.y),
                Point::new(0.0, anchor.viewport_offset.y),
            ),
            ScrollAxis::Horizontal => ScrollAnchor::new(
                anchor.target_id,
                Point::new(anchor.target_offset.x, 0.0),
                Point::new(anchor.viewport_offset.x, 0.0),
            ),
            ScrollAxis::Both => anchor,
        };

        // SPEC-HAL-133 Phase 2 INV: Linked anchors are indistinguishable from user anchors.
        // We use the same set_anchor() path - resolver does not know origin.
        self.state.set_anchor(filtered_anchor);
        self.state.mark_anchor_dirty();
    }

    /// Get the scroll axis this ScrollArea supports.
    ///
    /// SPEC-HAL-133 Phase 2: Used for axis compatibility checking.
    ///
    /// Returns:
    /// - Vertical: only vertical scrolling enabled
    /// - Horizontal: only horizontal scrolling enabled
    /// - Both: both axes enabled
    pub fn supported_scroll_axis(&self) -> ScrollAxis {
        match (self.enable_scrollbar_v, self.enable_scrollbar_h) {
            (true, true) => ScrollAxis::Both,
            (true, false) => ScrollAxis::Vertical,
            (false, true) => ScrollAxis::Horizontal,
            (false, false) => ScrollAxis::Vertical, // Default to vertical
        }
    }

    /// Clear anchor, keeping current logical_offset.
    ///
    /// SPEC-HAL-131A: Switches to absolute mode.
    pub fn clear_anchor(&mut self) {
        self.state.clear_anchor();
    }

    /// Check if anchor is currently set.
    pub fn has_anchor(&self) -> bool {
        self.state.anchor.is_some()
    }

    /// Jump to anchor position (explicit, single-shot).
    ///
    /// SPEC-HAL-131A Stage 6: Pure forwarder to ScrollState::jump_to_anchor.
    ///
    /// Returns true if jump occurred, false if no anchor or resolution failed.
    /// See ScrollState::jump_to_anchor for invariants.
    pub fn jump_to_anchor(&mut self, resolver: &dyn AnchorResolver) -> bool {
        self.state.jump_to_anchor(resolver)
    }

    /// Resolve anchor, jump, and optionally clear anchor.
    ///
    /// SPEC-HAL-131A Stage 7: Pure forwarder to ScrollState::resolve_and_jump_anchor.
    ///
    /// Returns true if jump occurred, false if no anchor or resolution failed.
    /// See ScrollState::resolve_and_jump_anchor for invariants.
    ///
    /// SPEC-HAL-131A Stage 8: Advanced/internal API. Prefer `jump_to_anchor_and_keep`
    /// or `jump_to_anchor_and_clear` for explicit ownership intent.
    #[doc(hidden)]
    pub fn resolve_and_jump_anchor(
        &mut self,
        resolver: &dyn AnchorResolver,
        clear_after: bool,
    ) -> bool {
        self.state.resolve_and_jump_anchor(resolver, clear_after)
    }

    // =========================================================================
    // SPEC-HAL-131A Stage 8: Anchor Ownership & Call-Site Enforcement
    // =========================================================================
    //
    // Intent-named wrappers that make anchor lifecycle ownership explicit.
    //
    // INV-ANCHOR-OWN-1: Call sites must choose ownership explicitly.
    //   - No boolean flags exposed to UI / widget code paths.
    //   - Use `jump_to_anchor_and_keep` to preserve anchor after jump.
    //   - Use `jump_to_anchor_and_clear` to clear anchor after jump.
    //
    // SPEC-HAL-131A Stage 10: These are now internal APIs. Use
    // `resolve_anchor_if_needed` for scheduled anchor resolution.
    // =========================================================================

    /// Jump to anchor position and preserve the anchor.
    ///
    /// SPEC-HAL-131A Stage 8: Intent-named wrapper for anchor resolution with
    /// explicit ownership. Anchor remains set after jump for subsequent
    /// resolution cycles.
    ///
    /// SPEC-HAL-131A Stage 10: Internal API. Use `resolve_anchor_if_needed`
    /// for scheduled anchor resolution in the frame pipeline.
    ///
    /// Returns true if jump occurred, false if no anchor or resolution failed.
    /// INV-ANCHOR-OWN-1: Explicit ownership - caller chose "keep".
    #[doc(hidden)]
    pub fn jump_to_anchor_and_keep(&mut self, resolver: &dyn AnchorResolver) -> bool {
        self.resolve_and_jump_anchor(resolver, false)
    }

    /// Jump to anchor position and clear the anchor.
    ///
    /// SPEC-HAL-131A Stage 8: Intent-named wrapper for anchor resolution with
    /// explicit ownership. Anchor is cleared after jump (single-shot usage).
    ///
    /// SPEC-HAL-131A Stage 10: Internal API. Use `resolve_anchor_if_needed`
    /// for scheduled anchor resolution in the frame pipeline.
    ///
    /// Returns true if jump occurred, false if no anchor or resolution failed.
    /// INV-ANCHOR-OWN-1: Explicit ownership - caller chose "clear".
    #[doc(hidden)]
    pub fn jump_to_anchor_and_clear(&mut self, resolver: &dyn AnchorResolver) -> bool {
        self.resolve_and_jump_anchor(resolver, true)
    }

    // =========================================================================
    // SPEC-HAL-131A Stage 10: Anchor Resolution Scheduling (Read-Path Lock-In)
    // SPEC-HAL-131A Stage 11: Anchor Dirtying & Re-arm Semantics
    // =========================================================================
    //
    // Single resolution hook for the frame pipeline. Anchors may only be
    // resolved at this explicit point, ensuring predictable timing.
    //
    // INV-ANCHOR-RESOLVE-SCHED-1: Anchor resolution triggered only via
    //   resolve_anchor_if_needed. Direct calls to jump_to_anchor_* are internal.
    //
    // INV-ANCHOR-RESOLVE-SCHED-2: Anchors are resolved at most once per dirtied cycle.
    //   - Resolution is skipped if anchor is clean.
    //   - After successful resolution, anchor_dirty = false, anchor remains.
    //
    // Intended call site: post-layout, pre-paint (wiring deferred to later stage).
    // =========================================================================

    /// Resolve anchor if one is pending and dirty.
    ///
    /// SPEC-HAL-131A Stage 10/11: Single resolution hook for the frame pipeline.
    ///
    /// If an anchor is set AND dirty, resolves it and adjusts scroll offset.
    /// After resolution, anchor remains set but is marked clean.
    ///
    /// INV-ANCHOR-RESOLVE-SCHED-1: This is the only allowed resolution trigger.
    /// INV-ANCHOR-RESOLVE-SCHED-2: State-driven resolution.
    /// - With dirty anchor: performs exactly one jump, then clears dirty flag
    /// - With clean anchor: no-op, resolver not called
    /// - Without anchor: no-op, resolver not called
    /// - Multiple calls: idempotent (first call resolves and clears dirty, rest no-op)
    pub fn resolve_anchor_if_needed(&mut self, resolver: &dyn AnchorResolver) {
        // INV-ANCHOR-RESOLVE-SCHED-2: Only resolve if dirty
        if !self.state.is_anchor_dirty() {
            return;
        }

        // Resolve and keep anchor (anchor persists, only dirty flag cleared)
        // INV-ANCHOR-RESOLVE-SCHED-2: Single resolution per dirtied cycle
        if self.jump_to_anchor_and_keep(resolver) {
            self.state.clear_anchor_dirty();
        }
    }

    /// Mark anchor as dirty, triggering resolution on next cycle.
    ///
    /// SPEC-HAL-131A Stage 11: Public invalidation hook.
    /// No-op if no anchor is set.
    ///
    /// Use case: Content mutations (insert, delete, resize) that require
    /// anchor re-resolution.
    pub fn mark_anchor_dirty(&mut self) {
        self.state.mark_anchor_dirty();
    }

    /// Check if anchor needs resolution.
    ///
    /// SPEC-HAL-131A Stage 11: Returns true if anchor exists AND is dirty.
    pub fn is_anchor_dirty(&self) -> bool {
        self.state.is_anchor_dirty()
    }

    // =========================================================================
    // SPEC-HAL-131A Stage 12: Anchor Invalidation Sources (Structural Wiring)
    // =========================================================================
    //
    // Explicit invalidation hooks for content mutations. These define the
    // contract surface for anchor dirtying without automatic wiring.
    //
    // INV-ANCHOR-DIRTY-SOURCES-1: Anchor dirtying occurs only via:
    //   - set_anchor_* (new anchor)
    //   - set_content_extent (size change)
    //   - explicit mutation hooks (on_items_*)
    //
    // No hidden dirtying. No automatic integration with List/Layout.
    // Callers explicitly invoke these hooks when content mutates.
    // =========================================================================

    /// Notify that items were inserted into the content.
    ///
    /// SPEC-HAL-131A Stage 12: Explicit invalidation hook for item insertion.
    /// Marks anchor dirty so it will be re-resolved on next cycle.
    ///
    /// No-op if no anchor is set.
    /// Does NOT resolve anchor (resolution happens in resolve_anchor_if_needed).
    /// Does NOT clear anchor.
    ///
    /// Call this when items are added to a virtualized list.
    pub fn on_items_inserted(&mut self) {
        self.state.mark_anchor_dirty();
    }

    /// Notify that items were removed from the content.
    ///
    /// SPEC-HAL-131A Stage 12: Explicit invalidation hook for item removal.
    /// Marks anchor dirty so it will be re-resolved on next cycle.
    ///
    /// No-op if no anchor is set.
    /// Does NOT resolve anchor (resolution happens in resolve_anchor_if_needed).
    /// Does NOT clear anchor.
    ///
    /// Call this when items are removed from a virtualized list.
    pub fn on_items_removed(&mut self) {
        self.state.mark_anchor_dirty();
    }

    /// Notify that items were resized in the content.
    ///
    /// SPEC-HAL-131A Stage 12: Explicit invalidation hook for item resize.
    /// Marks anchor dirty so it will be re-resolved on next cycle.
    ///
    /// No-op if no anchor is set.
    /// Does NOT resolve anchor (resolution happens in resolve_anchor_if_needed).
    /// Does NOT clear anchor.
    ///
    /// Call this when item heights change in a virtualized list.
    pub fn on_items_resized(&mut self) {
        self.state.mark_anchor_dirty();
    }

    // =========================================================================
    // SPEC-HAL-131A Stage 13: Anchor Resolution Placement (Frame Pipeline Wiring)
    // =========================================================================
    //
    // Defines WHERE anchor resolution lives in the frame lifecycle.
    // This is placement definition only - no behavior change.
    //
    // PIPELINE PHASE: post-layout, pre-paint
    //
    // INV-ANCHOR-PIPELINE-1: Anchor resolution is allowed ONLY at this phase.
    //   - NOT during input handling
    //   - NOT during layout
    //   - NOT during paint
    //
    // The single hook `resolve_anchor_if_needed` must be called at this exact
    // pipeline phase. Resolution does NOT occur automatically; the frame loop
    // must explicitly invoke the hook at the declared phase.
    //
    // This stage freezes temporal authority. After this, integration is
    // deterministic: the frame loop calls the hook, the hook resolves if dirty.
    // =========================================================================

    /// Pipeline hook: Resolve anchor at post-layout, pre-paint phase.
    ///
    /// SPEC-HAL-131A Stage 13: This is the declared pipeline placement.
    ///
    /// INV-ANCHOR-PIPELINE-1: This hook is the ONLY allowed call site for
    /// anchor resolution in the frame pipeline. The frame loop must call
    /// this method exactly once per frame, after layout completes but
    /// before paint begins.
    ///
    /// Resolution does NOT occur automatically. This is an explicit hook
    /// that must be invoked by the frame loop at the correct phase.
    ///
    /// # Pipeline Placement
    ///
    /// ```text
    /// Frame Loop:
    ///   1. Input handling      ← anchor resolution NOT allowed
    ///   2. Layout              ← anchor resolution NOT allowed
    ///   3. post_layout_resolve_anchors()  ← HERE (INV-ANCHOR-PIPELINE-1)
    ///   4. Paint               ← anchor resolution NOT allowed
    /// ```
    ///
    /// # Usage
    ///
    /// The frame loop (e.g., App::run_frame) should call this method on each
    /// ScrollArea after layout completes:
    ///
    /// ```ignore
    /// // In frame loop, after layout:
    /// scroll_area.post_layout_resolve_anchors(&resolver);
    /// // Then paint...
    /// ```
    pub fn post_layout_resolve_anchors(&mut self, resolver: &dyn AnchorResolver) {
        // INV-ANCHOR-PIPELINE-1: This is the single pipeline hook.
        // Delegates to resolve_anchor_if_needed, which handles dirty checking.
        self.resolve_anchor_if_needed(resolver);
    }

    /// Get the current logical scroll offset.
    ///
    /// SPEC-HAL-131A: This is the resolved scroll position.
    pub fn logical_offset(&self) -> Point {
        self.state.logical_offset
    }
}

impl Drop for ScrollArea {
    fn drop(&mut self) {
        self.publish_vertical_scroll_position();
    }
}

impl WidgetDowncast for ScrollArea {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl DeferredWiring for ScrollArea {
    fn is_fully_wired(&self) -> bool {
        // SPEC-HAL-319: standalone_empty bypasses the wiring requirement —
        // the layout/paint paths never call content.get() in that mode.
        self.standalone_empty || self.content.is_set()
    }

    fn seal(&mut self) {
        assert!(
            self.is_fully_wired(),
            "ScrollArea::seal called before fully wired (SPEC-HAL-154: wiring incomplete)"
        );
        // SPEC-HAL-319: For standalone_empty, the Deferred is intentionally
        // unset; sealing it would panic.
        if !self.standalone_empty {
            self.content.seal();
        }
        self.sealed = true;
    }

    fn is_sealed(&self) -> bool {
        self.sealed
    }
}

// SPEC-HAL-268: Proximity scroll helpers
impl ScrollArea {
    /// SPEC-HAL-268: Check proximity scroll activation conditions
    fn check_proximity_activation(
        &self,
        modifiers: &crate::core::event::Modifiers,
        ctrl_double_tap: bool,
    ) -> ProximityScrollSource {
        if ctrl_double_tap {
            return ProximityScrollSource::CtrlDoubleTap;
        }
        // Alt+Ctrl (Windows/Linux) or Option+Cmd (macOS)
        if modifiers.alt && (modifiers.ctrl || modifiers.meta) {
            return ProximityScrollSource::ModifierCombo;
        }
        ProximityScrollSource::None
    }

    /// SPEC-HAL-268: Apply positional scroll offset from cursor displacement.
    /// Same ratio as scrollbar thumb drag: content/viewport.
    fn apply_proximity_offset(&mut self) {
        let dx = self.proximity.cursor_pos.0 - self.proximity.anchor.0;
        let dy = self.proximity.cursor_pos.1 - self.proximity.anchor.1;

        let vw = self.state.viewport_extent.width;
        let vh = self.state.viewport_extent.height;
        let cw = self.state.content_extent.width;
        let ch = self.state.content_extent.height;

        // Cap ratio so large content doesn't cause teleportation.
        // Max ratio 8: 100px mouse displacement = 800px content scroll (~1.5 viewports).
        const MAX_RATIO: f32 = 8.0;
        let ratio_x = if vw > 0.0 {
            (cw / vw).min(MAX_RATIO)
        } else {
            0.0
        };
        let ratio_y = if vh > 0.0 {
            (ch / vh).min(MAX_RATIO)
        } else {
            0.0
        };

        let new_x = self.proximity.scroll_origin.0 + dx * ratio_x;
        let new_y = self.proximity.scroll_origin.1 + dy * ratio_y;

        let max_x = (cw - vw).max(0.0);
        let max_y = (ch - vh).max(0.0);
        let clamped_x = new_x.clamp(0.0, max_x);
        let clamped_y = new_y.clamp(0.0, max_y);

        self.state.logical_offset.x = clamped_x;
        self.state.logical_offset.y = clamped_y;
        // Smooth convergence via DRAG_LERP (same path as scrollbar thumb drag)
        self.smooth.set_target_offset((clamped_x, clamped_y));
        self.publish_vertical_scroll_position();
    }

    /// SPEC-HAL-268: Deactivate proximity scroll and clean up
    fn deactivate_proximity_scroll(&mut self) {
        self.proximity.active = false;
        self.proximity.source = ProximityScrollSource::None;
        self.smooth.end_drag();
        self.smooth.set_force_animate(false);
    }
}

impl Widget for ScrollArea {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // =======================================================================
        // INV-SCROLLBAR-GEOM-1: Scrollbar config sync (CANONICAL LOCATION)
        // =======================================================================
        // Theme → Config sync MUST happen here, before any paint/event consumer.
        // Layout always runs before paint/event each frame (engine guarantee).
        // All consumers (paint_scrollbars, handle_scrollbar_down, update_scrollbar_hover,
        // ScrollbarV/H::layout helper) read from config, never theme directly.
        // =======================================================================
        self.scrollbar_v.config.thickness = cx.theme.scrollbar_geom.thickness;
        self.scrollbar_v.config.corner_radius = cx.theme.scrollbar_geom.corner_radius;
        self.scrollbar_v.config.min_thumb_size = cx.theme.scrollbar_geom.min_thumb_size;
        self.scrollbar_v.config.hit_expansion = cx.theme.scrollbar_geom.hit_expansion;
        self.scrollbar_h.config.thickness = cx.theme.scrollbar_geom.thickness;
        self.scrollbar_h.config.corner_radius = cx.theme.scrollbar_geom.corner_radius;
        self.scrollbar_h.config.min_thumb_size = cx.theme.scrollbar_geom.min_thumb_size;
        self.scrollbar_h.config.hit_expansion = cx.theme.scrollbar_geom.hit_expansion;

        // INV-SCROLL-FINITE-MEASURE: An unbounded maximum is measurement
        // capability, not an allocated viewport extent. ScrollArea has no
        // intrinsic size on that axis, so retain the finite incoming minimum
        // until a bounded allocation pass arrives.
        let finite_extent = |min: f32, max: f32| {
            if max.is_finite() {
                max.max(0.0)
            } else if min.is_finite() {
                min.max(0.0)
            } else {
                0.0
            }
        };

        // SPEC-HAL-319: standalone empty mode — render an empty viewport
        // sized to the constraints. If children have been added to the
        // live tree after construction (composition path), stack them
        // vertically inside the viewport; otherwise return zero content.
        if self.standalone_empty {
            let size = Size::new(
                finite_extent(constraints.min_width, constraints.max_width),
                finite_extent(constraints.min_height, constraints.max_height),
            );
            self.container_size = size;
            if !children.is_empty() {
                let inner = Constraints::new(0.0, size.width, 0.0, f32::INFINITY);
                let mut total = 0.0_f32;
                for &child_id in children {
                    let child_size = cx.layout_child(child_id, inner);
                    cx.set_child_rect(
                        child_id,
                        RectF::new(0.0, total, size.width, child_size.height),
                    );
                    total += child_size.height;
                }
                let content = Size::new(size.width, total.max(size.height));
                return LayoutResult::new(size, content);
            }
            return LayoutResult::new(size, size);
        }

        // SPEC-HAL-154: content is guaranteed wired+sealed at this point.
        let content = self.content.get();

        // SPEC-HAL-011 Section 3.9:
        // 1. Determine viewport (configured or from constraints)
        // 2. Layout content with constrained width, infinite height
        // 3. Compute max_scroll = content_size - viewport_size
        // 4. Return viewport size (NOT content size)

        // Stage 2.8: Detect "final pass" (tight constraints from Flex allocation).
        // Flex calls layout twice: measurement pass (loose) then allocation pass (tight).
        // Only update scroll metrics on the final/tight pass to avoid premature clamping.
        let is_tight_w = (constraints.min_width - constraints.max_width).abs() < 0.01;
        let is_tight_h = (constraints.min_height - constraints.max_height).abs() < 0.01;
        let is_final_pass = is_tight_w && is_tight_h;
        self.layout_anchor_metrics_finalized = is_final_pass;

        // Container size = full allocation (for scrollbar positioning in paint).
        // Configured viewport extents remain authoritative when present; an
        // otherwise unbounded measurement axis retains only its finite minimum.
        let requested_container = if let Some(config) = self.configured_viewport {
            Size::new(
                config.width.min(constraints.max_width),
                config.height.min(constraints.max_height),
            )
        } else {
            Size::new(constraints.max_width, constraints.max_height)
        };
        let container_size = Size::new(
            finite_extent(constraints.min_width, requested_container.width),
            finite_extent(constraints.min_height, requested_container.height),
        );

        // =======================================================================
        // Child-facing viewport: the full container. Child constraints and
        // virtualization are unchanged by overlay-scrollbar occlusion.
        let content_viewport_size = container_size;

        // SPEC-HAL-110 §18 / INV-SCROLL-DOMAIN-CLOSED (occlusion-aware): an
        // overlay scrollbar occludes a `thickness` strip of the container on
        // its edge, so the OPPOSING axis's scroll domain must extend past
        // that strip — otherwise the last `thickness` px of content are
        // unreachable at max scroll (Workbench pipeline-table regression,
        // 2026-06-12). Bar visibility derives from the previous frame's
        // content extent: frame-driven convergence, no same-pass relayout
        // (SPEC-PM-005 discipline). Only the scroll domain changes; child
        // layout is untouched.
        let bar_thickness = self.scrollbar_v.config.thickness;
        let hbar_occludes =
            self.enable_scrollbar_h && self.state.content_extent.width > container_size.width + 0.5;
        let vbar_occludes = self.enable_scrollbar_v
            && self.state.content_extent.height > container_size.height + 0.5;
        let mut domain_viewport = Size::new(
            container_size.width - if vbar_occludes { bar_thickness } else { 0.0 },
            container_size.height - if hbar_occludes { bar_thickness } else { 0.0 },
        );

        self.container_size = container_size;
        self.content_offset = 0.0;

        // Track old extents for drag cancellation check
        let old_content = self.state.content_extent;

        // INV-LAYOUT-AUTHORITY-1: Layout constraints are authoritative.
        // Content receives viewport height as max_height, enabling constraint-driven
        // virtualization. List derives visible slot count from this constraint.
        //
        // SPEC-HAL-086 Stage 8 (REVISED): Width ALWAYS constrained to viewport.
        // - Child uses LayoutResult.content_size.width to report overflow (W_content).
        // - ScrollArea compares content_size.width > viewport.width for scrollbar visibility.
        // - Width NEVER passes INFINITY: child must report content_size, not echo constraints.
        // PM-005: Height is INFINITY for non-virtualizing children (Flex, etc.) so they
        // overflow naturally instead of shrink-compressing. Virtualizing children (List,
        // ListView) receive bounded height per INV-LIST-VIEWPORT-1.
        let needs_bounded = cx.tree.needs_viewport_constraint(content);
        let max_h = if self.enable_scrollbar_v && !needs_bounded {
            f32::INFINITY
        } else if needs_bounded && !content_viewport_size.height.is_finite() {
            // INV-LIST-VIEWPORT-1: A child that declared needs_viewport_constraint = true
            // MUST NOT receive INFINITY as max_height. When our own incoming constraints
            // are unbounded (parent is measuring us — typical Flex measurement pass for a
            // flexible child), there is no viewport to give. Passing 0.0 yields a
            // degenerate viewport: virtualization returns zero visible rows, the body
            // performs no per-row materialization, and content_size is still computed
            // from total_count × row_height. The subsequent tight/final pass arrives
            // with proper constraints and the child lays out normally.
            //
            // Without this branch, virtualization is bypassed: at NODE_COUNT=100k the
            // body would materialize 10k+ disclosure widgets per measurement pass.
            0.0
        } else {
            content_viewport_size.height
        };
        let content_constraints = Constraints::new(0.0, content_viewport_size.width, 0.0, max_h);

        // SPEC-HAL-131A Stage 14: Anchor resolution moved to post-layout phase.
        // See post_layout_resolve_scroll_anchors() in app.rs.
        // In-layout resolution removed to avoid double-resolution.

        // SPEC-HAL-131A: Get content size from child layout result
        // SPEC-HAL-134: Use engine helper to ensure pre_layout() is called.
        // INV-PHASE-TRANSITION-1: No widget calls another widget's pre_layout() directly.
        let retained_horizontal_offset = if is_final_pass {
            self.pending_retained_horizontal_offset()
        } else {
            None
        };
        let retained_vertical_offset = if is_final_pass {
            self.pending_retained_vertical_offset()
        } else {
            None
        };
        let content_owned_layout = is_final_pass && self.content_owns_vertical_translation;
        let mut settled_before_realization = false;
        let mut layout_content = |cx: &mut LayoutContext| {
            if retained_horizontal_offset.is_some() || retained_vertical_offset.is_some() {
                cx.layout_child_full_with_scroll_settlement(
                    content,
                    content_constraints,
                    |extent| {
                        settled_before_realization = true;
                        self.finalize_scroll_geometry(
                            container_size,
                            extent,
                            &mut domain_viewport,
                            retained_horizontal_offset,
                            retained_vertical_offset,
                        )
                    },
                )
            } else {
                cx.layout_child_full(content, content_constraints)
            }
        };
        let result = if content_owned_layout {
            cx.with_content_owned_vertical_translation(content, layout_content)
        } else {
            layout_content(cx)
        };

        // SPEC-HAL-131A: Use content_size for scroll extent (supports virtualization)
        // content_size = full logical size; size = rendered/visible size
        let content_size = result.content_size;

        // SPEC-HAL-024: Update content rect position (layout_child_full sets size)
        // Scroll offsets applied by traversal, not layout
        //
        // INV-SA-GEOM-1 (ScrollArea content coverage):
        // The child rect height assigned to content MUST be >= content_viewport_size.height.
        // This ensures the content rect always covers the viewport, preventing black gaps
        // when the realized content (e.g., virtualized List) is shorter than viewport.
        let child_rect_height = result.size.height.max(content_viewport_size.height);
        let child_rect_width = result.size.width.max(content_viewport_size.width);

        debug_assert!(
            child_rect_height >= content_viewport_size.height - 0.01,
            "INV-SA-GEOM-1 VIOLATION: child_rect_height ({}) < content_viewport_size.height ({})",
            child_rect_height,
            content_viewport_size.height
        );

        cx.set_child_rect(
            content,
            RectF::new(0.0, 0.0, child_rect_width, child_rect_height),
        );

        // Stage 2.8: Only update scroll metrics on the final/tight pass.
        // In measurement passes (loose constraints), skip all state mutations to avoid
        // premature clamping based on incorrect max_scroll values.
        if is_final_pass {
            if !settled_before_realization {
                self.finalize_scroll_geometry(
                    container_size,
                    content_size,
                    &mut domain_viewport,
                    retained_horizontal_offset,
                    retained_vertical_offset,
                );
            }

            // SPEC-HAL-086 Stage 10: Cancel active drags when content size ACTUALLY changes.
            // During drag, we capture track geometry at mouse-down. If content changes
            // (items added/removed), the captured geometry becomes stale.
            //
            // NOTE: We intentionally ignore viewport changes because Flex calls layout
            // twice per frame (measurement + final) with different viewport sizes.
            // This oscillation (e.g., 500 → 358.1 → 500 → 358.1) would cancel drags
            // every frame. Only content size changes represent real geometry changes
            // that require drag cancellation.
            //
            // FIX A2: old_content is content_extent which includes 2*root_inset.
            // We must compare it to content_size (also includes 2*root_inset),
            // NOT content_size (which excludes root_inset). Comparing like-with-like
            // prevents the delta from root_inset from triggering spurious drag cancellation.
            // SPEC-HAL-086 Stage 10: Cancel active drags ONLY when content SHRINKS.
            // Content growth (Console adding lines, list appending rows) is safe —
            // the captured drag geometry drifts slightly but the drag remains usable.
            // Content shrinkage is dangerous — captured max_scroll may exceed new
            // max_scroll, producing invalid scroll offsets.
            let content_shrunk_v = content_size.height < old_content.height - 0.5;
            let content_shrunk_h = content_size.width < old_content.width - 0.5;

            if self.scrollbar_v.dragging && content_shrunk_v {
                self.scrollbar_v.end_drag();
                self.smooth.end_drag();
            }
            if self.scrollbar_h.dragging && content_shrunk_h {
                self.scrollbar_h.end_drag();
                self.smooth.end_drag();
            }
        }

        // A bounded parent allocation remains authoritative. During an
        // unbounded measurement pass, return the finite retained viewport
        // extent rather than leaking measurement capability into tree geometry.
        // If horizontal scrolling is enabled, only content width may overflow.
        let assigned_width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            container_size.width
        };
        let assigned_height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            container_size.height
        };

        // FIX #6: Runtime assertions (strict)
        debug_assert!(
            !constraints.max_width.is_finite()
                || (assigned_width - constraints.max_width).abs() < 0.1,
            "ScrollArea: returned width ({}) must equal constraints.max_width ({})",
            assigned_width,
            constraints.max_width
        );

        // =========================================================================
        // INV-SCROLL-DOMAIN-CLOSED: Scroll domain boundary assertions
        // =========================================================================
        //
        // With INV-ROOT-INSET-SCROLLABLE, the scroll domain is:
        // - viewport_extent = container_size
        // - content_extent = child_content + 2*root_inset
        // - max_scroll = content_extent - viewport_extent
        //              = (child_content + 2*inset) - container
        //              = child_content - (container - 2*inset)
        //              = child_content - content_viewport_size  (same as old formula!)
        //
        // Verify that:
        // - max_scroll_y = child_content - content_viewport_size (scroll range preserved)
        // - At scroll_y == 0: child appears at root_inset from container top (INV-NO-FROZEN-GUTTER)
        // - At scroll_y == max: child_bottom at container - root_inset (symmetric padding)
        //
        // Stage 2.8: Only check on final pass, since state is only updated then.
        #[cfg(debug_assertions)]
        if is_final_pass && content_viewport_size.height > 1.0 {
            let max_scroll = self.state.max_scroll();

            // Verify max_scroll formula: max_scroll_y = child_content - content_viewport_size
            // This equals (content_extent - viewport_extent) with expanded extents.
            let expected_max_y = (content_size.height - domain_viewport.height).max(0.0);
            debug_assert!(
                (max_scroll.y - expected_max_y).abs() < 0.01,
                "INV-SCROLL-DOMAIN-CLOSED VIOLATION: max_scroll.y ({}) != child_content ({}) - domain_viewport ({}).  \
                 Formula mismatch.",
                max_scroll.y,
                content_size.height,
                domain_viewport.height
            );

            // NOTE: logical_offset may temporarily exceed max_scroll during Flex measurement pass.
            // Clamping happens in scroll input handlers, not during layout.
            // This is intentional to avoid premature clamping based on measurement-pass viewport.

            // INV-ROOT-INSET-SCROLLABLE: At scroll_y == 0, child is at (root_inset, root_inset).
            // Content appears inset from container top. Clip is at (0, 0), so no frozen gutter.
            // The inset is visible and scrolls away as user scrolls.

            // INV-SCROLL-DOMAIN-CLOSED: At scroll_y == max, child_bottom at container - root_inset.
            // viewport shows [max_scroll, max_scroll + container] of content space.
            // Child bottom = root_inset + child_h - max_scroll = container - root_inset.
            // Bottom inset is symmetric with top inset.
        }

        // =========================================================================
        // HAL_DEBUG_SCROLL: Viewport height source diagnostics (Stage 2.7/2.8)
        // =========================================================================
        #[cfg(debug_assertions)]
        {
            if std::env::var_os("HAL_DEBUG_SCROLL").is_some() {
                let max_scroll = self.state.max_scroll();
                let logical_y = self.state.logical_offset.y;
                let vis_offset = self.smooth.visual_offset();
                let tgt_offset = self.smooth.target_offset();
                eprintln!(
                    "[HAL_DEBUG_SCROLL] ScrollArea layout: final={} c_min_h={:.1} c_max_h={:.1} viewport_h={:.1} content_h={:.1} max_y={:.1} logical_y={:.1} vis_y={:.1} tgt_y={:.1}",
                    if is_final_pass { 1 } else { 0 },
                    constraints.min_height,
                    constraints.max_height,
                    content_viewport_size.height,
                    content_size.height,
                    max_scroll.y,
                    logical_y,
                    vis_offset.1,
                    tgt_offset.1,
                );
            }
        }

        // =========================================================================
        // INV-SCROLL-TIGHT-CONSTRAINTS: Debug-only invariant check
        // =========================================================================
        //
        // ScrollArea only finalizes scroll metrics on final/tight passes.
        // If content exceeds viewport but we never get a tight pass, scrolling
        // will silently break (max_scroll stays 0).
        //
        // This check fires a warning when:
        // - content_h > viewport_h (vertical) OR content_w > viewport_w (horizontal)
        // - this pass is NOT a final/tight pass
        // - we haven't already warned for this instance (rate-limiting)
        //
        // Gated by HAL_WARN_SCROLL_TIGHT_CONSTRAINTS=1 to avoid noise in normal debug runs.
        // =========================================================================
        #[cfg(debug_assertions)]
        {
            // On final pass, clear the warning flag (state has been updated correctly)
            if is_final_pass {
                self.warned_no_tight_constraints = false;
            } else {
                // Check if content exceeds viewport and we haven't warned yet
                let content_exceeds_v = content_size.height > content_viewport_size.height + 0.5;
                let content_exceeds_h = self.enable_scrollbar_h
                    && content_size.width > content_viewport_size.width + 0.5;

                if (content_exceeds_v || content_exceeds_h) && !self.warned_no_tight_constraints {
                    // Only warn if env var is set
                    if std::env::var_os("HAL_WARN_SCROLL_TIGHT_CONSTRAINTS").is_some() {
                        tracing::warn!(
                            "[INV-SCROLL-TIGHT-CONSTRAINTS] ScrollArea content exceeds viewport but no tight pass received. \
                             Scrolling will NOT work. Fix: Wrap ScrollArea in Flex/other parent that gives tight constraints. \
                             viewport_h={:.1} content_h={:.1} c_min_h={:.1} c_max_h={:.1} final={}",
                            content_viewport_size.height,
                            content_size.height,
                            constraints.min_height,
                            constraints.max_height,
                            if is_final_pass { 1 } else { 0 }
                        );
                    }
                    self.warned_no_tight_constraints = true;
                }
            }
        }

        // SPEC-HAL-131A: Return LayoutResult with scroll info for position computation.
        // No smoothing - logical_offset is the visual offset.
        let offset = self.state.logical_offset;
        LayoutResult::scroll(
            Size::new(assigned_width, assigned_height),
            content_size,
            (offset.x, offset.y),
        )
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        // SPEC-HAL-086 Stage 6: Visual offset is updated by WidgetTree::update_scroll_animations()
        // in the frame loop, not here. ScrollArea.paint() only reads visual_offset() for rendering.
        //
        // SPEC-HAL-011 Section 3.9: ScrollArea paints nothing in main paint pass
        // SPEC Section 5: ScrollArea must NOT perform its own clipping
        // Clipping is applied by paint_node() via clip_bounds()
        // Children are painted by traversal in app::paint_node()
        // Scrollbars are painted in paint_overlay() to render on top of children
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        // Paint scrollbars AFTER children so they render on top
        if self.show_scrollbars && (!self.auto_hides_scrollbars() || self.scrollbar_fade_alpha > 0.0) {
            self.paint_scrollbars(cx, rect, tree_widget_id);
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // SPEC-HAL-011 Section 3.9: Handle scroll events
        // SPEC-HAL-086 Stage 8: Handle scrollbar interaction

        match event {
            Event::Pointer(pe) => {
                let competing_scroll = self.scroll_intent_admission.blocks_competing_scroll();
                if competing_scroll
                    && matches!(
                        pe.kind,
                        PointerEventKind::Down | PointerEventKind::Scroll { .. }
                    )
                {
                    return false;
                }
                // A descendant overlay is logically parented through this
                // ScrollArea, but it is not part of the covered scroll surface.
                // ScrollArea-owned scrollbar overlays arrive in Target phase.
                if cx.pointer_route_is_overlay() && phase != EventPhase::Target {
                    if phase == EventPhase::Capture
                        && pe.kind == PointerEventKind::Move
                        && pe.pointer_type != PointerType::Touch
                        && self.clear_scrollbar_rail_hover()
                    {
                        cx.invalidate_paint();
                    }
                    return false;
                }

                // DEBUG: Trace scrollbar interaction
                #[cfg(debug_assertions)]
                {
                    let v_drag = self.scrollbar_v.dragging;
                    let h_drag = self.scrollbar_h.dragging;
                    if v_drag || h_drag || matches!(pe.kind, PointerEventKind::Down) {
                        tracing::debug!(
                            "[SCROLLAREA] handle_event: phase={:?} kind={:?} v_drag={} h_drag={} local_pos=({:.1}, {:.1})",
                            phase,
                            pe.kind,
                            v_drag,
                            h_drag,
                            pe.local_pos.0,
                            pe.local_pos.1
                        );
                    }
                }

                // Handle drag continuation regardless of phase (we have pointer capture)
                if self.scrollbar_v.dragging || self.scrollbar_h.dragging {
                    match pe.kind {
                        PointerEventKind::Move => {
                            #[cfg(debug_assertions)]
                            tracing::debug!("[SCROLLAREA] Drag continuation: handling Move");
                            return self.handle_scrollbar_drag(cx, pe);
                        }
                        PointerEventKind::Up => {
                            // SPEC-HAL-131A: End drag mode
                            #[cfg(debug_assertions)]
                            tracing::debug!("[SCROLLAREA] Drag continuation: handling Up");
                            self.finish_scrollbar_drag();
                            if self.auto_hides_scrollbars() {
                                self.observe_scrollbar_rail(cx, pe);
                            }
                            return true;
                        }
                        _ => {}
                    }
                }

                // SPEC-HAL-268: Proximity scroll — runs in all phases (Target + Bubble)
                if let PointerEventKind::Move = pe.kind {
                    // Active proximity scroll: update position and check deactivation
                    if self.proximity.active {
                        let previous = self.state.logical_offset;
                        self.proximity.cursor_pos = (pe.local_pos.0, pe.local_pos.1);
                        self.apply_proximity_offset();
                        self.notify_scroll_input(cx, previous);
                        cx.request_cursor(crate::core::event::CursorIcon::Hidden);

                        // Check modifier release for modifier-activated modes
                        if matches!(
                            self.proximity.source,
                            ProximityScrollSource::CtrlDoubleTap
                                | ProximityScrollSource::ModifierCombo
                        ) {
                            let source = self.check_proximity_activation(
                                &pe.modifiers,
                                cx.ctrl_double_tap_active(),
                            );
                            if source == ProximityScrollSource::None {
                                // Modifiers released — deactivate
                                cx.request_cursor(crate::core::event::CursorIcon::Default);
                                cx.release_pointer();
                                self.deactivate_proximity_scroll();
                            }
                        }
                    }
                    // Not active: check for new activation (only when no capture held)
                    else if !cx.has_pointer_capture() && !competing_scroll {
                        let source = self
                            .check_proximity_activation(&pe.modifiers, cx.ctrl_double_tap_active());
                        if source != ProximityScrollSource::None {
                            self.retire_layout_anchor_for_scroll_intent();
                            self.proximity.active = true;
                            self.proximity.source = source;
                            self.proximity.anchor = (pe.local_pos.0, pe.local_pos.1);
                            self.proximity.scroll_origin =
                                (self.state.logical_offset.x, self.state.logical_offset.y);
                            self.proximity.cursor_pos = (pe.local_pos.0, pe.local_pos.1);
                            self.smooth.begin_drag();
                            self.smooth.set_force_animate(true);
                            cx.capture_pointer(crate::core::event::PointerButton::Auxiliary);
                            cx.request_cursor(crate::core::event::CursorIcon::Hidden);
                        }
                    }
                }

                // SPEC-HAL-268: Middle mouse proximity scroll — runs in all phases
                if let PointerEventKind::Down = pe.kind {
                    if pe.button == Some(crate::core::event::PointerButton::Auxiliary)
                        && !cx.has_pointer_capture()
                        && !self.proximity.active
                    {
                        self.retire_layout_anchor_for_scroll_intent();
                        self.proximity.active = true;
                        self.proximity.source = ProximityScrollSource::MiddleMouse;
                        self.proximity.anchor = (pe.local_pos.0, pe.local_pos.1);
                        self.proximity.scroll_origin =
                            (self.state.logical_offset.x, self.state.logical_offset.y);
                        self.proximity.cursor_pos = (pe.local_pos.0, pe.local_pos.1);
                        self.smooth.begin_drag();
                        self.smooth.set_force_animate(true);
                        cx.capture_pointer(crate::core::event::PointerButton::Auxiliary);
                        return true;
                    }
                }
                if let PointerEventKind::Up = pe.kind {
                    if pe.button == Some(crate::core::event::PointerButton::Auxiliary)
                        && self.proximity.active
                        && self.proximity.source == ProximityScrollSource::MiddleMouse
                    {
                        cx.request_cursor(crate::core::event::CursorIcon::Default);
                        cx.release_pointer();
                        self.deactivate_proximity_scroll();
                        return true;
                    }
                }

                // SPEC-HAL-086 Stage 8: Handle scrollbar clicks in Target phase
                // (SceneNode::ScrollbarV/H makes ScrollArea the hit target when clicking scrollbars)
                if phase == EventPhase::Target {
                    if let PointerEventKind::Down = pe.kind {
                        #[cfg(debug_assertions)]
                        tracing::debug!(
                            "[SCROLLAREA] Target phase Down: calling handle_scrollbar_down"
                        );
                        if self.handle_scrollbar_down(cx, pe) {
                            #[cfg(debug_assertions)]
                            tracing::debug!(
                                "[SCROLLAREA] handle_scrollbar_down returned true, v_drag={}",
                                self.scrollbar_v.dragging
                            );
                            return true;
                        }
                    }
                }

                // Scrollbar interactions are handled above. At this point ScrollArea
                // leaves bubbled pointer events unconsumed. Ancestors decide hover and
                // resize admission with their own geometry and existing capture, phase
                // and button checks.
                if phase == EventPhase::Bubble {
                    match pe.kind {
                        // Ancestors can inspect movement for hover.
                        PointerEventKind::Move => {
                            return false;
                        }
                        // Ancestors qualify presses for resize.
                        PointerEventKind::Down => {
                            return false;
                        }
                        _ => {
                            return false;
                        }
                    }
                }

                match pe.kind {
                    PointerEventKind::Scroll { delta_x, delta_y } => {
                        // SPEC-HAL-038 / INV-INTERACTION-STATE-5: Scroll requires capture.
                        // A widget MUST NOT consume scroll input unless it is in capture state.
                        // If not captured, return false (do not handle).
                        // If captured (e.g., during scrollbar drag), consume and scroll.
                        if !cx.has_pointer_capture() {
                            return false;
                        }

                        // SHIFT+scroll: swap axes for horizontal scrolling.
                        // Standard platform behavior (Windows, macOS).
                        // Only swap when delta_x is zero (pure vertical input).
                        let (delta_x, delta_y) = if pe.modifiers.shift && delta_x == 0.0 {
                            (delta_y, 0.0)
                        } else {
                            (delta_x, delta_y)
                        };

                        // INV-RESIZE-HSCROLL-2: Zero delta_x when horizontal scroll is frozen
                        // (e.g., during column resize drag).
                        let frozen = self.horizontal_freeze.as_ref().map_or(false, |f| f.get());
                        let effective_delta_x = if frozen { 0.0 } else { delta_x };

                        // SPEC-HAL-131A: Use scroll_by for incremental scroll
                        self.retire_layout_anchor_for_scroll_intent();
                        let previous = self.state.logical_offset;
                        self.state
                            .scroll_by(Point::new(-effective_delta_x, -delta_y));
                        // Sync target to smooth container for frame-driven convergence
                        let offset = self.state.logical_offset;
                        self.smooth.set_target_offset((offset.x, offset.y));
                        self.publish_vertical_scroll_position();
                        self.notify_scroll_input(cx, previous);
                        if !self.auto_hides_scrollbars() {
                            self.scrollbar_activity = true;
                        }
                        true
                    }
                    PointerEventKind::Move => {
                        // INV-HOVER-FORWARD-NO-CAPTURE: Do not forward during capture.
                        // Scrollbar drag is checked above (line 1335); this checks child capture.
                        if self.auto_hides_scrollbars() {
                            if matches!(phase, EventPhase::Capture | EventPhase::Target) {
                                self.observe_scrollbar_rail(cx, pe);
                            }
                        } else if self.show_scrollbars {
                            self.update_scrollbar_hover(
                                Point::new(pe.local_pos.0, pe.local_pos.1),
                                self.container_size,
                            );
                        }
                        if cx.has_pointer_capture() {
                            // During capture, only update local scrollbar hover, do not bubble.
                            return true; // Consume during capture to prevent parent hover.
                        }

                        false // Allow bubble to parent for separator hover.
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.clear_scrollbar_rail_hover() {
            cx.invalidate_paint();
        }
        // SPEC-HAL-268: Safety net — deactivate proximity scroll if hover lost
        // Normally unreachable during proximity scroll (pointer is captured)
        if self.proximity.active {
            // Note: cannot request cursor or release pointer from on_hover_leave
            // because the event has already been processed. Sticky cursor state
            // will be cleared on the next explicit cursor request.
            self.deactivate_proximity_scroll();
        }
    }

    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut EventContext,
        _pointer: crate::core::event::PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
        if self.auto_hides_scrollbars() && (self.scrollbar_v.dragging || self.scrollbar_h.dragging) {
            self.finish_scrollbar_drag();
            cx.invalidate_paint();
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        self.publish_vertical_scroll_position();

        if self.auto_hides_scrollbars() {
            self.update_scrollbar_reveal(dt, cx);
            return;
        }

        // Reset timer on activity.
        if self.scrollbar_activity {
            self.scrollbar_activity = false;
            self.scrollbar_hide_timer = OVERLAY_SCROLLBAR_HIDE_DELAY_SECS;
            self.scrollbar_fade_alpha = 1.0;
            return;
        }

        // Scrollbar hovered or dragged — stay fully visible, reset timer.
        if self.scrollbar_v.state != ScrollbarState::Normal
            || self.scrollbar_h.state != ScrollbarState::Normal
            || self.scrollbar_v.dragging
            || self.scrollbar_h.dragging
        {
            self.scrollbar_hide_timer = OVERLAY_SCROLLBAR_HIDE_DELAY_SECS;
            self.scrollbar_fade_alpha = 1.0;
            return;
        }

        // Count down.
        if self.scrollbar_hide_timer > 0.0 {
            self.scrollbar_hide_timer -= dt;
            return;
        }

        // Fade out.
        if self.scrollbar_fade_alpha > 0.0 {
            self.scrollbar_fade_alpha =
                (self.scrollbar_fade_alpha - dt / SCROLLBAR_FADE_SECS).max(0.0);
        }
    }

    fn needs_animation(&self) -> bool {
        // Need continuous frames while timer is running or fade is in progress.
        !self.auto_hides_scrollbars()
            && (self.scrollbar_hide_timer > 0.0 || self.scrollbar_fade_alpha > 0.001)
    }

    fn needs_update(&self) -> bool {
        if !self.auto_hides_scrollbars() {
            return false;
        }
        let axes = self.overflowing_scrollbar_axes();
        (axes.0 || axes.1)
            && (self.scrollbar_activity
                || (self.scrollbar_fade_alpha > 0.0 && !self.scrollbar_engaged(axes)))
    }

    fn debug_type_name(&self) -> &'static str {
        "ScrollArea"
    }

    fn clip_bounds(&self) -> Option<RectF> {
        // SPEC-HAL-086 Stage 6: Return viewport bounds relative to self.
        // paint_node() converts to absolute coordinates and wraps children in ClipNode.
        // Batcher captures clip state, renderer applies scissor for GPU clipping.
        //
        // =======================================================================
        // INV-VIEWPORT-ORIGIN-ZERO: Clip origin at (0, 0).
        // INV-NO-FROZEN-GUTTER: No permanent Y-offset in clip bounds.
        // =======================================================================
        //
        // Clip at container origin, not content_offset. Content is positioned at
        // (root_inset, root_inset) and scrolls within the container. At scroll=0,
        // the inset is visible. As scroll increases, the inset scrolls away and
        // content can reach the container edge.
        //
        // Child clipping is visible-viewport geometry, not scroll-domain
        // geometry. `state.viewport_extent` may subtract overlay scrollbar
        // occlusion for max-scroll/thumb math; using it here creates a frozen
        // gutter where body content is clipped narrower than its header/footer.
        // =======================================================================
        Some(RectF::new(
            0.0,
            0.0,
            self.container_size.width,
            self.container_size.height,
        ))
    }

    /// SPEC-R-020 Stage 6: Disabled for now (pipeline validated, no forced rounding)
    fn clip_bounds_rounded(&self) -> Option<crate::core::paint::RoundedCorners> {
        None
    }

    fn scroll_offset(&self) -> Option<(f32, f32)> {
        // Frame-driven smooth scrolling: return visual offset (smoothed toward target).
        //
        // INV-VIRT-SCROLL-1 ENFORCEMENT:
        // This is the SOLE source of scroll transform for the render pipeline.
        // Paint transform = -visual_offset (applied by tree traversal).
        // Content moves smoothly each frame as visual converges toward target.
        //
        // INV-VIRT-SCROLL-3 ENFORCEMENT:
        // Anchor resolution affects state.logical_offset (the target).
        // Visual offset smooths toward it; anchor logic does not affect visual directly.
        let offset = self.smooth.visual_offset();

        // The horizontal scroll link is published from update() (once per frame,
        // before paint), NOT here: scroll_offset() runs mid-paint-traversal, after
        // the header has already read the link. See update() / INV-HSCROLL-LINK-3.
        //
        // SPEC-PM-017 R1b: with content-owned vertical translation, the
        // content widget already painted at viewport-relative Y (single f64
        // subtraction); the scene must not translate Y again. X translation
        // (columns) is unchanged.
        if self.content_owns_vertical_translation {
            return Some((offset.0, 0.0));
        }
        Some((offset.0, self.effective_visual_scroll_y()))
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn gesture_surface(&self) -> &'static [GestureKind] {
        &[GestureKind::Scroll]
    }

    fn accepts_touch_scroll_gesture(&self) -> bool {
        true
    }

    fn touch_scroll_intent(&self, _motion: QualifiedTouchMotion, axis: GestureAxis) -> bool {
        let max = self.state.max_scroll();
        match axis {
            GestureAxis::Horizontal => self.enable_scrollbar_h && max.x > 0.0,
            GestureAxis::Vertical => {
                self.supported_scroll_axis() != ScrollAxis::Horizontal && max.y > 0.0
            }
            GestureAxis::Any => false,
        }
    }

    fn on_gesture_commit(&mut self, cx: &mut EventContext, kind: GestureKind) {
        if kind != GestureKind::Scroll {
            return;
        }
        let Some(motion) = cx.touch_scroll_motion() else {
            return;
        };
        let original_offset = match motion.axis {
            GestureAxis::Horizontal => self.state.logical_offset.x,
            GestureAxis::Vertical => self.target_scroll_y(),
            GestureAxis::Any => unreachable!("HAL-396 locks the selected axis"),
        };
        self.touch_scroll = Some(TouchScroll {
            press: motion.press,
            axis: motion.axis,
            original_offset,
            original_anchor: self.state.anchor,
            samples: {
                let mut samples = TouchScrollSamples::default();
                samples.record(motion.axis, motion.sample.origin_time, motion.sample.origin);
                samples.record(motion.axis, motion.sample.previous_time, motion.sample.previous);
                samples.record(motion.axis, motion.sample.time, motion.sample.position);
                samples
            },
        });
        // Sample the effective visible offset before retiring anchor tail slack.
        let previous = self.touch_scroll_visual_offset();
        if motion.axis == GestureAxis::Vertical {
            self.clear_layout_anchor();
            self.pending_vertical_restore = false;
        } else {
            self.pending_horizontal_restore = false;
        }
        self.state.clear_anchor();
        self.apply_touch_scroll_motion(cx, motion, previous);
    }

    fn on_touch_scroll_motion(&mut self, cx: &mut EventContext) {
        let Some(motion) = cx.touch_scroll_motion() else {
            return;
        };
        let Some(pan) = self.touch_scroll.as_mut()
            .filter(|pan| pan.press == motion.press && pan.axis == motion.axis) else
        {
            return;
        };
        pan.samples.record(motion.axis, motion.sample.time, motion.sample.position);
        self.apply_touch_scroll_motion(cx, motion, self.touch_scroll_visual_offset());
        if motion.is_release {
            let pan = self.touch_scroll.take().expect("matching direct pan");
            if let Some(velocity) = pan.samples.release_velocity(motion.sample.time) {
                let origin = TouchScrollSamples::axis_position(motion.axis, self.touch_scroll_visual_offset());
                let maximum = TouchScrollSamples::axis_position(motion.axis, self.state.max_scroll());
                if (velocity < 0.0 && origin > 0.0) || (velocity > 0.0 && origin < maximum) {
                    self.kinetic_touch_scroll = Some(KineticTouchScroll {
                        press: motion.press, axis: motion.axis, released_at: motion.sample.time,
                        origin, velocity,
                    });
                    cx.request_kinetic_scroll();
                }
            }
        }
    }

    fn on_gesture_cancel(&mut self, cx: &mut EventContext, kind: GestureKind) {
        if kind == GestureKind::Scroll
            && self
                .touch_scroll
                .as_ref()
                .is_some_and(|pan| Some(pan.press) == cx.touch_gesture_press())
        {
            let pan = self.touch_scroll.take().expect("same qualified pan");
            self.jump_touch_scroll(
                cx,
                pan.axis,
                pan.original_offset,
                self.touch_scroll_visual_offset(),
            );
            if let Some(anchor) = pan.original_anchor {
                self.state.set_anchor(anchor);
            }
        }
    }

    fn scroll_container(&self) -> Option<&ScrollContainer> {
        Some(&self.smooth)
    }

    fn scroll_container_mut(&mut self) -> Option<&mut ScrollContainer> {
        Some(&mut self.smooth)
    }

    /// SPEC-HAL-078B: Returns true if this scroll container has pending anchor resolution.
    ///
    /// CONFLICT-4: Widgets inside scroll containers with pending anchor resolution
    /// are denied layout transitions because anchor preservation takes priority.
    fn has_pending_anchor_resolution(&self) -> bool {
        self.state.is_anchor_dirty()
    }

    /// SPEC-HAL-154: Expose DeferredWiring for engine-controlled sealing.
    /// INV-SEAL-ENG-4: Widgets MUST NOT call seal() themselves (engine owns lifecycle).
    fn deferred_wiring_mut(
        &mut self,
    ) -> Option<&mut dyn crate::core::deferred_wiring::DeferredWiring> {
        Some(self)
    }

    /// SPEC-HAL-154: Immutable access for paint/event guard assertions.
    fn deferred_wiring(&self) -> Option<&dyn crate::core::deferred_wiring::DeferredWiring> {
        Some(self)
    }
}

impl ScrollArea {
    fn overflowing_scrollbar_axes(&self) -> (bool, bool) {
        let viewport = self.state.viewport_extent;
        let content = self.state.content_extent;
        (
            self.show_scrollbars
                && self.enable_scrollbar_v
                && self.scrollbar_v.is_visible(content.height, viewport.height),
            self.show_scrollbars
                && self.enable_scrollbar_h
                && self.scrollbar_h.is_visible(content.width, viewport.width),
        )
    }

    fn scrollbar_engaged(&self, (vertical, horizontal): (bool, bool)) -> bool {
        (vertical && self.scrollbar_v.state != ScrollbarState::Normal)
            || (horizontal && self.scrollbar_h.state != ScrollbarState::Normal)
    }

    fn update_scrollbar_reveal(&mut self, dt: f32, cx: &mut UpdateContext) {
        let previous_alpha = self.scrollbar_fade_alpha;
        let (vertical, horizontal) = self.overflowing_scrollbar_axes();
        let activity = std::mem::take(&mut self.scrollbar_activity);
        if !vertical && !self.scrollbar_v.dragging {
            self.scrollbar_v.state = ScrollbarState::Normal;
        }
        if !horizontal && !self.scrollbar_h.dragging {
            self.scrollbar_h.state = ScrollbarState::Normal;
        }
        if !vertical && !horizontal {
            self.scrollbar_hide_timer = 0.0;
            self.scrollbar_fade_alpha = 0.0;
        } else if activity || self.scrollbar_engaged((vertical, horizontal)) {
            self.scrollbar_hide_timer = OVERLAY_SCROLLBAR_HIDE_DELAY_SECS;
            self.scrollbar_fade_alpha = 1.0;
        } else if previous_alpha > 0.0 {
            let fade_dt = (dt - self.scrollbar_hide_timer).max(0.0);
            self.scrollbar_hide_timer = (self.scrollbar_hide_timer - dt).max(0.0);
            self.scrollbar_fade_alpha = (previous_alpha - fade_dt / SCROLLBAR_FADE_SECS).max(0.0);
        }
        if self.scrollbar_fade_alpha != previous_alpha {
            cx.invalidate_paint();
        }
    }

    pub(crate) fn scrollbar_overlay_visible(&self) -> bool {
        let (vertical, horizontal) = self.overflowing_scrollbar_axes();
        self.scrollbar_fade_alpha > 0.0 && (vertical || horizontal)
    }

    /// Container-space geometry before applying the horizontal content inset or
    /// ancestor scroll. Paint and input use the same presented scroll offset.
    fn scrollbar_layouts(
        &self,
        container: Size,
    ) -> (Option<ScrollbarLayout>, Option<ScrollbarLayout>) {
        let (vertical, horizontal) = self.overflowing_scrollbar_axes();
        let viewport = self.state.viewport_extent;
        let content = self.state.content_extent;
        let (scroll_x, scroll_y) = self.smooth.visual_offset();
        let height = container.height.max(0.0);
        let inset_width = (container.width - 2.0 * self.content_offset).max(0.0);
        let vertical = vertical.then(|| {
            let track_limit = if horizontal {
                height - self.scrollbar_h.config.thickness
            } else {
                height
            };
            self.scrollbar_v.layout(
                RectF::new(0.0, 0.0, container.width, height),
                track_limit,
                content.height,
                viewport.height,
                scroll_y,
            )
        });
        let horizontal = horizontal.then(|| {
            let track_limit = if vertical.is_some() {
                inset_width - self.scrollbar_v.config.thickness
            } else {
                inset_width
            };
            self.scrollbar_h.layout(
                RectF::new(0.0, 0.0, inset_width, height),
                track_limit,
                content.width,
                viewport.width,
                scroll_x,
            )
        });
        (vertical, horizontal)
    }

    fn paint_scrollbars(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        // =======================================================================
        // SPEC-HAL-110 Section 18: Scrollbar Visibility & Gutter Semantics
        // =======================================================================
        //
        // INV-SCROLLBAR-GUTTER:
        //   Scrollbar track space MUST be reserved iff scrollbar is visible.
        //   - Scrollbars render ONLY when content > viewport
        //   - NO unconditional space reservation in layout
        //   - Left/right padding identical when scrollbar hidden
        //   - Padding asymmetry permitted ONLY when scrollbar rendered
        //
        // INV-SCROLLBAR-CONTAINER-SPACE:
        //   Scrollbar geometry, hit-testing, and painting MUST use container rect.
        //   - paint_scrollbars(): uses rect (container)
        //   - handle_scrollbar_down(): uses container_size
        //   - update_scrollbar_hover(): uses container_size
        //   - scrollbar.layout(): widget_rect = container, viewport for max_scroll only
        //
        // INV-VIEWPORT-CONTENT-ONLY:
        //   viewport_extent is for scroll math only.
        //   MUST NOT be used for scrollbar track/hit geometry.
        //
        // =======================================================================

        let viewport = self.state.viewport_extent;

        // INV-ROOT-INSET-SCROLLABLE: scrollbar geometry stays in container
        // space even when viewport is reduced for occlusion-aware scroll math.
        debug_assert!(
            rect.width >= viewport.width - 0.1,
            "ScrollArea paint: rect.width ({}) must be >= viewport.width ({})",
            rect.width,
            viewport.width
        );

        let (vertical, horizontal) = self.scrollbar_layouts(Size::new(rect.width, rect.height));
        let interactive = self.auto_hides_scrollbars() || self.scrollbar_fade_alpha > 0.01;

        if let Some(layout) = vertical {
            let layout = layout.translated(rect.x, rect.y);
            if interactive {
                cx.register_overlay_hit_region(OverlayHitRegion {
                    abs_rect: layout.hit_rect,
                    owner_abs_rect: rect,
                    widget_id: tree_widget_id,
                });
            }

            let abs_thumb = layout.thumb;
            let thumb_color = self
                .scrollbar_v
                .thumb_color(&cx.theme.colors)
                .with_alpha(self.scrollbar_fade_alpha);
            // SPEC-HAL-093 v3 §2 R-0: scrollbar thumb is chrome on a base surface,
            // not a floating element - sharp 0px corners. R-XS (2px) and above are
            // reserved for overlays only (§2 Rule 3).
            // INV-PAINT-LAYERS-FOCUS: Scrollbar uses LAYER_SCROLLBAR (above focus indicators)
            cx.scene
                .push_rect(abs_thumb, thumb_color, LAYER_SCROLLBAR, cx.scene_key);

            // SPEC-HAL-093 v3: subtle grip ridges — barely-visible tonal
            // shift of the thumb color (≈6% in linear) per ridge.
            // Affordance hint, not glyphs. 3 ridges on tall thumbs, 2 on
            // medium, none on tiny thumbs.
            let grip_count: u32 = if abs_thumb.height > 40.0 {
                3
            } else if abs_thumb.height > 25.0 {
                2
            } else {
                0
            };
            if grip_count > 0 {
                let grip_spacing: f32 = 2.0;
                let grip_total_height = (grip_count as f32 - 1.0) * grip_spacing;
                let grip_width = 6.0_f32.min(abs_thumb.width * 0.5);
                let thumb_center_x = abs_thumb.x + abs_thumb.width / 2.0;
                let thumb_center_y = abs_thumb.y + abs_thumb.height / 2.0;
                let grip_start_y = thumb_center_y - grip_total_height / 2.0;
                let grip_x = thumb_center_x - grip_width / 2.0;
                let grip_color_dark = Color::from_linear(
                    (thumb_color.r - 0.06).max(0.0),
                    (thumb_color.g - 0.06).max(0.0),
                    (thumb_color.b - 0.06).max(0.0),
                    thumb_color.a,
                );
                let grip_color_light = Color::from_linear(
                    (thumb_color.r + 0.06).min(1.0),
                    (thumb_color.g + 0.06).min(1.0),
                    (thumb_color.b + 0.06).min(1.0),
                    thumb_color.a,
                );
                for i in 0..grip_count {
                    let y = grip_start_y + i as f32 * grip_spacing;
                    cx.scene.push_rect(
                        RectF::new(grip_x, y, grip_width, 1.0),
                        grip_color_dark,
                        LAYER_SCROLLBAR,
                        cx.scene_key,
                    );
                    cx.scene.push_rect(
                        RectF::new(grip_x, y + 1.0, grip_width, 1.0),
                        grip_color_light,
                        LAYER_SCROLLBAR,
                        cx.scene_key,
                    );
                }
            }
        }

        if let Some(layout) = horizontal {
            let layout = layout.translated(rect.x + self.content_offset, rect.y);
            if interactive {
                cx.register_overlay_hit_region(OverlayHitRegion {
                    abs_rect: layout.hit_rect,
                    owner_abs_rect: rect,
                    widget_id: tree_widget_id,
                });
            }

            let abs_thumb = layout.thumb;
            let thumb_color = self
                .scrollbar_h
                .thumb_color(&cx.theme.colors)
                .with_alpha(self.scrollbar_fade_alpha);
            // SPEC-HAL-093 v3 §2 R-0: scrollbar thumb is chrome on a base surface,
            // not a floating element - sharp 0px corners. R-XS (2px) and above are
            // reserved for overlays only (§2 Rule 3).
            // INV-PAINT-LAYERS-FOCUS: Scrollbar uses LAYER_SCROLLBAR (above focus indicators)
            cx.scene
                .push_rect(abs_thumb, thumb_color, LAYER_SCROLLBAR, cx.scene_key);

            // SPEC-HAL-093 v3: subtle grip ridges (vertical, on horizontal
            // thumb) — barely-visible tonal shift of the thumb color
            // (≈6% in linear) per ridge.
            let grip_count: u32 = if abs_thumb.width > 40.0 {
                3
            } else if abs_thumb.width > 25.0 {
                2
            } else {
                0
            };
            if grip_count > 0 {
                let grip_spacing: f32 = 2.0;
                let grip_total_width = (grip_count as f32 - 1.0) * grip_spacing;
                let grip_height = 6.0_f32.min(abs_thumb.height * 0.5);
                let thumb_center_x = abs_thumb.x + abs_thumb.width / 2.0;
                let thumb_center_y = abs_thumb.y + abs_thumb.height / 2.0;
                let grip_start_x = thumb_center_x - grip_total_width / 2.0;
                let grip_y = thumb_center_y - grip_height / 2.0;
                let grip_color_dark = Color::from_linear(
                    (thumb_color.r - 0.06).max(0.0),
                    (thumb_color.g - 0.06).max(0.0),
                    (thumb_color.b - 0.06).max(0.0),
                    thumb_color.a,
                );
                let grip_color_light = Color::from_linear(
                    (thumb_color.r + 0.06).min(1.0),
                    (thumb_color.g + 0.06).min(1.0),
                    (thumb_color.b + 0.06).min(1.0),
                    thumb_color.a,
                );
                for i in 0..grip_count {
                    let x = grip_start_x + i as f32 * grip_spacing;
                    cx.scene.push_rect(
                        RectF::new(x, grip_y, 1.0, grip_height),
                        grip_color_dark,
                        LAYER_SCROLLBAR,
                        cx.scene_key,
                    );
                    cx.scene.push_rect(
                        RectF::new(x + 1.0, grip_y, 1.0, grip_height),
                        grip_color_light,
                        LAYER_SCROLLBAR,
                        cx.scene_key,
                    );
                }
            }
        }
    }

    /// Handle scrollbar click (PointerDown) - called in Target phase
    ///
    /// SPEC-HAL-110 Section 18 INV-SCROLLBAR-CONTAINER-SPACE:
    /// Uses container_size for scrollbar geometry, matching paint_scrollbars().
    fn handle_scrollbar_down(
        &mut self,
        cx: &mut EventContext,
        pe: &crate::core::event::PointerEvent,
    ) -> bool {
        if !self.show_scrollbars {
            #[cfg(debug_assertions)]
            tracing::debug!(
                "[SCROLLAREA] handle_scrollbar_down: show_scrollbars=false, returning false"
            );
            return false;
        }

        let (local_x, local_y) = pe.local_pos;
        let (global_x, global_y) = pe.global_pos;
        if self.auto_hides_scrollbars()
            && (self.scrollbar_fade_alpha <= 0.0 || !cx.pointer_route_is_overlay())
        {
            return false;
        }
        #[cfg(debug_assertions)]
        let viewport = self.state.viewport_extent; // For scroll math only (INV-VIEWPORT-CONTENT-ONLY)
        #[cfg(debug_assertions)]
        let content = self.state.content_extent;
        // PM-002: Hit-test geometry MUST match painted geometry.
        // Scrollbar painted at visual_offset → hit-test at visual_offset.
        let visual = self.smooth.visual_offset();
        let scroll_x = visual.0;
        let scroll_y = visual.1;
        let max_scroll = self.state.max_scroll();
        let max_x = max_scroll.x;
        let max_y = max_scroll.y;

        // SPEC-HAL-086 Stage 10: Get canonical global rect from WidgetNode.absolute_paint_rect
        // This is the source of truth for global coordinates, NOT pointer-derived hacks.
        let global_rect = match cx.widget_global_rect() {
            Some(rect) => rect,
            None => {
                eprintln!("[SCROLLBAR ERROR] Could not get widget global rect");
                return false;
            }
        };

        let auto_hide = self.auto_hides_scrollbars();
        let container = if auto_hide {
            global_rect.size()
        } else {
            self.container_size
        };
        let inset = self.content_offset;
        let (vertical, horizontal) = self.scrollbar_layouts(container);
        let point = Point::new(local_x, local_y);

        #[cfg(debug_assertions)]
        tracing::debug!(
            "[SCROLLAREA] handle_scrollbar_down: local=({:.1},{:.1}) container=({:.1}x{:.1}) viewport=({:.1}x{:.1}) content=({:.1}x{:.1}) v_visible={} h_visible={}",
            local_x,
            local_y,
            container.width,
            container.height,
            viewport.width,
            viewport.height,
            content.width,
            content.height,
            vertical.is_some(),
            horizontal.is_some()
        );

        if let Some(layout) = vertical {
            let thumb_hit = layout.thumb.contains(point);
            let hit_area_contains = layout.hit_rect.contains(point);

            #[cfg(debug_assertions)]
            tracing::debug!(
                "[SCROLLAREA] V scrollbar hit check: point=({:.1},{:.1}) thumb={:?} hit_rect={:?} thumb_hit={} hit_area={}",
                point.x,
                point.y,
                layout.thumb,
                layout.hit_rect,
                thumb_hit,
                hit_area_contains
            );

            // SPEC-HAL-086 Stage 10: Capture track geometry in global coords
            let origin_y = if auto_hide {
                // Routing already projected local_y through the ancestor scrolls.
                global_y - local_y
            } else {
                global_rect.y + inset
            };
            let track_top = origin_y + layout.track.y;
            let track_bottom = track_top + layout.track.height;
            let thumb_len = layout.thumb.height;

            if thumb_hit {
                #[cfg(debug_assertions)]
                tracing::debug!("[SCROLLAREA] V thumb HIT! Calling begin_drag and capture_pointer");
                self.retire_layout_anchor_for_scroll_intent();
                self.scrollbar_v.begin_drag(
                    global_y,
                    scroll_y,
                    track_top,
                    track_bottom,
                    thumb_len,
                    max_y,
                );
                self.smooth.begin_drag(); // Snappier lerp for thumb tracking
                self.scrollbar_activity = true;
                if let Some(button) = pe.button {
                    cx.capture_pointer(button);
                    #[cfg(debug_assertions)]
                    tracing::debug!(
                        "[SCROLLAREA] capture_pointer called with button={:?}, v_dragging={}",
                        button,
                        self.scrollbar_v.dragging
                    );
                }
                return true;
            } else if hit_area_contains {
                // Click in track area (not on thumb) - jump to position AND start drag
                let click_ratio = (point.y - layout.track.y) / layout.track.height;
                let new_scroll = click_ratio * max_y;
                self.retire_layout_anchor_for_scroll_intent();
                let previous = self.state.logical_offset;
                self.state.set_offset(Point::new(scroll_x, new_scroll));
                // Sync target for jump
                let offset = self.state.logical_offset;
                self.smooth.set_target_offset((offset.x, offset.y));
                self.notify_scroll_input(cx, previous);
                self.scrollbar_activity = true;

                // Begin drag session with the NEW scroll position
                self.scrollbar_v.begin_drag(
                    global_y,
                    new_scroll,
                    track_top,
                    track_bottom,
                    thumb_len,
                    max_y,
                );
                self.smooth.begin_drag(); // Snappier lerp for thumb tracking
                if let Some(button) = pe.button {
                    cx.capture_pointer(button);
                }
                return true;
            }
        }

        if let Some(layout) = horizontal {
            let point_h = Point::new(local_x - inset, local_y);
            let thumb_hit = layout.thumb.contains(point_h);
            let hit_area_contains = layout.hit_rect.contains(point_h);

            // SPEC-HAL-086 Stage 10: Capture track geometry in global coords
            let origin_x = if auto_hide { global_x - local_x } else { global_rect.x };
            let track_left = origin_x + inset + layout.track.x;
            let track_right = track_left + layout.track.width;
            let thumb_len = layout.thumb.width;

            if thumb_hit {
                self.retire_layout_anchor_for_scroll_intent();
                self.scrollbar_h.begin_drag(
                    global_x,
                    scroll_x,
                    track_left,
                    track_right,
                    thumb_len,
                    max_x,
                );
                self.smooth.begin_drag(); // Snappier lerp for thumb tracking
                self.scrollbar_activity = true;
                if let Some(button) = pe.button {
                    cx.capture_pointer(button);
                }
                return true;
            } else if hit_area_contains {
                // Click in track area (not on thumb) - jump to position AND start drag
                // point_h.x is already inset-relative
                let click_ratio = (point_h.x - layout.track.x) / layout.track.width;
                let new_scroll = click_ratio * max_x;
                self.retire_layout_anchor_for_scroll_intent();
                let previous = self.state.logical_offset;
                self.state.set_offset(Point::new(new_scroll, scroll_y));
                // Sync target for jump
                let offset = self.state.logical_offset;
                self.smooth.set_target_offset((offset.x, offset.y));
                self.notify_scroll_input(cx, previous);
                self.scrollbar_activity = true;

                // Begin drag session with the NEW scroll position
                self.scrollbar_h.begin_drag(
                    global_x,
                    new_scroll,
                    track_left,
                    track_right,
                    thumb_len,
                    max_x,
                );
                self.smooth.begin_drag(); // Snappier lerp for thumb tracking
                if let Some(button) = pe.button {
                    cx.capture_pointer(button);
                }
                return true;
            }
        }

        false
    }

    /// Handle scrollbar drag (PointerMove while dragging)
    ///
    /// SPEC-HAL-131A: Scrollbar uses captured values from begin_drag() for stable ratio.
    /// Scrollbar drag uses snappier smoothing via smooth.begin_drag() mode.
    fn handle_scrollbar_drag(
        &mut self,
        cx: &mut EventContext,
        pe: &crate::core::event::PointerEvent,
    ) -> bool {
        let (global_x, global_y) = pe.global_pos;

        if self.scrollbar_v.dragging {
            let raw_scroll = self.scrollbar_v.update_drag(global_y);
            let previous = self.state.logical_offset;
            self.state.set_offset_y(raw_scroll);
            // Sync target for frame-driven convergence (drag mode uses snappier lerp)
            let offset = self.state.logical_offset;
            self.smooth.set_target_offset((offset.x, offset.y));
            self.publish_vertical_scroll_position();
            self.notify_scroll_input(cx, previous);
            self.scrollbar_activity = true;
            return true;
        }

        if self.scrollbar_h.dragging {
            let raw_scroll = self.scrollbar_h.update_drag(global_x);
            let previous = self.state.logical_offset;
            self.state.set_offset_x(raw_scroll);
            // Sync target for frame-driven convergence
            let offset = self.state.logical_offset;
            self.smooth.set_target_offset((offset.x, offset.y));
            self.notify_scroll_input(cx, previous);
            self.scrollbar_activity = true;
            return true;
        }

        false
    }

    /// Update scrollbar hover state
    ///
    /// SPEC-HAL-110 Section 18 INV-SCROLLBAR-CONTAINER-SPACE:
    /// Uses container_size for scrollbar geometry, matching paint_scrollbars().
    fn update_scrollbar_hover(&mut self, point: Point, container: Size) -> bool {
        let (vertical, horizontal) = self.scrollbar_layouts(container);
        let auto_hide = self.auto_hides_scrollbars();
        let hover_region = |layout: ScrollbarLayout| {
            if auto_hide { layout.hit_rect } else { layout.thumb }
        };
        let vertical = vertical.is_some_and(|layout| hover_region(layout).contains(point));
        let horizontal = horizontal.is_some_and(|layout| {
            hover_region(layout).contains(Point::new(point.x - self.content_offset, point.y))
        });
        self.set_scrollbar_hover(vertical, horizontal)
    }

    fn set_scrollbar_hover(&mut self, vertical: bool, horizontal: bool) -> bool {
        let state = |hovered| {
            if hovered { ScrollbarState::Hovered } else { ScrollbarState::Normal }
        };
        let next = (state(vertical), state(horizontal));
        let changed = (self.scrollbar_v.state, self.scrollbar_h.state) != next;
        (self.scrollbar_v.state, self.scrollbar_h.state) = next;
        self.scrollbar_activity |= changed && self.auto_hides_scrollbars();
        changed
    }

    pub(crate) fn clear_scrollbar_rail_hover(&mut self) -> bool {
        self.auto_hides_scrollbars()
            && !self.scrollbar_v.dragging
            && !self.scrollbar_h.dragging
            && self.set_scrollbar_hover(false, false)
    }

    fn observe_scrollbar_rail(&mut self, cx: &mut EventContext, pe: &PointerEvent) {
        if pe.pointer_type == PointerType::Touch
            || self.scrollbar_v.dragging
            || self.scrollbar_h.dragging
        {
            return;
        }
        // Capture delivers moves before a descendant can consume them. Use the
        // tree's clipped hit projection because Capture local_pos omits the
        // ancestor scroll transform.
        let changed = match (cx.scrollbar_rail_point(), cx.widget_global_rect()) {
            (Some(point), Some(rect)) => self.update_scrollbar_hover(point, rect.size()),
            _ => self.clear_scrollbar_rail_hover(),
        };
        if changed {
            cx.invalidate_paint();
        }
    }

    fn finish_scrollbar_drag(&mut self) {
        if self.scrollbar_v.dragging {
            self.scrollbar_v.end_drag();
        } else {
            self.scrollbar_h.end_drag();
        }
        self.smooth.end_drag();
        self.scrollbar_activity |= self.auto_hides_scrollbars();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{FusedInputEvent, InteractionDomains, ScrollSource, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::core::icon::IconRegistry;
    use crate::core::layout::FlexParams;
    use crate::core::rect_authority::UpdateAuthorityGuard;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::flex::Flex;
    use crate::widgets::gauge::{Gauge, GaugeConfig};
    use crate::widgets::groupbox::GroupBox;
    use std::time::Instant;

    #[test]
    fn scroll_area_scrollbar_reveal_policy_preserves_retained_paint_and_input() {
        use crate::core::app::{FrameReason, FrameTime, test_begin_frame_layout_tracking};
        use crate::core::event::{
            CancellationReason, PointerButton, PointerButtonsState, PointerDevice, PointerId,
            PointerRouteOrigin,
        };
        use crate::core::input_fusion::InputFusion;
        use crate::core::layout::EdgeInsets;
        use crate::core::scene::{RectNode, SceneNode};
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::container::Container;
        use std::sync::Arc;
        use std::time::Duration;

        struct RailContent {
            id: WidgetInternalId,
            height: f32,
            moves: usize,
            presses: usize,
        }
        impl WidgetDowncast for RailContent {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
        impl Widget for RailContent {
            fn id(&self) -> WidgetInternalId {
                self.id
            }

            fn layout(
                &mut self,
                _: &mut LayoutContext,
                constraints: Constraints,
                _: &[NodeId],
            ) -> LayoutResult {
                LayoutResult::fit(constraints.constrain(Size::new(220.0, self.height)))
            }

            fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
                cx.scene.push_rect(rect, Color::WHITE, 0, None);
            }

            fn handle_event(
                &mut self,
                cx: &mut EventContext,
                event: &Event,
                phase: EventPhase,
            ) -> bool {
                let Event::Pointer(pointer) = event else {
                    return false;
                };
                if phase != EventPhase::Target {
                    return false;
                }
                match pointer.kind {
                    PointerEventKind::Move => {
                        self.moves += 1;
                        true
                    }
                    PointerEventKind::Down => {
                        self.presses += 1;
                        cx.capture_pointer(PointerButton::Primary);
                        true
                    }
                    PointerEventKind::Up => true,
                    _ => false,
                }
            }
        }

        fn bars(nodes: &[SceneNode]) -> Vec<RectNode> {
            let mut rects = Vec::new();
            for node in nodes {
                match node {
                    SceneNode::Rect(rect) if rect.z_index == LAYER_SCROLLBAR => rects.push(rect.clone()),
                    SceneNode::Clip(clip) => rects.extend(bars(&clip.children)),
                    SceneNode::Layer(layer) => rects.extend(bars(&layer.children)),
                    SceneNode::Scroll(scroll) => rects.extend(bars(&scroll.children)),
                    SceneNode::RetainedFragment(fragment) => rects.extend(bars(&fragment.children)),
                    _ => {}
                }
            }
            rects
        }

        use ScrollAreaScrollbarRevealPolicy::{AutoHideOnScrollOrRailProximity, DefaultLegacy};
        for (policy, nested) in [
            (DefaultLegacy, false),
            (AutoHideOnScrollOrRailProximity, false),
            (AutoHideOnScrollOrRailProximity, true),
        ] {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Flex::column().with_padding(EdgeInsets::all(12.0))),
                None,
                0,
            );
            let (parent, outer) = if nested {
                let outer = tree.add_child(
                    root,
                    Box::new(ScrollArea::new_unwired().with_viewport_size(220.0, 200.0).with_scrollbars(false)),
                    None,
                    0,
                );
                let body = tree.add_child(outer, Box::new(Flex::column()), None, 0);
                tree.add_child(body, Box::new(Container::empty().with_height(70.0)), None, 0);
                tree.node_mut(outer).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>()
                    .unwrap().wire_content(body);
                (body, Some(outer))
            } else {
                (root, None)
            };
            let area_parent = if nested {
                tree.add_child(
                    parent,
                    Box::new(Container::empty().with_width(220.0).with_height(150.0)),
                    None,
                    0,
                )
            } else {
                parent
            };
            let area = tree.add_child(
                area_parent,
                Box::new(
                    ScrollArea::new_unwired()
                        .with_viewport_size(220.0, 150.0)
                        .enable_horizontal_scrollbar(false)
                        .with_scrollbar_reveal_policy(policy),
                ),
                None,
                0,
            );
            if nested {
                // This bounded host gives the inner ScrollArea its tight
                // settlement pass within the unbounded outer scroll content.
                crate::replace_widget!(
                    tree,
                    area_parent,
                    Container::new(area, 0.0).with_width(220.0).with_height(150.0)
                );
                tree.add_child(parent, Box::new(Container::empty().with_height(260.0)), None, 0);
            }
            let content = tree.add_child(
                area,
                Box::new(RailContent { id: WidgetInternalId::new(), height: 480.0, moves: 0, presses: 0 }),
                None,
                0,
            );
            tree.node_mut(area).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>()
                .unwrap().wire_content(content);

            let owner = tree.node(area).unwrap().id;
            let content_owner = tree.node(content).unwrap().id;
            let theme = Theme::default();
            let icons = IconRegistry::new(&[]);
            let mut text = FontManagerTextEngine::new();
            let mut harness = RetainedSceneTestHarness::new();
            let mut router = EventRouter::new();
            let mut fusion = InputFusion::new();
            let mut domains = InteractionDomains::new();
            let mut bindings = BindingRegistry::new();
            let mut time = FrameTime {
                now: Instant::now(),
                dt: Duration::ZERO,
                frame_index: 0,
                reason: FrameReason::Animation,
            };
            macro_rules! scroll_area {
                () => { tree.node(area).unwrap().widget.as_any().downcast_ref::<ScrollArea>().unwrap() };
            }
            macro_rules! frame {
                ($dt:expr) => {{
                    time.dt = Duration::from_secs_f32($dt);
                    time.now += time.dt;
                    time.frame_index += 1;
                    {
                        let _update = UpdateAuthorityGuard::enter();
                        tree.update_scroll_animations(time.dt.as_secs_f32());
                    }
                    test_begin_frame_layout_tracking();
                    harness.frame_with_gesture_sessions(
                        &mut tree, root, Constraints::tight(Size::new(256.0, 238.0)),
                        &theme, &mut text, &icons, &time, None, &mut domains,
                        WindowId::PRIMARY, &mut router, &mut fusion,
                    )
                }};
            }
            macro_rules! route {
                ($event:expr) => {
                    router.route_input_event(
                        &mut tree, $event, Some(root), &mut text,
                        Some(harness.animation_context()), domains.primary_state_mut(), &mut bindings,
                    )
                };
            }
            let move_pointer = |pos, primary| FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                delta: (0.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState { primary, ..Default::default() },
            };
            let down = |pos| FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            };
            let up = |pos| FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            };
            let wheel = |pos, delta, source| FusedInputEvent::Scroll {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                delta: (0.0, delta),
                source,
            };
            let overlay = |harness: &RetainedSceneTestHarness| harness.overlay_fragment(owner).unwrap();
            let assert_paint = |harness: &RetainedSceneTestHarness, layout: ScrollbarLayout, alpha: f32| {
                let painted = bars(&overlay(harness));
                assert_eq!(painted.first().unwrap().rect, layout.thumb);
                assert!(painted.len() > 1, "thumb and grip");
                assert!(painted.iter().all(|rect| (rect.color.a - alpha).abs() < 0.0001));
            };
            let jump_outer = |tree: &mut WidgetTree, outer, y| {
                tree.node_mut(outer).unwrap().widget.as_any_mut().downcast_mut::<ScrollArea>()
                    .unwrap().jump_to_y(y);
                tree.mark_paint_dirty(outer);
            };

            frame!(0.0);
            if let Some(outer) = outer {
                jump_outer(&mut tree, outer, 55.0);
                frame!(0.0);
            }
            let unscrolled = tree.get_absolute_paint_rect(area).unwrap();
            let (sx, sy) = tree.cumulative_scroll_offset(area);
            let rect = unscrolled.translate(-sx, -sy);
            macro_rules! local_layout {
                () => {{
                    let (vertical, horizontal) = scroll_area!().scrollbar_layouts(unscrolled.size());
                    assert!(horizontal.is_none());
                    vertical.unwrap()
                }};
            }
            let initial = local_layout!();
            let rail_x = rect.x + initial.hit_rect.x + 1.0;
            let outside = Point::new(rect.x + 2.0, rect.y - 2.0);
            let gutter = Point::new(rail_x, rect.y + initial.track.height - 1.0);
            let center = Point::new(rect.x + 60.0, rect.y + 40.0);

            if policy == DefaultLegacy {
                assert_paint(&harness, initial.translated(rect.x, rect.y), 1.0_f32);
                assert_eq!(tree.hit_test(gutter.x, gutter.y).unwrap().widget_id, owner);
                frame!(1.5);
                assert_paint(&harness, initial.translated(rect.x, rect.y), 1.0_f32);
                frame!(0.15);
                assert_paint(&harness, initial.translated(rect.x, rect.y), 0.5_f32);
                continue;
            }
            assert!(bars(&overlay(&harness)).is_empty());
            assert!(!scroll_area!().needs_update());
            assert_eq!(tree.hit_test(gutter.x, gutter.y).unwrap().widget_id, content_owner);
            route!(down(gutter));
            let capture = |domains: &InteractionDomains| {
                domains.primary_state().pointer_capture_manager.current_capture(PointerId::MOUSE)
            };
            assert_eq!(capture(&domains), Some(content_owner));
            assert!(!scroll_area!().scrollbar_v.dragging);
            route!(up(gutter));
            assert_eq!(capture(&domains), None);
            assert_eq!(
                tree.node(content).unwrap().widget.as_any().downcast_ref::<RailContent>().unwrap().presses,
                1
            );
            frame!(0.0);
            assert!(bars(&overlay(&harness)).is_empty());

            route!(wheel(center, -70.0, ScrollSource::Wheel));
            frame!(0.0);
            assert!(scroll_area!().target_scroll_y() > 0.0);
            let half_body = tree.get_viewport(area).unwrap();
            assert_paint(&harness, local_layout!().translated(unscrolled.x, unscrolled.y), 1.0_f32);
            assert_eq!(tree.hit_test(gutter.x, gutter.y).unwrap().widget_id, owner);
            // Scroll interpolation advances per frame. Let it settle before
            // asserting that the scrollbar lifetime alone leaves paint clean.
            for _ in 0..20 {
                if !scroll_area!().smooth.needs_animation() {
                    break;
                }
                frame!(0.0);
            }
            assert!(!scroll_area!().smooth.needs_animation());
            frame!(1.5);
            assert_paint(&harness, local_layout!().translated(unscrolled.x, unscrolled.y), 1.0_f32);
            frame!(0.15);
            assert_paint(&harness, local_layout!().translated(unscrolled.x, unscrolled.y), 0.5_f32);
            let settled = frame!(0.15);
            assert!(settled.scene_built && bars(&settled.scene_root).is_empty());
            assert!(bars(&overlay(&harness)).is_empty());
            assert!(!scroll_area!().needs_update());
            assert_eq!(tree.get_viewport(area).unwrap(), half_body);
            assert_eq!(tree.hit_test(gutter.x, gutter.y).unwrap().widget_id, content_owner);
            let hidden = overlay(&harness);
            frame!(2.0);
            assert!(Arc::ptr_eq(&hidden, &overlay(&harness)));

            for y in [1.0, initial.track.height * 0.5, initial.track.height - 1.0] {
                let rail = Point::new(rail_x, rect.y + y);
                let bar = local_layout!();
                assert!(!bar.thumb.contains(Point::new(rail.x - rect.x, y)));
                let moves = tree.node(content).unwrap().widget.as_any().downcast_ref::<RailContent>().unwrap().moves;
                route!(move_pointer(rail, false));
                assert_eq!(
                    tree.node(content).unwrap().widget.as_any().downcast_ref::<RailContent>().unwrap().moves,
                    moves + 1,
                );
                assert_eq!(tree.hit_test(rail.x, rail.y).unwrap().widget_id, content_owner);
                frame!(0.0);
                assert_paint(&harness, local_layout!().translated(unscrolled.x, unscrolled.y), 1.0_f32);
                assert_eq!(tree.hit_test(rail.x, rail.y).unwrap().widget_id, owner);
                let held = overlay(&harness);
                frame!(2.0);
                assert!(Arc::ptr_eq(&held, &overlay(&harness)));
                // The routed hover owner is still the child that saw the Move.
                route!(move_pointer(outside, false));
                frame!(0.0);
                frame!(1.5);
                frame!(0.3);
                assert!(bars(&overlay(&harness)).is_empty());
            }

            route!(move_pointer(gutter, false));
            route!(move_pointer(outside, false));
            frame!(0.0);
            frame!(1.5);
            frame!(0.15);
            assert_paint(&harness, local_layout!().translated(unscrolled.x, unscrolled.y), 0.5_f32);
            route!(move_pointer(gutter, false));
            frame!(0.0);
            assert_paint(&harness, local_layout!().translated(unscrolled.x, unscrolled.y), 1.0_f32);

            if let Some(outer) = outer {
                let before = overlay(&harness);
                jump_outer(&mut tree, outer, 65.0);
                frame!(0.0);
                assert!(Arc::ptr_eq(&before, &overlay(&harness)));
                let shifted = Point::new(rail_x, rect.y - 9.0);
                assert!(shifted.y < unscrolled.y);
                assert_eq!(tree.hit_test(shifted.x, shifted.y).unwrap().widget_id, owner);
                assert_eq!(tree.get_absolute_paint_rect(area).unwrap(), unscrolled);
                continue;
            }

            let bar = local_layout!().translated(rect.x, rect.y);
            let thumb = Point::new(bar.thumb.x + 2.0, bar.thumb.y + bar.thumb.height * 0.5);
            assert_eq!(
                tree.hit_test_for_routing(thumb.x, thumb.y)
                    .map(|(hit, origin)| (hit.widget_id, origin)),
                Some((owner, PointerRouteOrigin::Overlay)),
                "painted thumb {bar:?} and mounted container {rect:?}"
            );
            route!(down(thumb));
            assert_eq!(
                capture(&domains),
                Some(owner),
                "scrollbar drag started: {}",
                scroll_area!().scrollbar_v.dragging
            );
            let below = Point::new(thumb.x, rect.bottom() + 100.0);
            route!(move_pointer(below, true));
            frame!(2.0);
            assert!(scroll_area!().scrollbar_v.dragging);
            assert_paint(&harness, local_layout!().translated(rect.x, rect.y), 1.0_f32);
            assert_eq!(scroll_area!().target_scroll_y(), scroll_area!().state.max_scroll().y);
            route!(up(below));
            assert_eq!(capture(&domains), None);
            assert!(!scroll_area!().scrollbar_v.dragging);
            frame!(0.0);
            frame!(1.5);
            frame!(0.3);
            assert!(bars(&overlay(&harness)).is_empty());

            route!(wheel(center, -40.0, ScrollSource::Touchpad));
            frame!(0.0);
            assert!(bars(&overlay(&harness)).is_empty(), "a clamped scroll does not wake");
            route!(wheel(center, 40.0, ScrollSource::Touchpad));
            frame!(0.0);
            assert_paint(&harness, local_layout!().translated(rect.x, rect.y), 1.0_f32);
            let track = Point::new(rail_x, rect.y + 1.0);
            let previous = scroll_area!().target_scroll_y();
            route!(down(track));
            assert_eq!(capture(&domains), Some(owner));
            assert!(scroll_area!().target_scroll_y() < previous);
            router.cancel_pointer_session_with_tree(
                &mut tree, PointerId::MOUSE, CancellationReason::SystemCancel, domains.primary_state_mut(),
            );
            assert!(!scroll_area!().scrollbar_v.dragging && capture(&domains).is_none());
            frame!(0.0);

            tree.node_mut(content).unwrap().widget.as_any_mut().downcast_mut::<RailContent>()
                .unwrap().height = 90.0;
            tree.mark_layout_dirty(content);
            frame!(0.0);
            route!(move_pointer(gutter, false));
            frame!(0.0);
            assert!(bars(&overlay(&harness)).is_empty());
            assert!(!scroll_area!().needs_update());
        }
    }

    #[test]
    fn scroll_area_vertical_overflow_does_not_invent_horizontal_range() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
        tree.set_flex(scroll, FlexParams::flexible(1.0));
        let dashboard = tree.add_child(scroll, Box::new(Flex::column()), None, 0);
        let group = tree.add_child(
            dashboard,
            Box::new(GroupBox::new(NodeId(0), "System Gauges")),
            None,
            0,
        );
        let content = tree.add_child(group, Box::new(Flex::column().with_spacing(8.0)), None, 0);
        crate::replace_widget!(tree, group, GroupBox::new(content, "System Gauges"));
        let mut first_gauge = None;
        for (row, labels) in [&["CPU", "MEMORY", "DISK I/O"][..], &["NETWORK", "GPU"][..]]
            .into_iter()
            .enumerate()
        {
            let row = tree.add_child(
                content,
                Box::new(Flex::row().with_spacing(8.0)),
                None,
                row as i16,
            );
            for (index, label) in labels.iter().enumerate() {
                let gauge = tree.add_child(
                    row,
                    Box::new(Gauge::new().with_config(GaugeConfig {
                        label: Some((*label).to_owned()),
                        ..Default::default()
                    })),
                    None,
                    index as i16,
                );
                tree.set_flex(gauge, FlexParams::flexible(1.0));
                first_gauge.get_or_insert(gauge);
            }
        }
        tree.node_mut(scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(dashboard);

        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let mut text = FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        // A real resize gives the scroll owner prior vertical-overflow metrics.
        for width in [806.0, 800.0] {
            harness.frame(
                &mut tree,
                root,
                Constraints::tight(Size::new(width, 600.0)),
                &theme,
                &mut text,
                &icons,
            );
        }
        let bounds = tree.get_rect(scroll).unwrap();
        let content_size = tree.get_content_size(dashboard).unwrap();
        assert!(content_size.width <= bounds.width);
        assert!(content_size.height > bounds.height);
        let gauge = tree.get_absolute_paint_rect(first_gauge.unwrap()).unwrap();
        let mut router = EventRouter::new();
        let mut domains = InteractionDomains::new();
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        router.route_input_event(
            &mut tree,
            FusedInputEvent::Scroll {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(gauge.x + gauge.width / 2.0, gauge.y + gauge.height / 2.0),
                delta: (-40.0, -40.0),
                source: ScrollSource::Wheel,
            },
            Some(root),
            &mut text,
            None,
            domains.primary_state_mut(),
            &mut bindings,
        );
        {
            let _update = UpdateAuthorityGuard::enter();
            tree.update_scroll_animations(1.0 / 60.0);
        }
        let area = tree
            .node(scroll)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ScrollArea>()
            .unwrap();
        let scroll = area.scroll_container().unwrap();
        eprintln!(
            "Gauges allocation={bounds:?}, content={content_size:?}, domain={:?}, max={:?}, target=({}, {}), visual={:?}",
            scroll.viewport_size,
            scroll.max_scroll(),
            area.target_scroll_x(),
            area.target_scroll_y(),
            area.scroll_offset(),
        );
        assert!(area.target_scroll_y() > 0.0 && area.visual_scroll_y() > 0.0);
        assert_eq!(
            area.target_scroll_x(),
            0.0,
            "fitting content must not acquire X movement"
        );
        assert_eq!(area.horizontal_scroll_offset(), 0.0);
        assert_eq!(scroll.max_scroll().0, 0.0);
    }
}
