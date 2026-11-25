use crate::compositor::{Compositor, SceneKey, SurfaceId};
use crate::core::appearance::FrameworkAppearanceSnapshot;
use crate::core::event::{
    FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, RawInputEvent,
    SessionQualifiedInput, WindowId,
};
#[cfg(any(target_arch = "wasm32", test))]
use crate::core::event_router::browser_text::{
    BrowserTextAction, BrowserTextReport, BrowserTextSession, BrowserTextSource,
};
use crate::core::event_router::{EventRouter, RelativePointerWindowFacts};
use crate::core::focus_authority::FocusResolver;
use crate::core::frame_projection_binding_registry::{
    FrameProjectionAuthorityAccess, FrameProjectionBinding, FrameProjectionBindingRegistry,
    FrameProjectionRuntimeAccess, FrameProjectionWindowAccess, NavigationProjectionSource,
    ScreenHostProjectionKey, pull_navigation_frame_projection,
};
use crate::core::frame_realization::{
    AppFrameAttempts, AppFrameCompileOutcome, AppFrameObserver, AppFrameRecorder,
    RequiredTextFaultGuard,
};
use crate::core::input_fusion::InputFusion;
use crate::core::layout::Constraints;
use crate::core::layout_transition::{
    ActiveLayoutTransitions, AuthorizationContext, InterpolatedVisualRects,
    compute_current_layout_interpolated_visual_rects, resolve_layout_transition_candidates,
};
#[cfg(test)]
use crate::core::layout_transition::compute_interpolated_visual_rects;
use crate::core::math::{Point, RectF, Size};
use crate::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationSurfaceOrchestrator, SceneKeyAllocator,
};
use crate::core::overlay_domain_coordinator::OverlayDomainCoordinator;
use crate::core::overlay_root::{DismissPolicy, OverlayRoot};
use crate::core::rect_authority::{
    LayoutAuthorityGuard, PaintAuthorityGuard, TransitionAuthorityGuard, UpdateAuthorityGuard,
};
use crate::core::relative_pointer::RelativePointerCancellationReason;
use crate::core::scene::{
    RecycledSceneStorage, SceneBuilder, SceneFrame, SceneGraph, SceneNode, SceneSwapchain,
};
use crate::core::scene_diff::{
    BasicCacheConsumer, CacheConsumers, SceneCache, SceneCacheExecutionMode,
    compute_scene_diff_for_cache, dispatch_cache_operations,
};
#[cfg(debug_assertions)]
use crate::core::scene_diff::{assert_identity_stability, trace_identity_changes};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::scheduler::HighFreqScheduler;
use crate::core::scroll_link::ScrollLinkCoordinator;
use crate::core::text_engine::TextEngine;
use crate::core::theme::Theme;
use crate::core::transition::TransitionLayer;
use crate::core::widget::{
    LayoutContext, OverlayHitOutput, PaintContext, ScrollSyncInvalidation, UpdateContext,
};
use crate::core::widget_tree::{NodeId, OverflowMode, OverlayHitEntry, WidgetDirty, WidgetTree};
use crate::model::navigation_host::{NavigationHost, NavigationNodeId};
use crate::model::notification::{
    NotificationAction, NotificationHandle, NotificationLevel, NotificationManager,
};
use crate::model::signal_store::SignalStore;
use crate::model::toast::{ToastAction, ToastHandle, ToastManager};
use crate::prelude::ScrollArea;
use crate::renderer::custom_render_prepare::{
    CustomRenderPresentationBindings, CustomRenderTargetSnapshot,
};
use crate::renderer::image_binding::{
    ImageBinding, ImageBindingIdentity, ImageBindingInvalidations,
};
use crate::renderer::sdf_frame_custody::{SdfFramePlan, SdfSceneCustody};
use crate::renderer::sdf_material_status::SdfMaterialPublication;
use crate::renderer::wgpu_renderer::WgpuRenderer;
use crate::runtime::ActionRegistry;
use crate::utils::id::TreeWidgetId;
use crate::widgets::screen_host::ScreenHost;
use crate::widgets::toast::ToastWidget;
use crate::platform::window::AppLoopEvent;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use web_time::{Duration, Instant};

mod cursor_output;
mod ui_scale;
#[cfg(target_arch = "wasm32")]
mod web_navigation;
#[cfg(test)]
mod web_navigation_tests;
#[cfg(target_arch = "wasm32")]
pub use web_navigation::{WebNavigation, WebNavigationIntentSink, WebNavigationRequestError, WebRouteError};
pub use cursor_output::UiThreadCursorOutput;
use ui_scale::UiScaleState;
pub use ui_scale::{
    UiScale, UiScaleEffectiveSnapshot, UiScaleReceipt, UiScaleRejection, UiScaleRequestOutcome,
    UiScaleRequestStatus, UiScaleStatus, UiScaleTargetFailure, UiScaleTargetOutput,
    UiScaleTargetStatus, UiScaleValueError,
};

// ============================================================================
// SPEC-HAL-131A Stage 16: Anchor Resolution Phase Tracking
// ============================================================================
//
// Thread-local flag to detect if realization is attempted during anchor resolution.
// INV-SCROLL-DIRTY-3: Anchor resolution MUST NOT trigger realization.
//
// Set to true during post_layout_resolve_scroll_anchors(), false otherwise.
// List::layout() checks this flag and panics if true (debug builds only).

thread_local! {
    static ANCHOR_RESOLUTION_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

#[derive(Clone)]
struct SharedSceneKeyAllocator {
    next: Rc<Cell<u64>>,
}

impl SharedSceneKeyAllocator {
    fn new(start: u64) -> Self {
        Self {
            next: Rc::new(Cell::new(start)),
        }
    }

    fn allocate_key(&self) -> SceneKey {
        let key = SceneKey::new(self.next.get());
        self.next.set(key.get() + 1);
        key
    }
}

impl SceneKeyAllocator for SharedSceneKeyAllocator {
    fn allocate(&mut self) -> SceneKey {
        self.allocate_key()
    }
}

#[cfg(test)]
#[path = "../tests/app/shared_scene_key_allocator_tests.rs"]
mod shared_scene_key_allocator_tests;

// ============================================================================
// DEBUG: Layout Epoch Tracking for Paint-Without-Layout Detection
// ============================================================================
//
// INV-PAINT-REQUIRES-LAYOUT-EPOCH: A node may not be painted unless laid out
// in the current layout epoch.
//
// Incremented at start of layout_tree(). Stamped on nodes in layout_node().
// Asserted in paint_node() to catch paint-without-layout violations.

#[cfg(any(debug_assertions, test))]
thread_local! {
    static LAYOUT_EPOCH: Cell<u64> = const { Cell::new(0) };
}

// ============================================================================
// SINGLE-LAYOUT-PER-FRAME INVARIANT ENFORCEMENT
// ============================================================================
thread_local! {
    /// Count of layout_tree calls in current frame
    static LAYOUT_TREE_CALLS_THIS_FRAME: Cell<u32> = const { Cell::new(0) };
}

// ============================================================================
// SPEC-HAL-161: SIGNAL DELIVERY PHASE GUARD
// ============================================================================
//
// INV-XWIN-003: Signal delivery must not occur during layout/scene/paint.
// This thread-local tracks whether we're in a "frame critical section" where
// signal delivery is forbidden.
//
// Set while layout/scene/paint is active. deliver_signals() asserts this is
// false in debug builds.

thread_local! {
    /// True when inside layout/scene/paint (signal delivery forbidden)
    static IN_FRAME_CRITICAL_SECTION: Cell<bool> = const { Cell::new(false) };
}

/// Enter the frame critical section (layout/scene/paint).
/// Signal delivery is forbidden while this is active.
#[inline]
fn frame_critical_section_enter() {
    IN_FRAME_CRITICAL_SECTION.with(|c| c.set(true));
}

/// Exit the frame critical section.
#[inline]
fn frame_critical_section_exit() {
    IN_FRAME_CRITICAL_SECTION.with(|c| c.set(false));
}

fn apply_window_cursor(
    window: &winit::window::Window,
    event_router: &mut EventRouter,
    apply_default: bool,
    relative_locked: bool,
) {
    apply_event_router_cursor(
        event_router,
        apply_default,
        relative_locked,
        |visible, icon| {
            if let Some(visible) = visible {
                window.set_cursor_visible(visible);
            }
            if let Some(icon) = icon {
                window.set_cursor(crate::core::event::cursor_to_winit(icon));
            }
        },
    );
}

fn apply_event_router_cursor(
    event_router: &mut EventRouter,
    apply_default: bool,
    relative_locked: bool,
    mut apply: impl FnMut(Option<bool>, Option<crate::core::event::CursorIcon>),
) {
    use crate::core::event::CursorIcon;
    let explicit_request = event_router.take_requested_cursor();

    if relative_locked {
        // The relative lease owns visibility until its platform terminator runs.
        event_router.cursor_hidden_sticky = false;
        return;
    }

    if let Some(CursorIcon::Hidden) = explicit_request {
        if !event_router.cursor_hidden_sticky {
            apply(Some(false), None);
            event_router.cursor_hidden_sticky = true;
        }
    } else if let Some(cursor) = explicit_request {
        let visible = event_router.cursor_hidden_sticky.then_some(true);
        event_router.cursor_hidden_sticky = false;
        apply(visible, Some(cursor));
    } else if apply_default && !event_router.cursor_hidden_sticky {
        apply(None, Some(CursorIcon::Default));
    }
}

fn cursor_default_event(event: &FusedInputEvent, pointer_captured: bool) -> bool {
    // Scroll routes to its scroll owner; derivative drags route to Capture.
    // Neither establishes a new unclaimed hover position.
    event.window_id() == WindowId::PRIMARY
        && match event {
            FusedInputEvent::PointerMove { .. } | FusedInputEvent::PointerUp { .. } => true,
            // A further button press stays with Capture and does not replace its cursor.
            FusedInputEvent::PointerDown { .. } => !pointer_captured,
            _ => false,
        }
}

fn publish_ui_cursor(
    output: Option<&cursor_output::CursorOutputSender>,
    event_router: &mut EventRouter,
    apply_default: bool,
    relative_locked: bool,
) {
    if let Some(output) = output {
        apply_event_router_cursor(
            event_router,
            apply_default,
            relative_locked,
            |visible, icon| {
                output.publish(visible, icon);
            },
        );
    }
}

struct FrameCriticalSectionGuard;

impl FrameCriticalSectionGuard {
    #[inline]
    fn enter() -> Self {
        frame_critical_section_enter();
        Self
    }
}

impl Drop for FrameCriticalSectionGuard {
    fn drop(&mut self) {
        frame_critical_section_exit();
    }
}

/// Check if we're currently in a frame critical section.
/// Used by deliver_signals() to enforce INV-XWIN-003.
#[inline]
pub fn in_frame_critical_section() -> bool {
    IN_FRAME_CRITICAL_SECTION.with(|c| c.get())
}

/// Called at start of each frame to reset layout_tree call counter
fn frame_begin_layout_tracking() {
    LAYOUT_TREE_CALLS_THIS_FRAME.with(|c| c.set(0));
}

#[cfg(test)]
pub(crate) fn test_begin_frame_layout_tracking() {
    frame_begin_layout_tracking();
}

/// Called at start of layout_tree to count and assert single-call invariant
fn layout_tree_call_enter() -> u32 {
    LAYOUT_TREE_CALLS_THIS_FRAME.with(|c| {
        let count = c.get() + 1;
        c.set(count);
        count
    })
}

/// DEBUG: Get current layout epoch for stamping/checking.
#[cfg(any(debug_assertions, test))]
#[inline]
pub fn current_layout_epoch() -> u64 {
    LAYOUT_EPOCH.with(|e| e.get())
}

/// DEBUG: Increment layout epoch at start of layout pass.
#[cfg(any(debug_assertions, test))]
#[inline]
fn increment_layout_epoch() {
    LAYOUT_EPOCH.with(|e| e.set(e.get().wrapping_add(1)));
}

/// SPEC-HAL-131A Stage 16: Check if anchor resolution is currently active.
///
/// Used by realization code (e.g., List::layout()) to assert that realization
/// is not being triggered during anchor resolution phase.
///
/// INV-SCROLL-DIRTY-3: Anchor resolution MUST NOT trigger realization.
#[inline]
pub fn is_anchor_resolution_active() -> bool {
    ANCHOR_RESOLUTION_ACTIVE.with(|f| f.get())
}

// ============================================================================
// HAL_PROFILE_RESIZE: Performance counters for resize analysis
// ============================================================================
mod resize_profiler {
    use std::cell::RefCell;
    use web_time::Instant;

    #[derive(Default)]
    pub struct ResizeCounters {
        pub resize_events: u32,
        pub mouse_move_events: u32,
        pub layout_passes: u32,
        pub layout_wrap_calls: u32,
        pub layout_child_calls: u32,
        pub relayout_child_calls: u32,
        pub text_measure_calls: u32,
        pub layout_time_us: u64,
        pub text_time_us: u64,
        pub frames: u32,
        pub local_paints: u32,
        /// HAL_AUDIT_INVALIDATION: count of mark_layout_dirty calls
        pub invalidation_count: u32,
        /// HAL_AUDIT_INVALIDATION: count of invalidations while LayoutAuthorityGuard held
        pub invalidation_during_layout: u32,
        /// Pipeline stage: build_scene (layout + paint)
        pub build_scene_time_us: u64,
        /// Pipeline stage: draw_plan_builder.build_multi
        pub draw_plan_time_us: u64,
        /// Pipeline stage: renderer.compile_multi (GPU buffer build)
        pub compile_time_us: u64,
        /// Pipeline stage: renderer.render_multi (command encoding)
        pub render_time_us: u64,
        /// Pipeline stage: queue.submit + present
        pub submit_time_us: u64,
        last_report: Option<Instant>,
    }

    impl ResizeCounters {
        pub fn new() -> Self {
            Self {
                last_report: Some(Instant::now()),
                ..Default::default()
            }
        }

        pub fn maybe_report(&mut self) -> bool {
            let now = Instant::now();
            if let Some(last) = self.last_report {
                if now.duration_since(last).as_secs_f32() >= 1.0 {
                    let fps = self.frames as f32;
                    // Pipeline timing breakdown (per-second totals in ms)
                    eprintln!(
                        "[HAL_PROFILE_RESIZE] fps={:.0} | build_scene={:.1}ms draw_plan={:.1}ms compile={:.1}ms render={:.1}ms submit={:.1}ms | \
                         layout={:.1}ms text={:.1}ms | resize_ev={} layout_pass={} local_paints={}",
                        fps,
                        self.build_scene_time_us as f32 / 1000.0,
                        self.draw_plan_time_us as f32 / 1000.0,
                        self.compile_time_us as f32 / 1000.0,
                        self.render_time_us as f32 / 1000.0,
                        self.submit_time_us as f32 / 1000.0,
                        self.layout_time_us as f32 / 1000.0,
                        self.text_time_us as f32 / 1000.0,
                        self.resize_events,
                        self.layout_passes,
                        self.local_paints,
                    );
                    // Reset counters
                    self.resize_events = 0;
                    self.mouse_move_events = 0;
                    self.layout_passes = 0;
                    self.layout_wrap_calls = 0;
                    self.layout_child_calls = 0;
                    self.relayout_child_calls = 0;
                    self.text_measure_calls = 0;
                    self.layout_time_us = 0;
                    self.text_time_us = 0;
                    self.frames = 0;
                    self.local_paints = 0;
                    self.invalidation_count = 0;
                    self.invalidation_during_layout = 0;
                    self.build_scene_time_us = 0;
                    self.draw_plan_time_us = 0;
                    self.compile_time_us = 0;
                    self.render_time_us = 0;
                    self.submit_time_us = 0;
                    self.last_report = Some(now);
                    return true;
                }
            }
            false
        }
    }

    thread_local! {
        pub static COUNTERS: RefCell<ResizeCounters> = RefCell::new(ResizeCounters::new());
    }

    pub fn is_enabled() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var_os("HAL_PROFILE_RESIZE").is_some())
    }

    pub fn inc_resize_event() {
        if is_enabled() {
            COUNTERS.with(|c| c.borrow_mut().resize_events += 1);
        }
    }

    pub fn inc_mouse_move_event() {
        if is_enabled() {
            COUNTERS.with(|c| c.borrow_mut().mouse_move_events += 1);
        }
    }

    pub fn inc_layout_pass() {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.layout_passes += 1;
            });
        }
    }

    pub fn inc_local_paint() {
        if is_enabled() {
            COUNTERS.with(|c| c.borrow_mut().local_paints += 1);
        }
    }

    pub fn inc_layout_wrap_call() {
        if is_enabled() {
            COUNTERS.with(|c| c.borrow_mut().layout_wrap_calls += 1);
        }
    }

    pub fn inc_layout_child_call() {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.layout_child_calls += 1;
            });
        }
    }

    pub fn inc_relayout_child_call() {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.relayout_child_calls += 1;
            });
        }
    }

    pub fn inc_text_measure_call() {
        if is_enabled() {
            COUNTERS.with(|c| c.borrow_mut().text_measure_calls += 1);
        }
    }

    pub fn add_layout_time_us(us: u64) {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.layout_time_us += us;
            });
        }
    }

    pub fn add_text_time_us(us: u64) {
        if is_enabled() {
            COUNTERS.with(|c| c.borrow_mut().text_time_us += us);
        }
    }

    pub fn add_build_scene_time_us(us: u64) {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.build_scene_time_us += us;
            });
        }
    }

    pub fn add_draw_plan_time_us(us: u64) {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.draw_plan_time_us += us;
            });
        }
    }

    pub fn add_compile_time_us(us: u64) {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.compile_time_us += us;
            });
        }
    }

    pub fn add_render_time_us(us: u64) {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.render_time_us += us;
            });
        }
    }

    pub fn add_submit_time_us(us: u64) {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut c = c.borrow_mut();
                c.submit_time_us += us;
            });
        }
    }

    pub fn inc_frame() {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut counters = c.borrow_mut();
                counters.frames += 1;
                counters.maybe_report();
            });
        }
    }

    /// HAL_AUDIT_INVALIDATION: Track layout invalidation calls
    pub fn inc_invalidation(during_layout: bool) {
        if is_enabled() {
            COUNTERS.with(|c| {
                let mut counters = c.borrow_mut();
                counters.invalidation_count += 1;
                if during_layout {
                    counters.invalidation_during_layout += 1;
                }
            });
        }
    }
}

pub use resize_profiler::{
    add_build_scene_time_us as profiler_add_build_scene_time_us,
    add_compile_time_us as profiler_add_compile_time_us,
    add_draw_plan_time_us as profiler_add_draw_plan_time_us,
    add_layout_time_us as profiler_add_layout_time_us,
    add_render_time_us as profiler_add_render_time_us,
    add_submit_time_us as profiler_add_submit_time_us,
    add_text_time_us as profiler_add_text_time_us, inc_invalidation as profiler_inc_invalidation,
    inc_layout_child_call as profiler_inc_layout_child_call,
    inc_layout_pass as profiler_inc_layout_pass,
    inc_layout_wrap_call as profiler_inc_layout_wrap_call,
    inc_mouse_move_event as profiler_inc_mouse_move_event,
    inc_relayout_child_call as profiler_inc_relayout_child_call,
    inc_resize_event as profiler_inc_resize_event,
    inc_text_measure_call as profiler_inc_text_measure_call,
};

// ============================================================================
// HAL_DUMP_LAYOUT / HAL_ASSERT_CONTAINMENT: Layout analysis tools
// ============================================================================

fn is_dump_layout_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("HAL_DUMP_LAYOUT").is_some())
}

fn is_assert_containment_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("HAL_ASSERT_CONTAINMENT").is_some())
}

/// Dump layout snapshot for tracked nodes (throttled to once per second)
pub fn dump_layout_snapshot(tree: &WidgetTree, root: NodeId) {
    if !is_dump_layout_enabled() {
        return;
    }

    static LAST_DUMP: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
    let now = Instant::now();
    {
        let mut last = LAST_DUMP.lock().unwrap();
        if let Some(t) = *last {
            if now.duration_since(t).as_secs_f32() < 1.0 {
                return;
            }
        }
        *last = Some(now);
    }

    eprintln!("\n[HAL_DUMP_LAYOUT] === Layout Snapshot ===");
    dump_node_recursive(tree, root, 0);
    eprintln!("[HAL_DUMP_LAYOUT] === End Snapshot ===\n");
}

fn dump_node_recursive(tree: &WidgetTree, node_id: NodeId, depth: usize) {
    let indent = "  ".repeat(depth);
    if let Some(node) = tree.node(node_id) {
        let type_name = node.widget.debug_type_name();
        if let Some(rect) = tree.get_rect(node_id) {
            let abs_rect = tree
                .get_absolute_paint_rect(node_id)
                .unwrap_or(RectF::zero());
            eprintln!(
                "[HAL_DUMP_LAYOUT] {}{} {:?} rect=({:.1},{:.1},{:.1},{:.1}) abs=({:.1},{:.1},{:.1},{:.1})",
                indent,
                type_name,
                node_id,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                abs_rect.x,
                abs_rect.y,
                abs_rect.width,
                abs_rect.height
            );
        }
    }

    for child_id in tree.children(node_id) {
        dump_node_recursive(tree, child_id, depth + 1);
    }
}

/// Assert containment: every child must be fully contained by its parent rect
pub fn assert_containment(tree: &WidgetTree, root: NodeId) {
    if !is_assert_containment_enabled() {
        return;
    }

    check_containment_recursive(tree, root);
}

fn check_containment_recursive(tree: &WidgetTree, node_id: NodeId) {
    let parent_rect = match tree.get_rect(node_id) {
        Some(r) => r,
        None => return,
    };
    let parent_type = tree
        .node(node_id)
        .map(|n| n.widget.debug_type_name())
        .unwrap_or("Unknown");

    const EPSILON: f32 = 0.5;

    for child_id in tree.children(node_id) {
        if let Some(child_rect) = tree.get_rect(child_id) {
            let child_type = tree
                .node(child_id)
                .map(|n| n.widget.debug_type_name())
                .unwrap_or("Unknown");

            // Check each edge
            let violations = [
                (child_rect.x < -EPSILON, "left"),
                (child_rect.y < -EPSILON, "top"),
                (
                    child_rect.x + child_rect.width > parent_rect.width + EPSILON,
                    "right",
                ),
                (
                    child_rect.y + child_rect.height > parent_rect.height + EPSILON,
                    "bottom",
                ),
            ];

            for (violated, edge) in violations {
                if violated {
                    eprintln!(
                        "\n[HAL_ASSERT_CONTAINMENT] VIOLATION: {} edge\n\
                         Parent: {} {:?} rect=({:.1},{:.1},{:.1},{:.1})\n\
                         Child:  {} {:?} rect=({:.1},{:.1},{:.1},{:.1})\n",
                        edge,
                        parent_type,
                        node_id,
                        0.0,
                        0.0,
                        parent_rect.width,
                        parent_rect.height,
                        child_type,
                        child_id,
                        child_rect.x,
                        child_rect.y,
                        child_rect.width,
                        child_rect.height
                    );
                    // Only report first violation
                    return;
                }
            }
        }

        // Recurse into children
        check_containment_recursive(tree, child_id);
    }
}

// ============================================================================
// SPEC-HAL-042: Frame Loop & Engine Ticker
// SPEC-HAL-043: Frame Timers, Throttling, Idle Frames (Phase 1: API surface)
// ============================================================================

/// SPEC-HAL-043: Reason why the current frame was triggered.
/// Used for throttling decisions and debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameReason {
    /// Frame triggered by user input (mouse, keyboard, etc.)
    Input,
    /// Frame triggered by an active animation
    Animation,
    /// Frame triggered by a timer expiration
    Timer,
    /// Frame explicitly forced (startup, resize, window redraw) - MUST render
    Forced,
    /// Default platform tick with no specific event - can lead to idle after 2 frames
    #[default]
    Tick,
}

/// SPEC-HAL-043 Phase 2: Engine activity state for idle detection.
/// Tracks whether the engine has meaningful work or is idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineActivity {
    /// Engine has active input, animations, or timers
    #[default]
    Active,
    /// Engine is idle (consecutive Forced frames, no activity)
    Idle,
}

/// SPEC-HAL-043 Phase 3: Target frame interval when idle (~30 FPS).
/// Render is throttled to this rate when engine is idle to save CPU/GPU.
const IDLE_FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// SPEC-HAL-354: Fallback cadence when the platform cannot report a display rate.
const DEFAULT_ACTIVE_REFRESH_RATE_MILLIHERTZ: u32 = 120_000;

fn frame_interval_for_refresh_rate(refresh_rate_millihertz: Option<u32>) -> Duration {
    let refresh_rate_millihertz = refresh_rate_millihertz
        .filter(|rate| *rate > 0)
        .unwrap_or(DEFAULT_ACTIVE_REFRESH_RATE_MILLIHERTZ);
    let rate = u64::from(refresh_rate_millihertz);
    let nanos = (1_000_000_000_000_u64 + rate / 2) / rate;
    Duration::from_nanos(nanos.max(1))
}

/// Owns the stable platform deadline for continuous engine work.
///
/// A deadline persists across event-loop callbacks. Recomputing `now + interval`
/// at callback entry and then immediately requesting redraw from `AboutToWait`
/// bypasses `WaitUntil` and permits an update-only widget to spin the loop.
#[derive(Debug)]
pub(crate) struct ContinuousFramePacer {
    next_tick: Option<Instant>,
    frame_interval: Duration,
}

impl Default for ContinuousFramePacer {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ContinuousFramePacer {
    pub(crate) fn new(refresh_rate_millihertz: Option<u32>) -> Self {
        Self {
            next_tick: None,
            frame_interval: frame_interval_for_refresh_rate(refresh_rate_millihertz),
        }
    }

    pub(crate) fn set_refresh_rate_millihertz(&mut self, refresh_rate_millihertz: Option<u32>) {
        let frame_interval = frame_interval_for_refresh_rate(refresh_rate_millihertz);
        if self.frame_interval != frame_interval {
            self.frame_interval = frame_interval;
            self.next_tick = None;
        }
    }

    pub(crate) fn deadline(&mut self, now: Instant, needed: bool) -> Option<Instant> {
        if !needed {
            self.next_tick = None;
            return None;
        }

        Some(*self.next_tick.get_or_insert(now + self.frame_interval))
    }

    pub(crate) fn take_due(&mut self, now: Instant, needed: bool) -> bool {
        let Some(deadline) = self.deadline(now, needed) else {
            return false;
        };
        if now < deadline {
            return false;
        }

        // Preserve the display-rate phase so ordinary WaitUntil wake latency does
        // not accumulate into a slower cadence. A real stall skips every elapsed
        // period in one step, so no catch-up redraw burst is emitted.
        let elapsed = now.saturating_duration_since(deadline);
        let periods = elapsed
            .as_nanos()
            .checked_div(self.frame_interval.as_nanos())
            .unwrap_or(0)
            .saturating_add(1);
        self.next_tick = u32::try_from(periods)
            .ok()
            .and_then(|periods| self.frame_interval.checked_mul(periods))
            .and_then(|advance| deadline.checked_add(advance))
            .or_else(|| now.checked_add(self.frame_interval));
        true
    }
}

/// Per-frame time snapshot used by all engine subsystems.
/// SPEC-HAL-042: Single source of truth for frame timing.
/// SPEC-HAL-043: Extended with frame reason.
#[derive(Debug, Clone, Copy)]
pub struct FrameTime {
    /// Current frame timestamp
    pub now: Instant,
    /// Time since previous frame (clamped to 0 if negative)
    pub dt: Duration,
    /// Monotonic frame counter
    pub frame_index: u64,
    /// SPEC-HAL-043: Why this frame was triggered
    pub reason: FrameReason,
}

/// SPEC-HAL-042: Centralized frame timing for the engine.
/// SPEC-HAL-043: Extended with frame reason, activity, and throttling.
/// Ensures all subsystems use the same `now` and `dt`.
pub struct EngineTicker {
    /// SPEC-HAL-101 Stage-4: Engine start time for frame_time_ms computation
    start_time: Instant,
    last_frame_time: Instant,
    frame_index: u64,
    /// SPEC-HAL-043: Pending reason for the next frame
    pending_reason: FrameReason,
    /// SPEC-HAL-043 Phase 2: Previous frame's reason (for Forced→Forced detection)
    last_frame_reason: FrameReason,
    /// SPEC-HAL-043 Phase 2: Current engine activity state
    activity: EngineActivity,
    /// SPEC-HAL-043 Phase 3: Last time we actually rendered (for throttling)
    last_render_time: Instant,
}

impl EngineTicker {
    /// Create a new ticker with the given start time.
    /// Note: last_render_time is initialized in the past so the first frame always renders.
    pub fn new(start: Instant) -> Self {
        Self {
            start_time: start,
            last_frame_time: start,
            frame_index: 0,
            pending_reason: FrameReason::Forced,
            last_frame_reason: FrameReason::Forced,
            activity: EngineActivity::Active, // Start active, settle to idle if no work
            last_render_time: start - IDLE_FRAME_INTERVAL, // Ensure first frame renders
        }
    }

    /// SPEC-HAL-101 Stage-4: Get frame time in milliseconds since engine start.
    /// Used for caret blinking and other time-based widget state.
    pub fn frame_time_ms(&self, now: Instant) -> u64 {
        now.duration_since(self.start_time).as_millis() as u64
    }

    /// Full-precision monotonic time for Timeline evaluation.
    pub(crate) fn timeline_time_secs(&self, now: Instant) -> f64 {
        now.duration_since(self.start_time).as_secs_f64()
    }

    /// SPEC-HAL-043: Notify the ticker of why the next frame should run.
    /// Sets the reason that will be attached to the next FrameTime.
    /// Multiple calls before begin_frame() will overwrite; last reason wins.
    /// SPEC-HAL-043 Phase 2: Non-Tick reasons immediately mark engine as Active.
    pub fn notify_reason(&mut self, reason: FrameReason) {
        self.pending_reason = reason;
        // Any explicit reason (Input/Animation/Timer/Forced) marks us active
        // Only Tick (default platform tick) can lead to idle
        if reason != FrameReason::Tick {
            self.activity = EngineActivity::Active;
        }
    }

    /// SPEC-HAL-043 Phase 2: Returns true if the engine is currently idle.
    /// Idle means consecutive Tick frames with no input/animation/timer/forced activity.
    pub fn is_idle(&self) -> bool {
        self.activity == EngineActivity::Idle
    }

    /// SPEC-HAL-043 Phase 2: Returns the current engine activity state.
    pub fn activity(&self) -> EngineActivity {
        self.activity
    }

    /// SPEC-HAL-043 Phase 3: Check if rendering should be throttled this frame.
    /// Returns true if we should skip rendering (idle + interval not met).
    /// Returns false if we should render (active, or enough time passed).
    pub fn should_throttle(&self, now: Instant) -> bool {
        // Always render when active
        if self.activity != EngineActivity::Idle {
            return false;
        }
        // When idle, render only if enough time has passed since last render
        now.duration_since(self.last_render_time) < IDLE_FRAME_INTERVAL
    }

    /// Begin a new frame. Returns timing info for all subsystems.
    /// Call this once at the start of each genuinely separate frame, before any
    /// executing frame phase. This also initializes the frame's layout accounting.
    /// Pass the returned timing to scene consumers; they do not begin a frame.
    ///
    /// SPEC-HAL-041: This is the ONLY place that calls Instant::now() for frame timing.
    /// All subsystems must use the returned FrameTime, never call Instant::now() directly.
    ///
    /// SPEC-HAL-043: Includes the pending reason and resets it to Tick.
    /// SPEC-HAL-043 Phase 2: Updates activity state based on Tick→Tick detection.
    /// NOTE: Does NOT update last_render_time; caller must call mark_rendered().
    pub fn begin_frame(&mut self) -> FrameTime {
        let now = Instant::now();
        self.begin_frame_at(now)
    }

    /// Begin a new frame with a specific timestamp (for testing).
    ///
    /// Production code should use `begin_frame()` which calls `Instant::now()` internally.
    /// This method is provided for deterministic testing only.
    /// It has the same once-per-frame accounting and phase contract as `begin_frame()`.
    pub fn begin_frame_at(&mut self, now: Instant) -> FrameTime {
        #[cfg(debug_assertions)]
        debug_assert!(
            !in_frame_critical_section()
                && !crate::core::rect_authority::is_update_authority_active()
                && !crate::core::rect_authority::is_layout_authority_active()
                && !crate::core::rect_authority::is_transition_authority_active()
                && !crate::core::rect_authority::is_paint_authority_active(),
            "INV-FRAME-BEGIN-PHASE: cannot restart a frame from an executing phase"
        );
        frame_begin_layout_tracking();
        let dt = now.saturating_duration_since(self.last_frame_time);
        self.last_frame_time = now;
        let prev_index = self.frame_index;
        self.frame_index = prev_index + 1;
        let reason = self.pending_reason;

        // SPEC-HAL-043 Phase 2: Update activity state
        // Idle ONLY after two consecutive Tick frames (no input/animation/timer/forced)
        // Forced (resize/startup) ALWAYS keeps engine Active
        if reason == FrameReason::Tick && self.last_frame_reason == FrameReason::Tick {
            self.activity = EngineActivity::Idle;
        } else {
            self.activity = EngineActivity::Active;
        }

        // Store this frame's reason for next frame's comparison
        self.last_frame_reason = reason;
        // Reset to Tick (default) - only explicit notify_reason() calls change this
        self.pending_reason = FrameReason::Tick;

        FrameTime {
            now,
            dt,
            frame_index: prev_index,
            reason,
        }
    }

    /// SPEC-HAL-043 Phase 3: Must be called after a successful render.
    /// Updates last_render_time for throttling decisions.
    pub fn mark_rendered(&mut self, now: Instant) {
        self.last_render_time = now;
    }
}

fn should_throttle_native_frame(
    ticker: &EngineTicker,
    now: Instant,
    tree: &WidgetTree,
    root: NodeId,
) -> bool {
    // HAL354: a settled paint/composition dependency is work for this already
    // delivered frame. Only authoritative Paint clears it. Do not require a
    // second wake, queue a future reason, or relax throttling for a clean Tick.
    ticker.should_throttle(now) && !tree.node(root).is_some_and(|node| node.dirty_composition)
}

// ============================================================================
// SPEC-HAL-042 Phase-2: Engine Frame Context
// ============================================================================

/// SPEC-HAL-042 Phase-2: Per-frame context for timing coordination.
/// Kept minimal to avoid borrow conflicts with renderer-owned text_engine.
/// Future SPECs (043-046) will extend with timers, throttling, etc.
#[derive(Debug, Clone, Copy)]
pub struct EngineFrameContext {
    pub time: FrameTime,
}

// ============================================================================
// SPEC-HAL-044: Build Scene (CPU-only, no GPU)
// ============================================================================

// ============================================================================
// SPEC-HAL-133: Pre-Layout Linked Scroll Processing
// ============================================================================

/// SPEC-HAL-133: Process linked scroll intents before layout.
///
/// Called between input handling and layout_tree().
/// Drains pending intents from the coordinator and propagates anchors
/// to ScrollArea widgets in the same group.
///
/// INV-LINK-ORDER-1: Occurs before layout_tree()
/// INV-LINK-ORDER-2: Pure scroll-state mutation only
/// INV-LINK-ORDER-3: No anchor resolution (that happens after layout)
/// INV-LINK-ANCHOR-2: Missing anchor target = no-op for that target
///
/// Note: `pub(crate)` for internal engine-path tests only (src/tests/).
/// NOT exposed to external crates (not in prelude, not re-exported).
pub(crate) fn process_linked_scroll_intents(
    coordinator: &mut ScrollLinkCoordinator,
    tree: &mut WidgetTree,
) {
    // Drain all pending intents
    let intents = coordinator.drain_intents();

    if intents.is_empty() {
        return;
    }

    // Phase 1: Collect all ScrollArea nodes with their group info
    let mut all_nodes = Vec::new();
    tree.for_each_node_in_stable_order(|node_id| {
        all_nodes.push(node_id);
    });

    // Phase 2: For each intent, propagate anchor to matching group members
    for (group_id, intent) in intents {
        // INV-LINK-ANCHOR-2: If no anchor, skip (no-op for all targets)
        let Some(anchor) = intent.anchor else {
            continue;
        };

        // SPEC-HAL-133 Phase 2: Capture axis for filtering
        let intent_axis = intent.axis;

        // Find all ScrollAreas in this group and set their anchor
        for &node_id in &all_nodes {
            if let Some(scroll_area) = tree
                .node_mut(node_id)
                .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>())
            {
                // INV-LINK-ORDER-2: mutate the same mounted widget's scroll
                // state; a temporary take/true replacement would retire its
                // concrete mount and every qualified G1 link.
                if scroll_area.scroll_group() == Some(group_id) {
                    // Missing content anchors remain resolver-owned no-ops.
                    scroll_area.set_linked_anchor(anchor, intent_axis);
                }
            }
        }
    }
}

// ============================================================================
// SPEC-HAL-131A Stage 14: Post-Layout Anchor Resolution
// ============================================================================

/// SPEC-HAL-131A Stage 14: Resolve scroll anchors after layout, before paint.
///
/// This is the post-layout phase that resolves scroll anchors for all
/// ScrollArea widgets in the tree. Called between layout_tree and paint_tree.
///
/// SPEC-HAL-131A Stage 14: Engine-owned post-layout anchor resolution.
///
/// Frame ordering: layout_tree() → post_layout_resolve_scroll_anchors() → paint_tree()
///
/// This is engine-owned, NOT a Widget trait hook:
/// - Iterates typed index of ScrollArea nodes
/// - Matches concrete ScrollArea type via downcast
/// - Calls ScrollArea::post_layout_resolve_anchors directly
pub fn post_layout_resolve_scroll_anchors(tree: &mut WidgetTree) {
    use crate::widgets::list::List;
    use crate::widgets::scroll_area::LayoutScrollAnchorResolution;

    // SPEC-HAL-131A Stage 16: Set anchor resolution phase flag
    // INV-SCROLL-DIRTY-3: Realization code checks this flag and panics if true
    ANCHOR_RESOLUTION_ACTIVE.with(|f| f.set(true));

    // Phase 1: Collect work items (read-only)
    // Build vector of (scroll_node_id, content_node_id)
    let mut work = Vec::new();

    for &scroll_id in tree.scroll_anchor_nodes() {
        let scroll = tree
            .node(scroll_id)
            .and_then(|node| node.widget.as_any().downcast_ref::<ScrollArea>())
            .expect("post-layout anchors: indexed ScrollArea is missing or stale");
        let content_id = scroll.content();
        let content = tree
            .node(content_id)
            .expect("post-layout anchors: invalid ScrollArea content pair");
        assert_eq!(
            content.parent(),
            Some(scroll_id),
            "post-layout anchors: wired content is not a child of its ScrollArea"
        );
        work.push((scroll_id, content_id));
    }

    // List scroll-follow-active: Collect requested scroll positions from List nodes.
    // List uses direct scroll_y values instead of anchor-based resolution.
    let mut direct_scroll_requests: Vec<(NodeId, f32)> = Vec::new();
    for &(scroll_id, content_id) in &work {
        if let Some(content_node) = tree.node_mut(content_id) {
            if let Some(list) = content_node.widget.as_any_mut().downcast_mut::<List>() {
                if let Some(scroll_y) = list.take_requested_scroll_y() {
                    direct_scroll_requests.push((scroll_id, scroll_y));
                }
            }
        }
    }

    // Generic scroll-follow-active: Use Widget::take_requested_scroll_y() for any widget.
    // This handles ListViewBody and any future widgets with scroll requests.
    for &(scroll_id, content_id) in &work {
        if let Some(content_node) = tree.node_mut(content_id) {
            if let Some(scroll_y) = content_node.widget.take_requested_scroll_y() {
                direct_scroll_requests.push((scroll_id, scroll_y));
            }
        }
    }

    // Apply direct scroll requests to ScrollArea.
    for (scroll_id, scroll_y) in direct_scroll_requests {
        if let Some(scroll_node) = tree.node_mut(scroll_id) {
            if let Some(scroll) = scroll_node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                scroll.scroll_to_y(scroll_y);
            }
        }
    }

    // Phase 2: Execute resolutions using split-borrow helper
    // node_and_node_mut guarantees disjoint borrows via split_at_mut
    for (scroll_id, content_id) in work {
        // Read final layout geometry before taking the disjoint tree borrows.
        #[cfg(debug_assertions)]
        let content_rect = tree
            .get_rect(content_id)
            .expect("post-layout anchors: invalid ScrollArea content pair");
        let (content_node, scroll_node) = tree
            .node_and_node_mut(content_id, scroll_id)
            .expect("post-layout anchors: invalid ScrollArea content pair");
        let scroll = scroll_node
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .expect("post-layout anchors: indexed ScrollArea is missing or stale");
        let Some(resolver) = content_node.widget.as_anchor_resolver() else {
            continue;
        };

        // SPEC-HAL-131A Stage 15: INV-REALIZE-ORDER-1 applies to the resolver handoff.
        #[cfg(debug_assertions)]
        debug_assert!(
            content_rect.width > 0.0 || content_rect.height > 0.0,
            "INV-REALIZE-ORDER-1 VIOLATION: Content node {:?} with anchor resolver not realized",
            content_id
        );

        scroll.post_layout_resolve_anchors(resolver);
    }

    // SPEC-HAL-352: Resolve explicit layout-change transactions from the
    // completed layout. Geometry collection is read-only; ScrollArea remains
    // the sole mutation owner.
    let mut layout_anchor_work: Vec<(NodeId, Option<LayoutScrollAnchorResolution>)> = Vec::new();
    for &scroll_id in tree.scroll_anchor_nodes() {
        let Some(transaction) = tree
            .node(scroll_id)
            .and_then(|node| node.widget.as_any().downcast_ref::<ScrollArea>())
            .and_then(ScrollArea::pending_layout_anchor)
        else {
            continue;
        };

        let resolution = tree
            .find_by_id(transaction.anchor)
            .zip(tree.find_by_id(transaction.affected_region))
            .and_then(|(anchor_id, region_id)| {
                if !tree.is_descendant_or_same(anchor_id, region_id) {
                    return None;
                }
                let anchor_rect = tree.layout_anchor_content_rect(scroll_id, anchor_id)?;
                let region_rect = tree.layout_anchor_content_rect(scroll_id, region_id)?;
                Some(LayoutScrollAnchorResolution {
                    anchor_content_y: anchor_rect.y,
                    affected_region_bottom_y: region_rect.y + region_rect.height,
                })
            });
        layout_anchor_work.push((scroll_id, resolution));
    }

    for (scroll_id, resolution) in layout_anchor_work {
        let Some(scroll) = tree
            .node_mut(scroll_id)
            .and_then(|node| node.widget.as_any_mut().downcast_mut::<ScrollArea>())
        else {
            continue;
        };
        if let Some(resolution) = resolution {
            scroll.resolve_layout_anchor_transaction(resolution);
        } else {
            scroll.cancel_pending_layout_anchor();
        }
    }

    // SPEC-HAL-131A Stage 16: Clear anchor resolution phase flag
    ANCHOR_RESOLUTION_ACTIVE.with(|f| f.set(false));
}

/// SPEC-HAL-134: Synchronize ScrollArea scroll offsets to descendant List widgets.
///
/// This is the engine-owned pass that propagates resolved scroll offsets from
/// ScrollArea widgets to any List widgets in their subtrees. Lists use this
/// to update their visible_range for virtualization.
///
/// Frame ordering: layout_tree() → post_layout_resolve_scroll_anchors() →
///                 post_layout_sync_scroll_to_lists() → paint_tree()
///
/// INV-SCROLL-SYNC-1: Engine owns all ScrollArea ↔ List coordination.
/// INV-SCROLL-SYNC-2: Lists see scroll updates every frame where offset changes.
/// INV-SCROLL-SYNC-3: Called after anchor resolution (scroll offset is final).
/// INV-SCROLL-SYNC-4: Lists must be in ScrollCompute phase (guaranteed by layout completion).

/// PRE-LAYOUT scroll→virtualizing widget sync.
///
/// Called BEFORE layout_tree() to ensure List.visible_range and TabularSubstrate visible range
/// match current scroll_y. This prevents the "1-frame lag" where children are positioned
/// at old visible_range, then scroll transform (-scroll_y) moves them off-screen, causing black gaps.
///
/// INV-PRE-SYNC-1: Uses current scroll_y from ScrollArea state (from input events).
/// INV-PRE-SYNC-2: Lists must be in ScrollCompute phase (default state between frames).
/// INV-PRE-SYNC-3: TabularSubstrate follows same sync pattern as List.
pub fn pre_layout_sync_scroll_to_lists(tree: &mut WidgetTree) {
    // Phase 1: Collect (scroll_node_id, scroll_y, content_node_id) tuples
    let mut work: Vec<(NodeId, f32, NodeId)> = Vec::new();

    for &node_id in tree.scroll_anchor_nodes() {
        if let Some(node) = tree.node(node_id) {
            if let Some(scroll) = node.widget.as_any().downcast_ref::<ScrollArea>() {
                // INV-LIST-VISUAL-SYNC-1: Use visual_scroll_y (NOT target_scroll_y).
                // The scroll transform uses visual_offset, so visible_range must match.
                // Using target would cause mismatch during smooth scrolling.
                let scroll_y = scroll.visual_scroll_y();
                work.push((node_id, scroll_y, scroll.content()));
            }
        }
    }

    // Phase 2: For each ScrollArea, sync scroll to all descendant widgets via
    // Widget::sync_scroll trait method. This is the SINGLE scroll-sync dispatch point.
    // List uses update_visible_range_from_scroll; ListViewBody uses
    // set_scroll_y_from_scroll_area (non-token setter — layout resolves geometry).
    // CRITICAL: Must happen BEFORE layout so realization uses correct scroll_y.
    for (_scroll_id, scroll_y, content_id) in work {
        let mut to_visit = vec![content_id];
        while let Some(current_id) = to_visit.pop() {
            let invalidation = if let Some(node) = tree.node_mut(current_id) {
                // Skip ScrollAreas (they own their own sync)
                if node.widget.as_any().downcast_ref::<ScrollArea>().is_some() {
                    continue;
                }
                if node.widget.sync_scroll(scroll_y) {
                    Some(node.widget.scroll_sync_invalidation())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(invalidation) = invalidation {
                match invalidation {
                    ScrollSyncInvalidation::Layout => tree.mark_layout_dirty(current_id),
                    ScrollSyncInvalidation::Paint => tree.mark_paint_dirty(current_id),
                }
            }

            // Add children to visit queue
            for child_id in tree.children(current_id) {
                to_visit.push(child_id);
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PostLayoutScrollSyncResult {
    pub layout_deferred: bool,
}

pub fn post_layout_sync_scroll_to_lists(tree: &mut WidgetTree) -> PostLayoutScrollSyncResult {
    let mut result = PostLayoutScrollSyncResult::default();

    // Phase 1: Collect (scroll_node_id, scroll_y, content_node_id) tuples
    // Read-only pass to gather scroll offsets and content roots
    let mut work: Vec<(NodeId, f32, NodeId)> = Vec::new();

    for &node_id in tree.scroll_anchor_nodes() {
        if let Some(node) = tree.node(node_id) {
            if let Some(scroll) = node.widget.as_any().downcast_ref::<ScrollArea>() {
                // INV-LIST-VISUAL-SYNC-1: Use visual_scroll_y (NOT target_scroll_y).
                // The scroll transform uses visual_offset, so visible_range must match.
                // Using target would cause mismatch during smooth scrolling.
                let scroll_y = scroll.visual_scroll_y();
                work.push((node_id, scroll_y, scroll.content()));
            }
        }
    }

    // Phase 2: For each ScrollArea, sync scroll to all descendant widgets via
    // Widget::sync_scroll trait method. Single dispatch point — same as pre-layout.
    // INV-SCROLL-SYNC-5: Do NOT descend into nested ScrollAreas (they own their own sync).
    for (scroll_id, scroll_y, content_id) in work {
        let mut to_visit = vec![content_id];
        while let Some(current_id) = to_visit.pop() {
            let invalidation = if let Some(node) = tree.node_mut(current_id) {
                // Skip ScrollAreas (they own their own sync)
                if node.widget.as_any().downcast_ref::<ScrollArea>().is_some() {
                    continue;
                }
                if node.widget.sync_scroll(scroll_y) {
                    Some(node.widget.scroll_sync_invalidation())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(invalidation) = invalidation {
                match invalidation {
                    ScrollSyncInvalidation::Layout => {
                        tree.mark_layout_dirty(current_id);
                        result.layout_deferred = true;
                    }
                    ScrollSyncInvalidation::Paint => tree.mark_paint_dirty(current_id),
                }
            }

            // Add children to visit queue
            for child_id in tree.children(current_id) {
                to_visit.push(child_id);
            }
        }

        // Phase 5: Copy the content widget's separator centerlines into ScrollArea's
        // compatibility cache in content coordinates. Ancestors own resize admission
        // using their current geometry.
        let separator_bands: Option<Vec<f32>> = tree
            .node(content_id)
            .and_then(|node| node.widget.separator_hit_bands());

        if let Some(bands) = separator_bands {
            if let Some(node) = tree.node_mut(scroll_id) {
                if let Some(scroll) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                    scroll.set_separator_hit_bands(bands);
                }
            }
        }
    }

    result
}

/// Per-frame widget update for time-based state changes.
///
/// Called before layout_tree() to allow widgets to update scroll, animation,
/// or other time-dependent state based on real frame delta time.
///
/// INV-UPDATE-1: Runs once per frame, before layout.
/// INV-UPDATE-2: Widgets receive real dt (seconds since last frame).
/// INV-UPDATE-3: This is the only place for time-based scroll mutation.
type FlatViewport3DFrameTransfer<'a> = Option<(
    &'a mut crate::three_d::transfer::FlatViewport3DTransfers,
    Option<crate::core::frame_realization::AppFrameAttemptId>,
)>;

#[cfg(test)]
fn pre_layout_update_widgets(
    tree: &mut WidgetTree,
    root: NodeId,
    dt: f32,
    transition_layer: &mut TransitionLayer,
    theme: &Theme,
    appearance: Option<&FrameworkAppearanceSnapshot>,
    flat_viewport3d: FlatViewport3DFrameTransfer<'_>,
) {
    pre_layout_update_widgets_with_geometry(
        tree,
        root,
        dt,
        transition_layer,
        theme,
        appearance,
        flat_viewport3d,
        None,
        1.0,
    );
}

#[allow(clippy::too_many_arguments)]
fn pre_layout_update_widgets_with_geometry(
    tree: &mut WidgetTree,
    root: NodeId,
    dt: f32,
    transition_layer: &mut TransitionLayer,
    theme: &Theme,
    appearance: Option<&FrameworkAppearanceSnapshot>,
    flat_viewport3d: FlatViewport3DFrameTransfer<'_>,
    layout_transitions: Option<&ActiveLayoutTransitions>,
    scale_factor: f32,
) {
    tree.sync_linked_horizontal_scroll_projections();
    let (mut transfers, cutoff_frame) = match flat_viewport3d {
        Some((transfers, frame)) => (Some(transfers), frame),
        None => (None, None),
    };
    // HAL-021/101: settle retired concrete owners before any successor Update.
    // The existing single Timeline advance and terminal observation window remain.
    transition_layer
        .animation_context()
        .cancel_retired_widget_owners();
    let mut cx = UpdateContext::with_theme(Some(transition_layer.animation_context()), theme);
    cx.set_flat_viewport3d_transfers(transfers.as_deref_mut());
    update_widget_recursive(
        tree,
        root,
        dt,
        &mut cx,
        appearance,
        layout_transitions,
        scale_factor,
    );
    // INV-354-G1-CUTOFF: the entire timed traversal and all producer borrows end
    // before the sole owner transfer, still inside the caller's Update authority.
    drop(cx);
    if let Some(transfers) = transfers {
        if transfers.has_active_for_tree(tree) {
            transfers.cutoff(tree, cutoff_frame);
        }
    }
}

fn update_widget_recursive(
    tree: &mut WidgetTree,
    node_id: NodeId,
    dt: f32,
    cx: &mut UpdateContext,
    appearance: Option<&FrameworkAppearanceSnapshot>,
    layout_transitions: Option<&ActiveLayoutTransitions>,
    scale_factor: f32,
) {
    let children: Vec<NodeId> = tree.children(node_id);
    let owner = tree.node(node_id).map(|node| node.id);
    let mut invalidations = (false, false, false);
    let mut needs_animation = false;
    if let Some(owner) = owner {
        cx.begin_widget_update_for_owner(owner);
        cx.prepare_flat_viewport3d_source(tree, owner);
        cx.set_appearance_facts(appearance.map(|snapshot| snapshot.for_owner(owner)));
        cx.set_composite_delegation(
            appearance.and_then(|snapshot| tree.composite_delegation(owner, snapshot)),
        );
        let mut geometry = tree.own_presentation_geometry(node_id, scale_factor);
        if let Some(transitions) = layout_transitions.filter(|transitions| transitions.has_active())
        {
            let mut ancestor = Some(node_id);
            while let Some(id) = ancestor {
                let Some(node) = tree.node(id) else {
                    geometry = None;
                    break;
                };
                if transitions
                    .get(node.id)
                    .is_some_and(|state| cx.animation_is_active(state.handle))
                {
                    geometry = None;
                    break;
                }
                ancestor = node.parent;
            }
        }
        cx.set_own_presentation_geometry(geometry);
        if let Some((widget, mount)) = tree.widget_with_animation_mount_mut(node_id) {
            let mut own_cx = cx.for_widget_update(owner, mount);
            widget.update(dt, &mut own_cx);
            invalidations = own_cx.take_widget_invalidations();
            needs_animation = widget.needs_animation();
        }
    } else {
        cx.begin_widget_update();
    }
    tree.capture_button_sdf_request(node_id);
    let (paint_invalidated, subtree_paint_invalidated, layout_invalidated) = invalidations;
    if layout_invalidated {
        tree.mark_layout_dirty(node_id);
    } else if subtree_paint_invalidated {
        tree.mark_subtree_paint_dirty(node_id);
    } else if paint_invalidated || needs_animation {
        tree.mark_paint_dirty(node_id);
    }
    for child_id in children {
        update_widget_recursive(
            tree,
            child_id,
            dt,
            cx,
            appearance,
            layout_transitions,
            scale_factor,
        );
    }
}

// ============================================================================
// SPEC-HAL-190: Deferred Mutation Phase
// ============================================================================

/// SPEC-HAL-190 Section 15.1: Deferred Mutation Phase.
///
/// This phase runs after event routing and before layout. It processes
/// all pending intents that were emitted during event handling.
///
/// # Invariants
///
/// - INV-INTENT-PROCESS-1: `process_intents` is called exactly once per frame if non-empty.
/// - INV-INTENT-PROCESS-2: Buffer is cleared before the call; owner receives owned data.
/// - INV-INTENT-BEST-EFFORT: If owner widget is removed before phase runs, intents are silently dropped.
///
/// # Phase Sequence
///
/// 1. EVENT ROUTING (input events delivered to widgets)
/// 2. DEFERRED MUTATION ← this phase (intents processed by authority owners)
/// 3. LAYOUT (geometry computed)
/// 4. POST-LAYOUT SYNC (scroll anchors resolved)
/// 5. PAINT (scene rendered)
fn run_deferred_mutation_phase(tree: &mut WidgetTree) {
    // Drain all intent buffers. Each (owner_id, intents) pair represents
    // a set of intents targeting a specific authority owner.
    let pending_intents = tree.drain_intent_buffers();

    // Process each owner's intents
    for (owner_id, intents) in pending_intents {
        // Look up the node by TreeWidgetId
        if let Some(node_id) = tree.find_by_id(owner_id) {
            // Get mutable access to the widget
            if let Some(node) = tree.node_mut(node_id) {
                // INV-AUTH-OWNER-MECHANICAL: Only call process_intents on AuthorityOwner.
                // Widgets that don't implement AuthorityOwner return None from as_authority_owner().
                // This mechanically enforces that only authority owners receive intents.
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
                // If not an AuthorityOwner, intents are silently dropped.
                // This is a programming error - emit_intent should only target authority owners.
            }
            // If node_mut returns None, widget was removed between find_by_id and here.
            // INV-INTENT-BEST-EFFORT: silently drop intents.
        }
        // If find_by_id returns None, owner widget was removed.
        // INV-INTENT-BEST-EFFORT: silently drop intents.
    }
}

pub(crate) fn next_qualified_widget_input(
    tree: &WidgetTree,
    intake: &mut std::vec::IntoIter<crate::core::event::SessionQualifiedInput>,
    deferred: &mut Vec<crate::core::event::SessionQualifiedInput>,
) -> Option<crate::core::event::SessionQualifiedInput> {
    let next = intake.next()?;
    if tree.has_pending_widget_availability() {
        deferred.push(next);
        deferred.extend(intake.by_ref());
        None
    } else {
        Some(next)
    }
}

/// SPEC-HAL-131C: Populates input projection snapshots for virtualizing containers.
///
/// INV-PROJECTION-SNAPSHOT-1: Called after layout_tree(), projections are read-only after.
/// INV-INPUT-PROJECTION-1: Input queries projection snapshot, never realized tree.
/// INV-INPUT-PROJECTION-2: Projection population does not trigger realization.
///
/// This iterates through the widget tree looking for widgets that implement
/// InputProjectionProvider and builds their projections for hit-testing.
pub fn post_layout_populate_input_projections(tree: &mut WidgetTree) {
    use crate::core::input_projection::{InputProjection, InputProjectionEntry};

    // Clear existing projections
    tree.clear_input_projections();

    // Collect all nodes first (avoid borrow conflicts)
    let mut all_nodes = Vec::new();
    tree.for_each_node_in_stable_order(|node_id| {
        all_nodes.push(node_id);
    });

    // Find widgets that provide input projections
    for node_id in all_nodes {
        let Some(node) = tree.node(node_id) else {
            continue;
        };

        let Some(provider) = node.widget.as_input_projection_provider() else {
            continue;
        };

        // Build the projection from the provider
        let item_count = provider.logical_item_count();
        if item_count == 0 {
            continue;
        }

        let mut projection = InputProjection::new();
        // SPEC-HAL-141: Access geometry through WidgetTree getter (private field).
        let abs_rect = tree
            .get_absolute_paint_rect(node_id)
            .unwrap_or(RectF::zero());
        projection.set_container(node.id, abs_rect);
        projection.set_content_extent(provider.content_width(), {
            // Get total content height from last item bounds
            if let Some(last_bounds) = provider.logical_item_bounds(item_count - 1) {
                last_bounds.y + last_bounds.height
            } else {
                0.0
            }
        });

        // Get scroll offset from parent ScrollArea if applicable
        if let Some(parent_id) = node.parent {
            if let Some(parent_node) = tree.node(parent_id) {
                if let Some(scroll) = parent_node.widget.as_any().downcast_ref::<ScrollArea>() {
                    let offset = scroll.logical_offset();
                    projection.set_scroll_offset(offset.x, offset.y);
                }
            }
        }

        // Add entries for all logical items
        let realized_range = provider.realized_range();

        // SPEC-HAL-134 Phase 3: Downstream consumers MUST treat realized_range as authoritative.
        // INV-VIRT-DECOUPLE-1: realized_range is derived ONLY from visible_range + prefetch.
        // This function MUST NOT recompute, clamp, supplement, or reinterpret the range.
        debug_assert!(
            realized_range.start <= realized_range.end,
            "SPEC-HAL-134/131B: realized_range must be well-formed (start <= end). \
             Callers must not modify or reinterpret visible_range."
        );
        debug_assert!(
            realized_range.end <= item_count,
            "SPEC-HAL-134/131B: realized_range.end ({}) must not exceed item_count ({}). \
             Callers must not extend beyond authoritative range.",
            realized_range.end,
            item_count
        );

        for i in 0..item_count {
            if let (Some(bounds), Some(id)) =
                (provider.logical_item_bounds(i), provider.logical_item_id(i))
            {
                // Check if this item is realized
                let realized_widget = if realized_range.contains(&i) {
                    provider.realized_widget_for_item(i)
                } else {
                    None
                };

                projection.add_entry(InputProjectionEntry {
                    logical_id: id,
                    content_bounds: bounds,
                    realized_widget,
                    logical_index: i,
                });
            }
        }

        tree.set_input_projection(node_id, projection);
    }
}

/// Reveal a focused widget by scrolling the nearest ancestor ScrollArea.
///
/// Called after layout to ensure focused widgets are visible in their scroll container.
/// This is the engine-level "focus follow scroll" mechanism for non-List widgets.
///
/// Requirements:
/// - `focus_widget_id`: The widget that just gained focus
/// - Finds the nearest ancestor ScrollArea (if any)
/// - Computes widget rect in ScrollArea's content coordinate space
/// - Calls resolve_reveal_rect() with ScrollAlign::Nearest
///
/// Input arbitration:
/// - If the ScrollArea is in drag mode (active scroll gesture), skip reveal
/// - If the ScrollArea has a pending anchor this frame, skip reveal
pub fn post_layout_reveal_focused_widget(tree: &mut WidgetTree, focus_widget_id: TreeWidgetId) {
    use crate::core::event::ScrollAlign;
    use crate::core::scroll_container::ScrollInputMode;

    let Some(focus_node_id) = tree.find_node_by_widget_id(focus_widget_id) else {
        return;
    };

    // Get the focused widget's absolute rect (computed during layout)
    // SPEC-HAL-141: Access geometry through WidgetTree getter (private field).
    let Some(focus_rect) = tree.get_absolute_paint_rect(focus_node_id) else {
        return;
    };

    // Walk up the tree to find the nearest ancestor ScrollArea
    let mut current_id = tree.parent(focus_node_id);
    let mut scroll_area_id: Option<NodeId> = None;

    while let Some(node_id) = current_id {
        if let Some(node) = tree.node(node_id) {
            if node.widget.as_any().downcast_ref::<ScrollArea>().is_some() {
                scroll_area_id = Some(node_id);
                break;
            }
            current_id = node.parent;
        } else {
            break;
        }
    }

    let Some(scroll_node_id) = scroll_area_id else {
        // No ancestor ScrollArea found - nothing to do
        return;
    };

    // Get ScrollArea info: absolute rect, whether it's in drag mode, has pending anchor
    // SPEC-HAL-141: Access geometry through WidgetTree getter (private field).
    let Some(scroll_abs_rect) = tree.get_absolute_paint_rect(scroll_node_id) else {
        return;
    };
    let (in_drag_mode, has_pending_anchor) = {
        let Some(scroll_node) = tree.node(scroll_node_id) else {
            return;
        };
        let Some(scroll) = scroll_node.widget.as_any().downcast_ref::<ScrollArea>() else {
            return;
        };

        let in_drag = scroll_node
            .widget
            .scroll_container()
            .map(|c| c.input_mode() == ScrollInputMode::Drag)
            .unwrap_or(false);
        let has_anchor = scroll.is_anchor_dirty();

        (in_drag, has_anchor)
    };

    // Input arbitration: Skip if active scroll gesture in progress
    if in_drag_mode {
        return;
    }

    // Skip if ScrollArea has pending anchor this frame
    if has_pending_anchor {
        return;
    }

    // Compute the focused widget rect in ScrollArea's content coordinate space.
    //
    // absolute_paint_rect is computed during LAYOUT (not paint), so scroll transforms
    // have NOT been applied. It represents the widget's position in content space,
    // offset by the ScrollArea's screen position.
    //
    // Content-relative position = focus_rect - scroll_abs_rect
    // (No need to add scroll offset - that's only relevant during paint)
    let content_rect = RectF::new(
        focus_rect.x - scroll_abs_rect.x,
        focus_rect.y - scroll_abs_rect.y,
        focus_rect.width,
        focus_rect.height,
    );

    // Call resolve_reveal_rect on the ScrollArea
    if let Some(scroll_node) = tree.node_mut(scroll_node_id) {
        if let Some(scroll) = scroll_node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
            scroll.resolve_reveal_rect(content_rect, ScrollAlign::Nearest);
        }
    }
}

/// SPEC-HAL-044: Builds a scene graph without rendering.
///
/// This is a pure CPU operation that:
/// 1. Performs layout pass (computes widget sizes and positions)
/// 2. Post-layout input projection population (SPEC-HAL-131C)
/// 3. Post-layout anchor resolution (SPEC-HAL-131A Stage 14)
/// 4. Post-layout focus reveal (scrolls focused widget into view)
/// 5. Performs paint pass (builds scene graph from widget tree)
///
/// Returns the completed SceneGraph ready for rendering on another thread.
///
/// Note: Input processing must be done BEFORE calling this function.
///
/// SPEC-HAL-101 Stage-4C: Added frame_time_ms for caret blinking.
/// SPEC-HAL-021: Added timeline for animation value consumption during paint.
/// SPEC-HAL-078 Stage 3: Added active_layout_transitions for paint-time interpolation.
/// SPEC-HAL-145: Pre-computes interpolated rects under TransitionAuthority before paint.
/// SPEC-HAL-078 FIX: Candidate collection/binding now happens AFTER layout (inside this function).
/// SPEC-HAL-226A: Tooltip render state passed from the frame loop into paint_tree.
#[derive(Clone, PartialEq)]
pub struct TooltipRenderState {
    pub anchor: crate::core::math::Point,
    pub content: crate::model::tooltip::TooltipPayload,
}

#[derive(Clone, Copy)]
enum TooltipSceneInput<'a> {
    Explicit(Option<&'a TooltipRenderState>),
    Managed {
        visible_target: Option<&'a crate::model::tooltip::TooltipTrigger>,
        pointer: Point,
        viewport: RectF,
    },
}

impl TooltipSceneInput<'_> {
    fn resolve(
        self,
        triggers: &[crate::model::tooltip::TooltipTrigger],
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
    ) -> Option<TooltipRenderState> {
        let (visible_target, pointer, viewport) = match self {
            Self::Explicit(explicit) => return explicit.cloned(),
            Self::Managed {
                visible_target,
                pointer,
                viewport,
            } => (visible_target, pointer, viewport),
        };
        let target = visible_target?;
        let current = triggers
            .iter()
            .find(|trigger| trigger.trigger_rect.contains(pointer))?;
        // Preserve the manager's widget+region dwell identity. Paint only derives
        // current output; a changed region settles at the next legal Update.
        if current.widget_id != target.widget_id || current.trigger_rect != target.trigger_rect {
            return None;
        }
        let crate::model::tooltip::TooltipPayload::Text(text) = &current.content;
        let run = text_engine.shape(text, theme.typography.font_size);
        let size = Size::new(
            run.width + crate::widgets::tooltip_surface::TOOLTIP_PADDING_X * 2.0,
            run.height + crate::widgets::tooltip_surface::TOOLTIP_PADDING_Y * 2.0,
        );
        Some(TooltipRenderState {
            anchor: crate::model::tooltip::compute_tooltip_anchor(
                current.trigger_rect,
                size,
                viewport,
                theme.tooltip.gap,
                current.placement,
            ),
            content: current.content.clone(),
        })
    }
}

fn advance_tooltip_manager(
    manager: &mut crate::model::tooltip::TooltipManager,
    pointer: Point,
    dt: f32,
    frame_time_ms: u64,
    suppressed: bool,
) -> Option<crate::model::tooltip::TooltipTrigger> {
    if suppressed {
        manager.suppress_interaction();
    } else {
        manager.tick(dt, frame_time_ms);
        manager.update_from_pointer(pointer.x, pointer.y, frame_time_ms);
    }
    manager
        .is_visible()
        .then(|| manager.hover_target_snapshot())
        .flatten()
}

fn publish_tooltip_triggers(
    manager: &mut crate::model::tooltip::TooltipManager,
    triggers: Vec<crate::model::tooltip::TooltipTrigger>,
    pointer: Point,
) -> bool {
    let previous = manager.hover_target_snapshot();
    // A completed empty snapshot retires the last declaration. This publication
    // is consumed by the manager at the next pre-layout checkpoint.
    *manager.triggers_mut() = triggers;
    match (
        previous.as_ref(),
        manager.find_trigger_at(pointer.x, pointer.y),
    ) {
        (None, None) => false,
        (Some(previous), Some(current)) => {
            previous.widget_id != current.widget_id
                || previous.trigger_rect != current.trigger_rect
                || previous.content != current.content
                || previous.placement != current.placement
        }
        _ => true,
    }
}

fn should_suppress_tooltips(
    widget_tree: &WidgetTree,
    keyboard_mode: KeyboardAuthorityMode,
    any_pointer_captured: bool,
    active_drag: bool,
) -> bool {
    keyboard_mode == KeyboardAuthorityMode::Capture
        || any_pointer_captured
        || active_drag
        || widget_tree.has_blocking_overlay_hit_region()
}

/// Legacy scene entry without domain-qualified appearance facts. Use
/// `build_scene_with_appearance` for consumers of the framework publication.
pub fn build_scene(
    widget_tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    frame_dt: f32,
    transition_layer: &mut TransitionLayer,
    active_layout_transitions: &mut ActiveLayoutTransitions,
    pending_focus_reveal: Option<TreeWidgetId>,
    scale_factor: f32,
    signals: Option<&mut SignalStore>,
    active_drag: Option<&crate::core::drag_drop::DragSession>,
    // SPEC-HAL-145 / SPEC-HAL-078A: When true, layout-transition candidate
    // binding is suppressed for this frame (direct manipulation in progress).
    // Caller passes `pointer_capture_manager.is_any_captured()` from the
    // active interaction domain.
    any_pointer_captured: bool,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: Option<&TooltipRenderState>,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: crate::core::event::KeyboardAuthorityMode,
) -> SceneGraph {
    let mut retained_cache = RetainedSceneCache::default();
    let mut recycled_scene = None;
    let mut custom_render_presentation_bindings = CustomRenderPresentationBindings::default();
    build_scene_reusing(
        widget_tree,
        root,
        constraints,
        theme,
        text_engine,
        icon_registry,
        frame_time_ms,
        frame_dt,
        transition_layer,
        active_layout_transitions,
        pending_focus_reveal,
        scale_factor,
        None,
        &mut custom_render_presentation_bindings,
        signals,
        active_drag,
        any_pointer_captured,
        tooltip_triggers,
        TooltipSceneInput::Explicit(tooltip_render),
        focused_widget,
        keyboard_mode,
        None,
        true,
        &mut recycled_scene,
        &mut retained_cache,
        None,
        false,
    )
    .scene
    .expect("fresh build_scene must produce a scene")
}

struct SceneBuildOutcome {
    scene: Option<SceneGraph>,
    visual_changed: bool,
    needs_next_frame: bool,
    material_pending: bool,
    plot_evidence: Vec<crate::plot::PlotPreparationEvidence>,
}

#[must_use]
enum SceneBuildContinuation {
    Deferred(SceneBuildOutcome),
    Paint(ScenePaintContinuation),
}

/// Same-attempt owned paint inputs; no Widget, tree or text-engine borrow escapes.
struct ScenePaintContinuation {
    tree_lifetime: crate::core::widget_tree::WidgetTreeLifetimeToken,
    root: NodeId,
    root_owner: Option<TreeWidgetId>,
    frame_time_ms: u64,
    scale_factor: f32,
    custom_render_target: Option<CustomRenderTargetSnapshot>,
    appearance: Option<FrameworkAppearanceSnapshot>,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: KeyboardAuthorityMode,
    debug_outlines: bool,
    active_drag: Option<RetainedDragState>,
    interpolated_rects: InterpolatedVisualRects,
    front_publication_changed: bool,
    plot_evidence: Vec<crate::plot::PlotPreparationEvidence>,
    force_scene: bool,
}

// Resource-only engine hook. It cannot mutate the tree or rerun Widget Update.
type CanonicalPrePaintHook = fn(&mut WgpuRenderer, &WidgetTree, &mut RetainedSceneCache);

fn prepare_canonical_sdf_materials(
    renderer: &mut WgpuRenderer,
    tree: &WidgetTree,
    cache: &mut RetainedSceneCache,
) {
    if cache.qualified_sdf_requests.is_none() && cache.sdf_material_staging.is_none() {
        return;
    }
    let current = cache.current_sdf_frame_inputs(tree);
    if current.is_empty() && cache.sdf_material_staging.is_none() {
        return;
    }
    let eligible = cache.active_sdf_size_replacement_keys(&current);
    let selected = cache
        .selected_sdf_materials
        .iter()
        .flat_map(|selected| selected.values())
        .flatten()
        .filter(|publication| {
            current.iter().any(|input| {
                cache
                    .sdf_material_staging
                    .as_ref()
                    .is_some_and(|staging| staging.can_continue_selection(input, publication.key()))
            })
        })
        .map(|publication| publication.key().clone())
        .collect::<Vec<_>>();
    let staging = cache
        .sdf_material_staging
        .get_or_insert_with(Default::default);
    if eligible.is_empty() {
        staging.prepare_before_paint(renderer, &current, &selected);
    } else {
        staging.prepare_with_active_size_replacements(renderer, &current, &selected, &eligible);
    }
    if current.is_empty() && staging.is_empty() {
        cache.sdf_material_staging = None;
    }
}

/// Build a scene with the same authoritative appearance publication as App.
/// Consumers such as Button require this source; the legacy snapshot-free
/// helper cannot establish window/domain/capture truth.
/// Begin each genuinely separate frame once through `EngineTicker` and pass that
/// frame's returned timing here. This helper consumes the begun frame; it does
/// not begin a frame or reset layout accounting.
#[allow(clippy::too_many_arguments)]
pub fn build_scene_with_appearance(
    widget_tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    frame_dt: f32,
    transition_layer: &mut TransitionLayer,
    active_layout_transitions: &mut ActiveLayoutTransitions,
    pending_focus_reveal: Option<TreeWidgetId>,
    scale_factor: f32,
    signals: Option<&mut SignalStore>,
    active_drag: Option<&crate::core::drag_drop::DragSession>,
    // SPEC-HAL-145 / SPEC-HAL-078A: When true, layout-transition candidate
    // binding is suppressed for this frame (direct manipulation in progress).
    // Caller passes `pointer_capture_manager.is_any_captured()` from the
    // active interaction domain.
    any_pointer_captured: bool,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: Option<&TooltipRenderState>,
    domains: &InteractionDomains,
    window: WindowId,
) -> SceneGraph {
    let mut retained_cache = RetainedSceneCache::default();
    let mut recycled_scene = None;
    let mut custom_render_presentation_bindings = CustomRenderPresentationBindings::default();
    build_scene_reusing(
        widget_tree,
        root,
        constraints,
        theme,
        text_engine,
        icon_registry,
        frame_time_ms,
        frame_dt,
        transition_layer,
        active_layout_transitions,
        pending_focus_reveal,
        scale_factor,
        None,
        &mut custom_render_presentation_bindings,
        signals,
        active_drag,
        any_pointer_captured,
        tooltip_triggers,
        TooltipSceneInput::Explicit(tooltip_render),
        None,
        KeyboardAuthorityMode::Navigation,
        Some((domains, window)),
        true,
        &mut recycled_scene,
        &mut retained_cache,
        None,
        false,
    )
    .scene
    .expect("fresh build_scene_with_appearance must produce a scene")
}

mod transient_presentation;
use transient_presentation::PresentationRuntime;

struct GestureSceneAccess<'a> {
    domains: &'a mut InteractionDomains,
    window: WindowId,
    router: &'a mut EventRouter,
    fusion: &'a mut InputFusion,
}

struct SessionInputDelivery<'a>(&'a crate::core::event::SessionQualifiedInput);

enum DeferredAppInput {
    Raw(RawInputEvent),
    #[cfg(any(target_arch = "wasm32", test))]
    Browser {
        source: BrowserTextSource,
        report: BrowserTextReport,
    },
    #[cfg(any(target_arch = "wasm32", test))]
    ProxyPaste,
    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    HostChoice(crate::core::event_router::host_choice::ChoiceReply),
}

/// The raw prefix has already reached App ingress. Stop at a browser report,
/// which needs to see the results of every preceding qualified event.
fn fuse_deferred_raw_prefix(
    deferred: &mut VecDeque<DeferredAppInput>,
    fused: &mut Vec<SessionQualifiedInput>,
    fusion: &mut InputFusion,
    tree: &WidgetTree,
    domains: &mut InteractionDomains,
) {
    let state = domains.primary_state_mut();
    let gate = EventRouter::relative_ingress_gate(state);
    while matches!(deferred.front(), Some(DeferredAppInput::Raw(_))) {
        let Some(DeferredAppInput::Raw(raw)) = deferred.pop_front() else {
            unreachable!("the raw prefix was just checked");
        };
        fusion.process_raw_with_gesture_sessions(
            raw,
            crate::core::event::InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            tree,
            gate.as_ref(),
            fused,
        );
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Default)]
struct AppBrowserEditor {
    session: Option<BrowserTextSession>,
    pending_paste: Option<crate::core::event_router::browser_paste::BrowserPaste>,
    visible_size: Option<Size>,
    geometry_pending: bool,
    restore_canvas_focus: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
impl AppBrowserEditor {
    fn retire_session(
        &mut self,
        #[cfg(target_arch = "wasm32")] browser_text: Option<&crate::platform::web::BrowserTextBridge>,
    ) {
        #[cfg(target_arch = "wasm32")]
        if let Some(text) = browser_text {
            let restore_focus = self.restore_canvas_focus && text.has_proxy_focus();
            text.retire();
            if restore_focus {
                text.restore_canvas_focus();
            }
        }
        self.session = None;
        self.geometry_pending |= self.visible_size.take().is_some();
        self.restore_canvas_focus = false;
    }

    fn record_proxy_paste(&self, deferred: &mut VecDeque<DeferredAppInput>) {
        if self.session.is_some() {
            deferred.push_back(DeferredAppInput::ProxyPaste);
        }
    }

    fn proxy_paste_pending(deferred: &VecDeque<DeferredAppInput>) -> bool {
        deferred
            .iter()
            .any(|input| matches!(input, DeferredAppInput::ProxyPaste))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppInputDrain {
    Drained,
    AvailabilityPending,
    #[cfg(any(target_arch = "wasm32", test))]
    GeometryPending,
}

struct AppInputDelivery<'a> {
    app_root: &'a mut AppRoot,
    widget_tree: &'a mut WidgetTree,
    event_router: &'a mut EventRouter,
    interaction_domains: &'a mut InteractionDomains,
    input_fusion: &'a mut InputFusion,
    text_engine: &'a mut dyn TextEngine,
    transition_layer: &'a mut TransitionLayer,
    active_layout_transitions: &'a ActiveLayoutTransitions,
    tooltip_manager: &'a mut crate::model::tooltip::TooltipManager,
    last_pointer_pos: &'a mut Point,
    window: Option<&'a winit::window::Window>,
    viewport: RectF,
    #[cfg(any(target_arch = "wasm32", test))]
    browser: &'a mut AppBrowserEditor,
    #[cfg(target_arch = "wasm32")]
    browser_text: Option<&'a crate::platform::web::BrowserTextBridge>,
    #[cfg(target_os = "ios")]
    ios_choice: &'a mut crate::platform::native_choice_ios::NativeChoiceHost,
    #[cfg(all(target_os = "macos", debug_assertions))]
    engine_conformance:
        Option<&'a crate::platform::relative_pointer_macos_conformance::EngineRecorder>,
}

impl AppInputDelivery<'_> {
    fn drain_frame_input(
        &mut self,
        fused: &mut Vec<SessionQualifiedInput>,
        _deferred: &mut VecDeque<DeferredAppInput>,
        _geometry_pending: bool,
        default_cursor_event: &mut bool,
    ) -> AppInputDrain {
        let status = self.drain_ready(fused, default_cursor_event);
        #[cfg(any(target_arch = "wasm32", test))]
        if status == AppInputDrain::Drained {
            return self.drain_browser_input(
                fused,
                _deferred,
                _geometry_pending,
                default_cursor_event,
            );
        }
        #[cfg(all(target_os = "ios", not(test)))]
        if status == AppInputDrain::Drained && !_geometry_pending {
            while !self.widget_tree.has_pending_widget_availability() {
                fuse_deferred_raw_prefix(
                    _deferred, fused, self.input_fusion, self.widget_tree, self.interaction_domains,
                );
                self.input_fusion.flush_with_gesture_sessions(fused);
                let status = self.drain_ready(fused, default_cursor_event);
                if status != AppInputDrain::Drained {
                    return status;
                }
                match _deferred.pop_front() {
                    Some(DeferredAppInput::HostChoice(reply)) => self.apply_host_choice(reply),
                    Some(DeferredAppInput::Raw(_)) => unreachable!("the raw prefix was just drained"),
                    None => break,
                }
            }
        }
        status
    }

    /// Settle the ordinary qualified input lane. Taking the batch gives each
    /// event one terminal delivery and preserves its unread suffix at a cutoff.
    fn drain_ready(
        &mut self,
        fused_events: &mut Vec<SessionQualifiedInput>,
        default_cursor_event: &mut bool,
    ) -> AppInputDrain {
        let Self {
            app_root,
            widget_tree,
            event_router,
            interaction_domains,
            input_fusion,
            text_engine,
            transition_layer,
            active_layout_transitions,
            tooltip_manager,
            last_pointer_pos,
            window,
            viewport,
            #[cfg(any(target_arch = "wasm32", test))]
            browser,
            #[cfg(target_arch = "wasm32")]
            browser_text,
            #[cfg(target_os = "ios")]
            ios_choice,
            #[cfg(all(target_os = "macos", debug_assertions))]
            engine_conformance,
        } = self;
        let text_engine = &mut **text_engine;
        let mut availability_intake = std::mem::take(fused_events).into_iter();
        while let Some(qualified) =
            next_qualified_widget_input(widget_tree, &mut availability_intake, fused_events)
        {
            #[cfg(any(target_arch = "wasm32", test))]
            if browser.pending_paste.as_ref().is_some_and(|paste| {
                app_root
                    .browser_paste_root()
                    .is_none_or(|root| !paste.is_current(widget_tree, interaction_domains, root))
            }) {
                browser.pending_paste = None;
                tracing::warn!("Browser Paste refused: stale input owner/source");
            }
            // Settle the preceding admitted event even if it took an early exit.
            if let Some(window) = *window {
                apply_window_cursor(
                    window,
                    event_router,
                    *default_cursor_event,
                    EventRouter::relative_ingress_gate(interaction_domains.primary_state())
                        .is_some(),
                );
            }
            *default_cursor_event = false;
            let _delivery = SessionInputDelivery(&qualified);
            let event = qualified.event().clone();
            #[cfg(target_arch = "wasm32")]
            event_router.set_host_choice_offer(
                if let (Some(text), Some(window), Some(motion)) = (*browser_text, *window, qualified.touch_motion())
                    && matches!(&event, FusedInputEvent::TouchUp { .. })
                {
                    text.choice().map(|choice| {
                        choice.offer(window.id(), motion.native_contact).unwrap_or(
                            crate::core::event_router::host_choice::HostOffer::MissingTrustedSelect,
                        )
                    })
                } else {
                    None
                },
            );
            #[cfg(target_os = "ios")]
            event_router.set_host_choice_offer(
                window.as_ref().filter(|_| matches!(&event, FusedInputEvent::TouchUp { .. }))
                    .filter(|window| ios_choice.available(window))
                    .map(|_| crate::core::event_router::host_choice::HostOffer::Native(event.window_id())),
            );
            // SPEC-HAL-226A: Track pointer position for tooltip hover detection
            match &event {
                FusedInputEvent::PointerMove { pos, .. }
                | FusedInputEvent::PointerDown { pos, .. }
                | FusedInputEvent::PointerUp { pos, .. } => {
                    **last_pointer_pos = *pos;
                }
                _ => {}
            }
            // SPEC-HAL-226A: Dismiss tooltip on pointer-down or Escape
            if matches!(&event, FusedInputEvent::PointerDown { .. }) {
                tooltip_manager.pointer_down();
            }
            if let FusedInputEvent::KeyDown { key_code, .. } = &event {
                if *key_code == crate::core::event::KeyCode::Escape {
                    tooltip_manager.escape_pressed();
                }
            }

            if let Some(snapshot) = app_root.capture_blueprint_shortcut(
                &event,
                widget_tree,
                interaction_domains,
                event_router,
                text_engine,
            ) {
                write_blueprint_shortcut_snapshot(&snapshot);
                qualified.finish_delivery(true);
                continue;
            }

            // SPEC-HAL-151: Active-window gate - drop events from non-active windows
            // except FocusChanged, Resize, and Tick which are always routed
            if !interaction_domains.should_route_event(&event) {
                tracing::debug!(
                    "SPEC-HAL-151: Dropped event from non-active window {:?}",
                    event.window_id().0
                );
                continue;
            }

            let event_window_id = event.window_id();
            let relative_facts = interaction_domains.domain_for_window(event_window_id).map(
                |interaction_domain_id| RelativePointerWindowFacts {
                    window_id: event_window_id,
                    interaction_domain_id,
                    focused_window_id: interaction_domains.active_window(),
                    client_bounds: *viewport,
                },
            );
            if let FusedInputEvent::FocusChanged {
                window_id,
                focused: false,
                ..
            } = &event
            {
                if let Some(state) = interaction_domains.resolve_window_mut(*window_id) {
                    #[cfg(all(target_os = "macos", debug_assertions))]
                    if let Some(recorder) = engine_conformance {
                        recorder.record(
                            "engine_trigger",
                            serde_json::json!({"trigger": "focus_loss"}),
                        );
                    }
                    event_router.cancel_active_relative_with_tree(
                        widget_tree,
                        state,
                        RelativePointerCancellationReason::FocusLost,
                        None,
                    );
                    input_fusion.discard_pending_relative_pointer();
                    #[cfg(all(target_os = "macos", debug_assertions))]
                    if let Some(recorder) = engine_conformance {
                        recorder.record_state("focus_loss", state);
                    }
                }
            }

            // SPEC-HAL-151: Handle FocusChanged specially for domain teardown
            let domain_state = if let FusedInputEvent::FocusChanged {
                window_id, focused, ..
            } = &event
            {
                match handle_window_focus_changed(
                    interaction_domains,
                    event_router,
                    widget_tree,
                    *window_id,
                    *focused,
                    text_engine,
                ) {
                    Some(state) => state,
                    None => {
                        tracing::debug!(
                            "SPEC-HAL-151: No domain for window {:?}, dropping FocusChanged",
                            window_id.0
                        );
                        continue;
                    }
                }
            } else {
                // SPEC-HAL-151: Resolve domain from window_id
                let window_id = event.window_id();
                match interaction_domains.resolve_window_mut(window_id) {
                    Some(state) => state,
                    None => {
                        tracing::debug!(
                            "SPEC-HAL-151: No domain for window {:?}, dropping event",
                            window_id.0
                        );
                        continue;
                    }
                }
            };

            if !prepare_session_input_for_app(widget_tree, event_router, domain_state, &qualified) {
                continue;
            }
            *default_cursor_event = cursor_default_event(
                &event,
                domain_state
                    .pointer_capture_manager
                    .is_captured(crate::core::event::PointerId::MOUSE),
            );
            if matches!(
                &event,
                FusedInputEvent::KeyDown {
                    key_code: crate::core::event::KeyCode::Escape,
                    ..
                }
            ) {
                if event_router.cancel_active_relative_with_tree(
                    widget_tree,
                    domain_state,
                    RelativePointerCancellationReason::ExplicitSemanticCancel,
                    relative_facts,
                ) {
                    input_fusion.discard_pending_relative_pointer();
                }
            }
            cancel_relative_overlay_takeover(app_root, widget_tree, event_router, domain_state);
            if EventRouter::claims_relative_event(&event, domain_state) {
                let input_root = app_root.active_input_root_node();
                event_router.route_session_input_event_with_relative_facts(
                    widget_tree,
                    qualified.clone(),
                    Some(input_root),
                    text_engine,
                    Some(transition_layer.animation_context()),
                    domain_state,
                    &mut app_root.context.registry.binding_registry,
                    relative_facts.expect("active relative lease has a registered domain"),
                );
                let gate = EventRouter::relative_ingress_gate(domain_state);
                input_fusion.reconcile_relative_ingress(gate.as_ref());
                #[cfg(all(target_os = "macos", debug_assertions))]
                if let Some(recorder) = engine_conformance {
                    recorder.record_state("relative_route", domain_state);
                }
                continue;
            }

            let mut reserved_action = |reservation| {
                if reservation == crate::core::event::KeyboardKeyReservation::Fullscreen
                    && let Some(window) = *window
                {
                    if window.fullscreen().is_some() {
                        window.set_fullscreen(None);
                    } else {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    }
                    window.request_redraw();
                }
            };
            let app_dispatch = dispatch_session_input_to_app_root_with_captured_routing(
                app_root,
                widget_tree,
                event_router,
                domain_state,
                &qualified,
                text_engine,
                Some(&mut reserved_action),
                Some(transition_layer.animation_context()),
                relative_facts,
            );
            cancel_relative_overlay_takeover(app_root, widget_tree, event_router, domain_state);
            #[cfg(any(target_arch = "wasm32", test))]
            if app_dispatch.blocks_browser_paste(&qualified, domain_state) {
                browser.pending_paste = None;
            }
            if app_dispatch.handled {
                if app_dispatch.captured_key
                    != crate::core::event_router::CapturedAppKeyOutcome::NotEligible
                {
                    let pending_focus = event_router.geometry_pending_focus_reveal();
                    geometry_effect_checkpoint(
                        widget_tree,
                        event_router,
                        domain_state,
                        Some((active_layout_transitions, transition_layer)),
                        pending_focus,
                    );
                    let gate = EventRouter::relative_ingress_gate(domain_state);
                    input_fusion.reconcile_relative_ingress(gate.as_ref());
                }
                continue;
            }
            let input_root = app_root.active_input_root_node();
            if let Some(modal_root) = app_root.active_modal_input_root_node() {
                event_router.enforce_modal_input_boundary(widget_tree, modal_root, domain_state);
            }
            // SPEC-HAL-021 Stage 5 / SPEC-HAL-038 Stage 6: Construct
            // animation capability BEFORE dispatch_engine_action so
            // keyboard activation receives it.
            let animation = Some(transition_layer.animation_context());
            // SPEC-HAL-176: Engine action dispatch (shared with run_ui_loop)
            if dispatch_engine_action(
                &event,
                &mut app_root.context.registry.binding_registry,
                event_router,
                widget_tree,
                input_root,
                domain_state,
                text_engine,
                animation,
            ) {
                qualified.finish_delivery(true);
                #[cfg(any(target_arch = "wasm32", test))]
                {
                    browser.pending_paste = None;
                }
                geometry_effect_checkpoint(widget_tree, event_router, domain_state, None, None);
                continue;
            }
            // SPEC-HAL-021 Stage 5: Animation capability for all event types.
            // INV-TL-2: Router accepts but does not own animation.
            let animation = Some(transition_layer.animation_context());
            if domain_state.relative_pointer_authority.is_some() {
                event_router.route_session_input_event_with_relative_facts(
                    widget_tree,
                    qualified.clone(),
                    Some(input_root),
                    text_engine,
                    animation,
                    domain_state,
                    &mut app_root.context.registry.binding_registry,
                    relative_facts.expect("relative authority has a registered domain"),
                );
            } else {
                event_router.route_session_input_event(
                    widget_tree,
                    qualified.clone(),
                    Some(input_root),
                    text_engine,
                    animation,
                    domain_state,
                    &mut app_root.context.registry.binding_registry,
                );
            }
            let pending_focus = event_router.geometry_pending_focus_reveal();
            geometry_effect_checkpoint(
                widget_tree,
                event_router,
                domain_state,
                Some((active_layout_transitions, transition_layer)),
                pending_focus,
            );
            let gate = EventRouter::relative_ingress_gate(domain_state);
            input_fusion.reconcile_relative_ingress(gate.as_ref());
            #[cfg(target_arch = "wasm32")]
            if let Some(choice) = browser_text.and_then(|text| text.choice()) {
                if let Some(request) = event_router.take_host_choice_request()
                    && (event_router.current_host_choice(widget_tree, input_root, domain_state, event.window_id())
                        != Some(&request.lease) || !choice.arm(&request))
                {
                    event_router.retire_host_choice(&request.lease);
                }
                choice.retain(event_router.current_host_choice(
                    widget_tree, input_root, domain_state, event.window_id(),
                ));
            }
            #[cfg(target_os = "ios")]
            if let Some(window) = *window {
                if let Some(request) = event_router.take_host_choice_request() {
                    let lease = request.lease.clone();
                    if event_router.current_host_choice(widget_tree, input_root, domain_state, event.window_id())
                        != Some(&lease) || !ios_choice.present(window, request)
                    {
                        event_router.retire_host_choice(&lease);
                    }
                }
                ios_choice.retain(event_router.current_host_choice(
                    widget_tree, input_root, domain_state, event.window_id(),
                ));
            }
            #[cfg(target_arch = "wasm32")]
            if let (Some(text), Some(window)) = (*browser_text, *window)
                && let Some((motion, owner)) = event_router.take_touch_keyboard_activation()
                && text.trusted_touch_up(window.id(), motion.native_contact)
                && let Some(root) = app_root.browser_paste_root()
                && let Some((target, projection)) =
                    crate::core::event_router::browser_text::BrowserTextTarget::qualify(
                        widget_tree,
                        interaction_domains,
                        WindowId::PRIMARY,
                        root,
                    )
                && target.owner() == owner
                && text.activate(window.id(), motion.native_contact, &projection)
            {
                browser.session = Some(
                    crate::core::event_router::browser_text::BrowserTextSession::new(
                        target, projection,
                    ),
                );
            }
            #[cfg(all(target_os = "macos", debug_assertions))]
            if let Some(recorder) = engine_conformance {
                recorder.record_state("route", domain_state);
            }
        }
        if widget_tree.has_pending_widget_availability() {
            AppInputDrain::AvailabilityPending
        } else {
            AppInputDrain::Drained
        }
    }

    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    fn apply_host_choice(&mut self, reply: crate::core::event_router::host_choice::ChoiceReply) {
        let Some(state) = self.interaction_domains.resolve_window_mut(reply.lease.press.window) else {
            return;
        };
        let root = self.app_root.active_input_root_node();
        self.event_router.route_host_choice(
            self.widget_tree, root, state, reply, self.text_engine,
            Some(self.transition_layer.animation_context()),
        );
        let pending_focus = self.event_router.geometry_pending_focus_reveal();
        geometry_effect_checkpoint(
            self.widget_tree, self.event_router, state,
            Some((self.active_layout_transitions, self.transition_layer)),
            pending_focus,
        );
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl AppInputDelivery<'_> {
    fn receive_browser_input(
        &mut self,
        report: BrowserTextReport,
        fused: &mut Vec<SessionQualifiedInput>,
        deferred: &mut VecDeque<DeferredAppInput>,
        geometry_pending: bool,
        default_cursor_event: &mut bool,
    ) -> AppInputDrain {
        if let Some(session) = &self.browser.session {
            deferred.push_back(DeferredAppInput::Browser {
                source: session.source(),
                report,
            });
        }
        self.drain_browser_input(fused, deferred, geometry_pending, default_cursor_event)
    }

    fn drain_browser_input(
        &mut self,
        fused: &mut Vec<SessionQualifiedInput>,
        deferred: &mut VecDeque<DeferredAppInput>,
        geometry_pending: bool,
        default_cursor_event: &mut bool,
    ) -> AppInputDrain {
        loop {
            if geometry_pending
                || self.browser.geometry_pending
                || self.app_root.context.ui_scale.pending().is_some()
            {
                return AppInputDrain::GeometryPending;
            }
            self.input_fusion.flush_with_gesture_sessions(fused);
            let status = self.drain_ready(fused, default_cursor_event);
            if status != AppInputDrain::Drained {
                return status;
            }
            if self.app_root.context.ui_scale.pending().is_some() {
                return AppInputDrain::GeometryPending;
            }
            // A canvas paste keeps the ordinary frame intake. A proxy paste
            // also waits for earlier browser reports, including its shortcut.
            if self.browser.pending_paste.is_some()
                && !AppBrowserEditor::proxy_paste_pending(deferred)
            {
                self.settle_browser_paste();
                if self.widget_tree.has_pending_widget_availability() {
                    return AppInputDrain::AvailabilityPending;
                }
            }
            if matches!(deferred.front(), Some(DeferredAppInput::Raw(_))) {
                fuse_deferred_raw_prefix(
                    deferred,
                    fused,
                    self.input_fusion,
                    self.widget_tree,
                    self.interaction_domains,
                );
                continue;
            }
            match deferred.pop_front() {
                Some(DeferredAppInput::Browser { source, report }) => {
                    self.apply_browser_report(source, report, fused);
                }
                Some(DeferredAppInput::ProxyPaste) => self.settle_browser_paste(),
                Some(DeferredAppInput::HostChoice(reply)) => self.apply_host_choice(reply),
                Some(DeferredAppInput::Raw(_)) => unreachable!("the raw prefix was just drained"),
                None => {
                    self.refresh_browser_projection();
                    return if self.browser.geometry_pending {
                        AppInputDrain::GeometryPending
                    } else {
                        AppInputDrain::Drained
                    };
                }
            }
        }
    }

    fn settle_browser_paste(&mut self) {
        let Some(paste) = self.browser.pending_paste.take() else {
            return;
        };
        let result = self
            .app_root
            .browser_paste_root()
            .ok_or(crate::core::event_router::browser_paste::PasteRefusal::StaleOwner)
            .and_then(|root| {
                self.event_router.route_browser_paste(
                    self.widget_tree,
                    self.interaction_domains,
                    root,
                    paste,
                    self.text_engine,
                    Some(self.transition_layer.animation_context()),
                )
            });
        if let Err(reason) = result {
            tracing::warn!(?reason, "Browser Paste refused");
        }
    }

    fn current_browser_projection(
        &self,
    ) -> Option<(
        NodeId,
        crate::core::event_router::browser_text::BrowserTextProjection,
    )> {
        let session = self.browser.session.as_ref()?;
        let root = self.app_root.browser_paste_root()?;
        let projection =
            session
                .target
                .current_projection(self.widget_tree, self.interaction_domains, root)?;
        Some((root, projection))
    }

    fn publish_browser_projection(
        &mut self,
        projection: crate::core::event_router::browser_text::BrowserTextProjection,
    ) {
        if let Some(session) = self.browser.session.as_mut() {
            #[cfg(target_arch = "wasm32")]
            if let Some(text) = self.browser_text {
                text.project(&projection);
            }
            session.projection = projection;
        }
    }

    fn refresh_browser_projection(&mut self) {
        if self.browser.session.is_none() {
            return;
        }
        if let Some((_, projection)) = self.current_browser_projection() {
            self.publish_browser_projection(projection);
            self.browser.restore_canvas_focus = false;
        } else {
            self.retire_browser_session();
        }
    }

    fn retire_browser_session(&mut self) {
        self.browser.retire_session(
            #[cfg(target_arch = "wasm32")]
            self.browser_text,
        );
    }

    fn apply_browser_report(
        &mut self,
        source: BrowserTextSource,
        report: BrowserTextReport,
        fused: &mut Vec<SessionQualifiedInput>,
    ) {
        if !self
            .browser
            .session
            .as_ref()
            .is_some_and(|session| session.owns_source(&source))
        {
            return;
        }
        let Some((root, current)) = self.current_browser_projection() else {
            self.retire_browser_session();
            return;
        };
        let session = self
            .browser
            .session
            .as_mut()
            .expect("qualified editor lease");
        match report {
            BrowserTextReport::Key {
                mut key,
                shortcut_text,
            } => {
                use crate::core::event::{KeyCode, KeyState, KeyboardKeyReservation};
                if let RawInputEvent::Key {
                    key_code,
                    modifiers,
                    state,
                    repeat,
                    reservation,
                    ..
                } = &mut key
                {
                    *reservation = KeyboardKeyReservation::from_observed_key(
                        *key_code,
                        *modifiers,
                        *state == KeyState::Down,
                        *repeat,
                    );
                    self.browser.restore_canvas_focus |=
                        matches!(key_code, KeyCode::Escape | KeyCode::Tab);
                }
                let state = self.interaction_domains.primary_state_mut();
                let gate = EventRouter::relative_ingress_gate(state);
                for raw in std::iter::once(key).chain(shortcut_text) {
                    self.input_fusion.process_raw_with_gesture_sessions(
                        raw,
                        crate::core::event::InteractionDomainId::PRIMARY,
                        &mut state.gesture_registry,
                        self.widget_tree,
                        gate.as_ref(),
                        fused,
                    );
                }
                // The next drain settles these exact events, including the
                // original shortcut's Text half, before any later report.
            }
            BrowserTextReport::Viewport(visible_size) => {
                self.browser.geometry_pending |= self.browser.visible_size != visible_size;
                self.browser.visible_size = visible_size;
                self.event_router
                    .request_browser_caret_reveal(&session.target);
            }
            BrowserTextReport::Blur => {
                if current.composing {
                    self.event_router.route_browser_text(
                        self.widget_tree,
                        self.interaction_domains,
                        root,
                        &session.target,
                        &current,
                        &BrowserTextAction::Composition(crate::core::event::ImeEvent::Cancel),
                        self.text_engine,
                    );
                }
                let state = self.interaction_domains.primary_state_mut();
                state.keyboard_mode = KeyboardAuthorityMode::Navigation;
                self.event_router
                    .notify_capture_leave(self.widget_tree, state, self.text_engine);
                self.retire_browser_session();
            }
            edit => {
                let source_is_current = source.matches_projection(&current);
                session.projection = current;
                let action = match edit {
                    BrowserTextReport::Commit(_) if !source_is_current => {
                        session.end_composition(String::new())
                    }
                    _ if !source_is_current => None,
                    BrowserTextReport::BeforeInput => {
                        session.before_input();
                        None
                    }
                    BrowserTextReport::Input {
                        value,
                        kind,
                        composing,
                        selection,
                    } => session.after_input(&value, &kind, composing, selection),
                    BrowserTextReport::PreEdit(value) => Some(session.update_composition(value)),
                    BrowserTextReport::Commit(value) => session.end_composition(value),
                    BrowserTextReport::Select(anchor, head) => {
                        session.projection.select(anchor, head)
                    }
                    _ => unreachable!("non-edit reports were already handled"),
                };
                if let Some(action) = action
                    && let Some(projection) = self.event_router.route_browser_text(
                        self.widget_tree,
                        self.interaction_domains,
                        root,
                        &session.target,
                        &session.projection,
                        &action,
                        self.text_engine,
                    )
                {
                    session.projection = projection;
                }
            }
        }
    }
}

fn observe_raw_geometry_allocation(
    tree: &mut WidgetTree,
    root: NodeId,
    raw: &RawInputEvent,
    scale: f32,
) {
    if let RawInputEvent::Resize { width, height, .. } = raw {
        tree.observe_geometry_allocation(
            root,
            Constraints::loose(Size::new(*width, *height)),
            scale,
        );
    }
    if let RawInputEvent::ScaleFactorChanged { scale_factor, .. } = raw {
        tree.observe_geometry_scale(root, *scale_factor);
    }
}

impl Drop for SessionInputDelivery<'_> {
    fn drop(&mut self) {
        self.0.finish_delivery(true);
    }
}

fn touch_contact_identity(
    event: &FusedInputEvent,
) -> Option<(WindowId, crate::core::event::PointerId)> {
    match event {
        FusedInputEvent::TouchDown {
            window_id,
            pointer_id,
            ..
        }
        | FusedInputEvent::TouchMove {
            window_id,
            pointer_id,
            ..
        }
        | FusedInputEvent::TouchUp {
            window_id,
            pointer_id,
            ..
        }
        | FusedInputEvent::TouchCancel {
            window_id,
            pointer_id,
            ..
        } => Some((*window_id, *pointer_id)),
        _ => None,
    }
}

#[cfg(any(target_os = "ios", test))]
fn cancel_window_touch_input(
    app: &mut AppRoot,
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    fusion: &mut InputFusion,
    window_id: WindowId,
    pending: &mut Vec<crate::core::event::SessionQualifiedInput>,
    deferred: &mut VecDeque<DeferredAppInput>,
) {
    fusion.cancel_touch_contacts_with_gesture_sessions(window_id, Instant::now(), pending);
    let mut pointers = Vec::new();
    pending.retain(|input| {
        if let Some((window, pointer)) = touch_contact_identity(input.event()) {
            if window == window_id {
                pointers.push(pointer);
                input.finish_delivery(true);
                return false;
            }
        }
        true
    });
    pointers.extend(
        state
            .gesture_registry
            .active_pointers()
            .into_iter()
            .filter(|pointer| {
                state.gesture_registry.session_pointer_type(*pointer)
                    == Some(crate::core::event::PointerType::Touch)
            }),
    );
    pointers.sort_unstable_by_key(|pointer| pointer.0);
    pointers.dedup();
    for pointer in pointers {
        router.cancel_pointer_session_with_tree(
            tree,
            pointer,
            crate::core::event::CancellationReason::SystemCancel,
            state,
        );
    }
    deferred.retain(|input| !matches!(
        input,
        DeferredAppInput::Raw(RawInputEvent::Touch { window_id: window, .. })
            if *window == window_id
    ));
    app.modal_dismiss_touch_sequences
        .retain(|(window, _)| *window != window_id);
}

fn prepare_session_input_for_app(
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    input: &crate::core::event::SessionQualifiedInput,
) -> bool {
    if !router.prepare_session_input(tree, input, state) {
        return false;
    }
    let pending_focus = router.geometry_pending_focus_reveal();
    geometry_effect_checkpoint(tree, router, state, None, pending_focus);
    router.prepare_session_input(tree, input, state)
}

#[cfg(test)]
pub(crate) fn dispatch_session_input_to_app_root(
    app: &mut AppRoot,
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    input: &crate::core::event::SessionQualifiedInput,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
) -> bool {
    dispatch_session_input_to_app_root_with_reserved_action(
        app,
        tree,
        router,
        state,
        input,
        text_engine,
        None,
    )
}

struct SessionAppInputDispatch {
    handled: bool,
    captured_key: crate::core::event_router::CapturedAppKeyOutcome,
}

#[cfg(any(target_arch = "wasm32", test))]
impl SessionAppInputDispatch {
    fn blocks_browser_paste(
        &self,
        input: &crate::core::event::SessionQualifiedInput,
        state: &crate::core::event::InteractionDomainState,
    ) -> bool {
        use crate::core::event_router::CapturedAppKeyOutcome;
        if input.registered_global_shortcut().is_some() {
            return true;
        }
        if !self.handled {
            return false;
        }
        match self.captured_key {
            CapturedAppKeyOutcome::WidgetHandled => false,
            // `handled` also seals both-decline against replay. Only the existing
            // exact paired-Text receipt establishes actual App consumption here.
            CapturedAppKeyOutcome::AppFallback => matches!(
                input.event(), FusedInputEvent::KeyDown { time, .. }
                if state.swallow_text_at == Some(*time)
            ),
            // Clipboard reservation intentionally leaves its payload to the editor;
            // it is not an App command taking over the captured paste owner.
            CapturedAppKeyOutcome::NotEligible => !matches!(
                input.event(),
                FusedInputEvent::KeyDown {
                    reservation: Some(crate::core::event::KeyboardKeyReservation::Clipboard),
                    ..
                } | FusedInputEvent::KeyUp {
                    reservation: Some(crate::core::event::KeyboardKeyReservation::Clipboard),
                    ..
                }
            ),
        }
    }
}

impl From<bool> for SessionAppInputDispatch {
    fn from(handled: bool) -> Self {
        Self {
            handled,
            captured_key: crate::core::event_router::CapturedAppKeyOutcome::NotEligible,
        }
    }
}

#[cfg(test)]
fn dispatch_session_input_to_app_root_with_reserved_action(
    app: &mut AppRoot,
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    input: &crate::core::event::SessionQualifiedInput,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
    reserved_action: Option<&mut dyn FnMut(crate::core::event::KeyboardKeyReservation)>,
) -> bool {
    dispatch_session_input_to_app_root_with_captured_routing(
        app,
        tree,
        router,
        state,
        input,
        text_engine,
        reserved_action,
        None,
        None,
    )
    .handled
}

fn dispatch_session_input_to_app_root_with_captured_routing(
    app: &mut AppRoot,
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    input: &crate::core::event::SessionQualifiedInput,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
    reserved_action: Option<&mut dyn FnMut(crate::core::event::KeyboardKeyReservation)>,
    animation: Option<crate::core::animation::AnimationContext<'_>>,
    relative_facts: Option<crate::core::event_router::RelativePointerWindowFacts>,
) -> SessionAppInputDispatch {
    if !prepare_session_input_for_app(tree, router, state, input) {
        return true.into();
    }
    if matches!(input.event(), FusedInputEvent::TouchCancel { .. }) {
        if let Some(identity) = touch_contact_identity(input.event()) {
            app.modal_dismiss_touch_sequences.remove(&identity);
        }
        let input_root = app.active_input_root_node();
        router.route_session_input_event(
            tree,
            input.clone(),
            Some(input_root),
            text_engine,
            None,
            state,
            &mut app.context.registry.binding_registry,
        );
        return true.into();
    }
    if matches!(
        input.event(),
        FusedInputEvent::TouchMove { .. } | FusedInputEvent::TouchUp { .. }
    ) && state.touch_scroll_source().is_some_and(|source| {
        input
            .touch_motion()
            .is_some_and(|motion| motion.press == source.press)
    }) {
        // HAL-396: subsequent samples and final Up belong to the qualified
        // capture. The ordinary router still checks the current input root.
        return false.into();
    }
    if let FusedInputEvent::TextInput { time, .. } = input.event() {
        if state.swallow_text_at.take() == Some(*time) {
            input.finish_delivery(true);
            return true.into();
        }
    }
    use crate::core::event::{
        KeyboardCaptureInterruption, KeyboardCaptureInterruptionReason, KeyboardKeyReservation,
    };
    let physical_owner = EventRouter::physical_keyboard_capture_owner(tree, state);
    if let Some(key) = input.event().key_event() {
        let reservation = match input.event() {
            FusedInputEvent::KeyDown { reservation, .. }
            | FusedInputEvent::KeyUp { reservation, .. } => *reservation,
            _ => None,
        };
        if let Some(reservation) = reservation {
            if reservation == KeyboardKeyReservation::Clipboard && physical_owner.is_some() {
                if let FusedInputEvent::KeyDown { time, .. } = input.event() {
                    state.swallow_text_at = Some(*time);
                }
                return false.into();
            }
            if key.is_down {
                router.interrupt_physical_keyboard_capture(
                    tree,
                    state,
                    KeyboardCaptureInterruption {
                        reason: KeyboardCaptureInterruptionReason::Reserved(reservation),
                        key: Some(key),
                    },
                    text_engine,
                );
                if reservation == KeyboardKeyReservation::Fullscreen {
                    if let FusedInputEvent::KeyDown { time, .. } = input.event() {
                        state.swallow_text_at = Some(*time);
                    }
                }
                if let Some(action) = reserved_action {
                    action(reservation);
                }
            }
            input.finish_delivery(true);
            return true.into();
        }
        if physical_owner.is_some() {
            if key.is_down && key.key_code == crate::core::event::KeyCode::Escape {
                router.interrupt_physical_keyboard_capture(
                    tree,
                    state,
                    KeyboardCaptureInterruption {
                        reason: KeyboardCaptureInterruptionReason::Escape,
                        key: Some(key),
                    },
                    text_engine,
                );
                input.finish_delivery(true);
                return true.into();
            }
            if !key.is_down {
                return false.into();
            }
            let app_reserved = app.reserves_key(&key);
            let registered_global = input.registered_global_shortcut().is_some();
            if !app_reserved && !registered_global {
                return false.into();
            }
            router.interrupt_physical_keyboard_capture(
                tree,
                state,
                KeyboardCaptureInterruption {
                    reason: KeyboardCaptureInterruptionReason::AppConsumed,
                    key: Some(key),
                },
                text_engine,
            );
            if registered_global {
                if let FusedInputEvent::KeyDown { time, .. } = input.event() {
                    state.swallow_text_at = Some(*time);
                }
                if !app_reserved {
                    input.finish_delivery(true);
                    return true.into();
                }
            }
        }
    } else if physical_owner.is_some() && matches!(input.event(), FusedInputEvent::TextInput { .. })
    {
        return false.into();
    }
    if matches!(
        input.event(),
        FusedInputEvent::KeyDown {
            key_code: crate::core::event::KeyCode::Escape,
            ..
        }
    ) {
        let active_capture = state.keyboard_mode
            == crate::core::event::KeyboardAuthorityMode::Capture
            && app.active_modal_input_root_node().is_none_or(|modal_root| {
                state
                    .focus_manager
                    .focused()
                    .is_some_and(|owner| tree.is_descendant_of_node(owner, modal_root))
            });
        let action = if active_capture {
            Some(crate::runtime::actions::ActionId::RELEASE_CAPTURE)
        } else if app.overlay_root.is_empty() {
            Some(crate::runtime::actions::ActionId::EXIT_SCOPE)
        } else {
            None
        };
        let structural = action.is_some_and(|action| {
            router.handle_engine_action(
                tree,
                action,
                Some(app.active_input_root_node()),
                state,
                text_engine,
                None,
            )
        });
        if !structural {
            app.handle_fused_input_modal_first(input.event(), tree);
        }
        // Escape has completed its structural and Screen/App path. Do not
        // consult the widget again or deliver a second synthesized key.
        if let FusedInputEvent::KeyDown { time, .. } = input.event() {
            state.swallow_text_at = Some(*time);
        }
        prepare_session_input_for_app(tree, router, state, input);
        input.finish_delivery(true);
        return true.into();
    }
    // HAL328 §20: Capture-modified engine dispatch otherwise returns false;
    // the existing router still settles availability and every input phase.
    // Reservations, physical capture and modal/overlay routes above retain
    // their original precedence. Only this exact normal target is deferred.
    if app.modals.is_empty() && app.overlay_root.is_empty() {
        let root = app.active_input_root_node();
        if let Some(target) =
            crate::core::event_router::CapturedAppKeyTarget::qualify(tree, state, input, root)
        {
            let outcome = router.route_session_input_event_for_app(
                tree,
                input.clone(),
                root,
                text_engine,
                animation,
                state,
                &mut app.context.registry.binding_registry,
                &target,
                relative_facts,
            );
            finish_captured_app_key(
                app,
                tree,
                router,
                state,
                input,
                &target,
                outcome,
            );
            // Both-decline and stale qualification drop here, never replay.
            return SessionAppInputDispatch {
                handled: true,
                captured_key: outcome,
            };
        }
    }
    let consumed = app.handle_fused_input_modal_first(input.event(), tree);
    let current = prepare_session_input_for_app(tree, router, state, input);
    if consumed || !current {
        input.finish_delivery(true);
    }
    (consumed || !current).into()
}

fn finish_captured_app_key(
    app: &mut AppRoot,
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    input: &crate::core::event::SessionQualifiedInput,
    target: &crate::core::event_router::CapturedAppKeyTarget,
    outcome: crate::core::event_router::CapturedAppKeyOutcome,
) {
    use crate::core::event_router::CapturedAppKeyOutcome;
    if outcome != CapturedAppKeyOutcome::AppFallback
        || !app.modals.is_empty()
        || !app.overlay_root.is_empty()
        || !target.is_current(tree, state, input.event(), app.active_input_root_node())
    {
        return;
    }
    // The original widget dispatch and all tree/widget borrows have ended.
    let consumed = app.handle_fused_input_modal_first(input.event(), tree);
    if consumed {
        if let FusedInputEvent::KeyDown { time, .. } = input.event() {
            state.swallow_text_at = Some(*time);
        }
    }
    let current = prepare_session_input_for_app(tree, router, state, input);
    input.finish_delivery(consumed || !current);
}

pub(crate) fn settle_ui_scale_for_frame(
    context: &mut AppContext,
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    fusion: &mut InputFusion,
    root: NodeId,
    current_shaping_scale: f32,
    targets: &[(crate::compositor::SurfaceId, f32, (u32, u32))],
    refresh_metrics: bool,
) -> Result<Option<Vec<(crate::compositor::SurfaceId, f32, RectF)>>, UiScaleRejection> {
    let pending = context.ui_scale.pending();
    if pending.is_none() && !refresh_metrics {
        return Ok(None);
    }
    let requested = pending.unwrap_or_else(|| context.ui_scale.admitted());
    let mut prepared = Vec::with_capacity(targets.len());
    for &(surface, density, (width, height)) in targets {
        let metrics = crate::utils::coords::effective_scale_factor(density, requested.get())
            .and_then(|scale| {
                if width == 0 || height == 0 {
                    Ok((scale, RectF::new(0.0, 0.0, 0.0, 0.0)))
                } else {
                    crate::utils::coords::checked_logical_viewport(width, height, scale)
                        .map(|viewport| (scale, viewport))
                }
            });
        let Ok((scale, viewport)) = metrics else {
            let reason = UiScaleRejection::InvalidTargetMetrics { surface };
            if pending.is_none() {
                return Err(reason);
            }
            context.ui_scale.reject_pending(reason);
            return Ok(None);
        };
        prepared.push((surface, scale, viewport));
    }
    // The caller has routed the complete old normalized batch and Screen actions.
    router.cancel_active_relative_with_tree(
        tree,
        state,
        RelativePointerCancellationReason::NormalizationChanged,
        None,
    );
    fusion.discard_pending_relative_pointer();
    if let Some((_, scale, viewport)) = prepared.first() {
        let constraints = Constraints::loose(Size::new(viewport.width, viewport.height));
        tree.observe_geometry_allocation(root, constraints, *scale);
        router.retire_invalid_absolute_geometry(tree, state);
        if *scale != current_shaping_scale {
            // Layout's root and child caches can both still have equal constraints.
            tree.mark_subtree_dirty(root);
        } else if tree
            .node(root)
            .is_some_and(|node| node.last_constraints != Some(constraints))
        {
            tree.mark_layout_dirty(root);
        }
    }
    context.ui_scale.admit_pending();
    Ok(Some(prepared))
}

fn geometry_effect_checkpoint(
    tree: &mut WidgetTree,
    router: &mut EventRouter,
    state: &mut crate::core::event::InteractionDomainState,
    transitions: Option<(&ActiveLayoutTransitions, &TransitionLayer)>,
    pending_focus: Option<TreeWidgetId>,
) {
    router.retire_stale_touch_scroll(tree, state);
    let watches = state.gesture_registry.geometry_watches();
    if watches.is_empty() {
        return;
    }
    let mut effects = std::collections::HashSet::new();
    for (_, _, watch) in &watches {
        for (id, _) in &watch.path {
            if let Some(node) = tree.node(*id) {
                if let Some(scroll) = node.widget.as_any().downcast_ref::<ScrollArea>() {
                    if scroll.has_geometry_scroll_effect()
                        || tree
                            .node(scroll.content())
                            .is_some_and(|content| content.widget.has_requested_scroll())
                    {
                        effects.insert(*id);
                    }
                }
            }
        }
    }
    if let Some((transitions, layer)) = transitions {
        for (owner, transition) in transitions.iter() {
            if !layer.timeline().is_active(transition.handle) {
                continue;
            }
            if let Some(id) = tree.find_node_by_widget_id(*owner) {
                effects.insert(id);
            }
        }
    }
    if let Some(focus) = pending_focus.and_then(|owner| tree.find_node_by_widget_id(owner)) {
        let mut current = tree.parent(focus);
        while let Some(id) = current {
            let Some(node) = tree.node(id) else { break };
            if let Some(scroll) = node.widget.as_any().downcast_ref::<ScrollArea>() {
                let in_drag = node.widget.scroll_container().is_some_and(|container| {
                    container.input_mode() == crate::core::scroll_container::ScrollInputMode::Drag
                });
                if !in_drag && !scroll.is_anchor_dirty() {
                    if let (Some(rect), Some(origin)) = (
                        tree.get_absolute_paint_rect(focus),
                        tree.get_absolute_paint_rect(id),
                    ) {
                        let relative = RectF::new(
                            rect.x - origin.x,
                            rect.y - origin.y,
                            rect.width,
                            rect.height,
                        );
                        if scroll.reveal_would_change_geometry(
                            relative,
                            crate::core::event::ScrollAlign::Nearest,
                        ) {
                            effects.insert(id);
                        }
                    }
                }
                break;
            }
            current = node.parent;
        }
    }
    for id in effects {
        tree.record_geometry_origin(id);
    }
    router.retire_invalid_absolute_geometry(tree, state);
}

/// Consume one frame already begun by EngineTicker, including opted-in gesture
/// retirement after the normal Update and before Layout. This does not begin a frame.
#[allow(clippy::too_many_arguments)]
pub fn build_scene_with_gesture_sessions(
    widget_tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    frame_dt: f32,
    transition_layer: &mut TransitionLayer,
    active_layout_transitions: &mut ActiveLayoutTransitions,
    pending_focus_reveal: Option<TreeWidgetId>,
    scale_factor: f32,
    signals: Option<&mut SignalStore>,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: Option<&TooltipRenderState>,
    domains: &mut InteractionDomains,
    window: WindowId,
    router: &mut EventRouter,
    fusion: &mut InputFusion,
) -> SceneGraph {
    let mut retained_cache = RetainedSceneCache::default();
    let mut recycled_scene = None;
    let mut bindings = CustomRenderPresentationBindings::default();
    build_scene_reusing(
        widget_tree,
        root,
        constraints,
        theme,
        text_engine,
        icon_registry,
        frame_time_ms,
        frame_dt,
        transition_layer,
        active_layout_transitions,
        pending_focus_reveal,
        scale_factor,
        None,
        &mut bindings,
        signals,
        None,
        false,
        tooltip_triggers,
        TooltipSceneInput::Explicit(tooltip_render),
        None,
        KeyboardAuthorityMode::Navigation,
        None,
        true,
        &mut recycled_scene,
        &mut retained_cache,
        Some(GestureSceneAccess {
            domains,
            window,
            router,
            fusion,
        }),
        false,
    )
    .scene
    .expect("fresh build_scene_with_gesture_sessions must produce a scene")
}

#[allow(clippy::too_many_arguments)]
fn build_scene_reusing(
    widget_tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    frame_dt: f32,
    transition_layer: &mut TransitionLayer,
    active_layout_transitions: &mut ActiveLayoutTransitions,
    pending_focus_reveal: Option<TreeWidgetId>,
    scale_factor: f32,
    custom_render_target: Option<CustomRenderTargetSnapshot>,
    custom_render_presentation_bindings: &mut CustomRenderPresentationBindings,
    signals: Option<&mut SignalStore>,
    active_drag: Option<&crate::core::drag_drop::DragSession>,
    any_pointer_captured: bool,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: TooltipSceneInput<'_>,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: crate::core::event::KeyboardAuthorityMode,
    appearance_source: Option<(&InteractionDomains, WindowId)>,
    force_scene: bool,
    recycled_scene: &mut Option<RecycledSceneStorage>,
    retained_cache: &mut RetainedSceneCache,
    gesture_source: Option<GestureSceneAccess<'_>>,
    debug_outlines: bool,
) -> SceneBuildOutcome {
    match prepare_scene_paint(
        widget_tree,
        root,
        constraints,
        theme,
        text_engine,
        frame_time_ms,
        frame_dt,
        transition_layer,
        active_layout_transitions,
        pending_focus_reveal,
        scale_factor,
        custom_render_target,
        custom_render_presentation_bindings,
        signals,
        active_drag,
        any_pointer_captured,
        focused_widget,
        keyboard_mode,
        appearance_source,
        force_scene,
        retained_cache,
        gesture_source,
        None,
        debug_outlines,
    ) {
        SceneBuildContinuation::Deferred(outcome) => outcome,
        SceneBuildContinuation::Paint(continuation) => resume_scene_paint(
            continuation,
            widget_tree,
            theme,
            text_engine,
            icon_registry,
            transition_layer,
            tooltip_triggers,
            tooltip_render,
            recycled_scene,
            retained_cache,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_canonical_scene_reusing(
    widget_tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    renderer: &mut WgpuRenderer,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    frame_dt: f32,
    transition_layer: &mut TransitionLayer,
    active_layout_transitions: &mut ActiveLayoutTransitions,
    pending_focus_reveal: Option<TreeWidgetId>,
    scale_factor: f32,
    custom_render_target: Option<CustomRenderTargetSnapshot>,
    custom_render_presentation_bindings: &mut CustomRenderPresentationBindings,
    signals: Option<&mut SignalStore>,
    active_drag: Option<&crate::core::drag_drop::DragSession>,
    any_pointer_captured: bool,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: TooltipSceneInput<'_>,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: KeyboardAuthorityMode,
    appearance_source: Option<(&InteractionDomains, WindowId)>,
    force_scene: bool,
    recycled_scene: &mut Option<RecycledSceneStorage>,
    retained_cache: &mut RetainedSceneCache,
    gesture_source: Option<GestureSceneAccess<'_>>,
    pre_paint: Option<CanonicalPrePaintHook>,
    flat_viewport3d: FlatViewport3DFrameTransfer<'_>,
    debug_outlines: bool,
) -> SceneBuildOutcome {
    let continuation = prepare_scene_paint(
        widget_tree,
        root,
        constraints,
        theme,
        renderer.text_engine_mut(),
        frame_time_ms,
        frame_dt,
        transition_layer,
        active_layout_transitions,
        pending_focus_reveal,
        scale_factor,
        custom_render_target,
        custom_render_presentation_bindings,
        signals,
        active_drag,
        any_pointer_captured,
        focused_widget,
        keyboard_mode,
        appearance_source,
        force_scene,
        retained_cache,
        gesture_source,
        flat_viewport3d,
        debug_outlines,
    );
    match continuation {
        SceneBuildContinuation::Deferred(outcome) => outcome,
        SceneBuildContinuation::Paint(continuation) => {
            // Segment 1 returned owned data, so the text-engine borrow has ended.
            if let Some(prepare_resources) = pre_paint {
                let current = retained_cache.current_sdf_frame_inputs(widget_tree);
                let eligible = retained_cache.active_sdf_size_replacement_keys(&current);
                retained_cache.cancel_obsolete_sdf_composition(widget_tree, &eligible);
                prepare_resources(renderer, widget_tree, retained_cache);
                if let Some(staging) = retained_cache.sdf_material_staging.as_mut() {
                    staging
                        .admit_active_size_replacements_before_paint(renderer, &current, &eligible);
                    if current.iter().any(|input| {
                        staging.active_size_replacement_pending(renderer, input, &eligible)
                    }) {
                        return SceneBuildOutcome {
                            scene: None,
                            visual_changed: false,
                            needs_next_frame: false,
                            material_pending: true,
                            plot_evidence: continuation.plot_evidence,
                        };
                    }
                }
                retained_cache.commit_sdf_materials_before_paint(renderer, widget_tree);
                if !continuation.plot_evidence.iter().any(|plot| plot.pending_surface().is_some()) {
                    retained_cache.prepare_sdf_paint_inputs(widget_tree);
                }
            }
            resume_scene_paint(
                continuation,
                widget_tree,
                theme,
                renderer.text_engine_mut(),
                icon_registry,
                transition_layer,
                tooltip_triggers,
                tooltip_render,
                recycled_scene,
                retained_cache,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_scene_paint(
    widget_tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    frame_time_ms: u64,
    frame_dt: f32,
    transition_layer: &mut TransitionLayer,
    active_layout_transitions: &mut ActiveLayoutTransitions,
    pending_focus_reveal: Option<TreeWidgetId>,
    scale_factor: f32,
    custom_render_target: Option<CustomRenderTargetSnapshot>,
    custom_render_presentation_bindings: &mut CustomRenderPresentationBindings,
    signals: Option<&mut SignalStore>,
    active_drag: Option<&crate::core::drag_drop::DragSession>,
    any_pointer_captured: bool,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: KeyboardAuthorityMode,
    appearance_source: Option<(&InteractionDomains, WindowId)>,
    force_scene: bool,
    retained_cache: &mut RetainedSceneCache,
    mut gesture_source: Option<GestureSceneAccess<'_>>,
    flat_viewport3d: FlatViewport3DFrameTransfer<'_>,
    debug_outlines: bool,
) -> SceneBuildContinuation {
    if widget_tree.has_pending_widget_availability() {
        return SceneBuildContinuation::Deferred(SceneBuildOutcome {
            scene: None,
            visual_changed: false,
            needs_next_frame: true,
            material_pending: false,
            plot_evidence: Vec::new(),
        });
    }
    // INV-354-OUTLINE-DELTA: invalidate changed presentation before Layout/Paint.
    // The cache records only the last consumed immutable frame value.
    if retained_cache
        .context_key
        .is_some_and(|previous| previous.debug_outlines != debug_outlines)
    {
        widget_tree.mark_subtree_paint_dirty(root);
    }
    widget_tree.observe_geometry_allocation(root, constraints, scale_factor);
    if let Some(access) = gesture_source.as_mut() {
        if let Some(state) = access.domains.resolve_window_mut(access.window) {
            geometry_effect_checkpoint(
                widget_tree,
                access.router,
                state,
                Some((&*active_layout_transitions, &*transition_layer)),
                pending_focus_reveal,
            );
        }
    } else {
        assert!(
            widget_tree.geometry_protected_owners.is_empty(),
            "INV-ABSOLUTE-GEOMETRY-QUALIFIED-SCENE-HOST"
        );
    }
    if widget_tree.has_pending_widget_availability() {
        return SceneBuildContinuation::Deferred(SceneBuildOutcome {
            scene: None,
            visual_changed: false,
            needs_next_frame: true,
            material_pending: false,
            plot_evidence: Vec::new(),
        });
    }
    let mut appearance = gesture_source
        .as_ref()
        .and_then(|access| {
            FrameworkAppearanceSnapshot::capture(access.domains, access.window, widget_tree)
        })
        .or_else(|| {
            appearance_source.and_then(|(domains, window)| {
                FrameworkAppearanceSnapshot::capture(domains, window, widget_tree)
            })
        });
    let mut any_pointer_captured = any_pointer_captured;
    publish_framework_appearance(widget_tree, retained_cache, appearance.as_ref());
    let (focused_widget, keyboard_mode) = match (appearance_source, appearance.as_ref()) {
        (_, Some(snapshot)) => (snapshot.active_focused_owner(), snapshot.keyboard_mode()),
        (Some(_), None) => (None, KeyboardAuthorityMode::Navigation),
        (None, None) => (focused_widget, keyboard_mode),
    };
    #[cfg(test)]
    custom_render_presentation_bindings.reset_for_test();
    // =========================================================================
    // SPEC-HAL-131A/C ENGINE LAW: Frame Pipeline Ordering
    // =========================================================================
    // The following sequence is a STABLE INVARIANT. Reordering is forbidden
    // (except for pre-layout sync which is a targeted fix for virtualization).
    //
    //   0a. pre_layout_sync_scroll_to_lists()      — sync scroll offset to List widgets BEFORE layout
    //   1. layout_tree()                           — compute widget geometry
    //   2. post_layout_populate_input_projections()— build input projection snapshots
    //   3. post_layout_resolve_scroll_anchors()    — resolve scroll anchors
    //   4. post_layout_reveal_focused_widget()     — scroll focused widget into view (non-List)
    //   5. post_layout_sync_scroll_to_lists()      — sync scroll offset to List widgets (for anchor changes)
    //   6. paint_tree()                            — build scene graph
    //
    // PRE-LAYOUT INTENT RESOLUTION (step 0a): Resolves ScrollIntents from keyboard
    // navigation to ScrollArea offsets BEFORE pre_layout_sync runs. This ensures
    // Lists see the correct scroll offset for virtualization.
    //
    // PRE-LAYOUT SYNC (step 0b): Ensures List.visible_range matches current scroll_y
    // BEFORE layout runs. This prevents the "1-frame lag" where children are
    // positioned at old visible_range, then scroll transform moves them off-screen.
    //
    // POST-LAYOUT SYNC (step 4): Catches scroll changes from anchor resolution.
    // If anchor resolution changed scroll_y, Lists get updated for next frame.
    //
    // Anchor resolution MUST occur between layout and paint:
    // - Anchors depend on layout results (content extent, item positions)
    // - Paint depends on resolved scroll offset (no anchor access in paint)
    //
    // Input projection MUST occur after layout but before paint:
    // - INV-PROJECTION-SNAPSHOT-1: Computed during layout phase, read-only after
    // - INV-INPUT-PROJECTION-1: Input queries projection snapshot, not realized tree
    //
    // INV-ANCHOR-RESOLVE-1: Anchor resolved before layout callback fires
    // INV-ANCHOR-RESOLVE-2: Paint consumes resolved logical_offset only
    // =========================================================================

    // 0a. Pre-layout scroll→List sync (ensures visible_range matches scroll_y BEFORE layout)
    pre_layout_sync_scroll_to_lists(widget_tree);

    // 0c. Per-frame widget update (time-based state changes like auto-scroll)
    // SPEC-HAL-146: Update phase is the only legal time for visual state mutations.
    let mut presentation = retained_cache.presentation.take();
    if presentation.is_none() && widget_tree.presentation_inbox.is_some() {
        presentation = Some(Box::new(PresentationRuntime::default()));
    }
    {
        let _update_authority = UpdateAuthorityGuard::enter();
        if let Err(error) = active_layout_transitions.retire_stale_mounted(widget_tree, root, transition_layer) {
            tracing::debug!(?error, "HAL078 mounted retirement refused unqualified evidence");
        }
        pre_layout_update_widgets_with_geometry(
            widget_tree,
            root,
            frame_dt,
            transition_layer,
            theme,
            appearance.as_ref(),
            flat_viewport3d,
            Some(active_layout_transitions),
            scale_factor,
        );
        if let Some(access) = gesture_source.as_mut() {
            if let Some(state) = access.domains.resolve_window_mut(access.window) {
                geometry_effect_checkpoint(
                    widget_tree,
                    access.router,
                    state,
                    Some((&*active_layout_transitions, &*transition_layer)),
                    pending_focus_reveal,
                );
                let gate = EventRouter::relative_ingress_gate(state);
                access.fusion.reconcile_relative_ingress(gate.as_ref());
                any_pointer_captured = state.pointer_capture_manager.is_any_captured();
            }
            if widget_tree.has_pending_widget_availability() {
                retained_cache.qualified_sdf_requests = None;
                if let Some(runtime) = &mut presentation {
                    runtime.expire_frame();
                }
                retained_cache.presentation = presentation;
                return SceneBuildContinuation::Deferred(SceneBuildOutcome {
                    scene: None,
                    visual_changed: false,
                    needs_next_frame: true,
                    material_pending: false,
                    plot_evidence: Vec::new(),
                });
            }
            appearance =
                FrameworkAppearanceSnapshot::capture(access.domains, access.window, widget_tree);
            for owner in access.router.take_geometry_capture_changes(widget_tree) {
                if let (Some(snapshot), Some(node_id)) = (
                    appearance.as_ref(),
                    widget_tree.find_node_by_widget_id(owner),
                ) {
                    if let Some(node) = widget_tree.node_mut(node_id) {
                        if node.widget.absolute_drag_geometry_dependency().is_some() {
                            node.widget.settle_absolute_drag_geometry_appearance(
                                snapshot.for_owner(owner),
                            );
                        }
                    }
                    widget_tree.mark_paint_dirty(node_id);
                }
            }
            publish_framework_appearance(widget_tree, retained_cache, appearance.as_ref());
        }
        if let Some(runtime) = &mut presentation {
            let window_active = widget_tree
                .presentation_inbox
                .as_ref()
                .is_some_and(|inbox| {
                    appearance.as_ref().is_some_and(|snapshot| {
                        let facts = snapshot.for_owner(inbox.configuration.owner);
                        facts.window_active && facts.interaction_domain_active
                    })
                });
            runtime.update(
                widget_tree,
                transition_layer,
                active_layout_transitions,
                frame_time_ms,
                !any_pointer_captured
                    && window_active
                    && !widget_tree.has_blocking_overlay_hit_region(),
            );
        }
    }
    let active_drag = gesture_source
        .as_ref()
        .and_then(|access| {
            access
                .domains
                .state(access.domains.domain_for_window(access.window)?)?
                .drag_drop
                .session()
        })
        .or(active_drag)
        .map(RetainedDragState::capture);
    // SPEC-HAL-021: Mark layout phase started (debug invariant ANIM-INV-4).
    // After this point, Timeline must not be mutated until paint completes.
    #[cfg(debug_assertions)]
    crate::core::animation::debug_mark_layout_started();

    // SPEC-HAL-141: Enter layout authority scope.
    // All rect mutations must happen within this scope.
    let _layout_authority = LayoutAuthorityGuard::enter();

    // 1. Layout (with profiling)
    let layout_start = web_time::Instant::now();
    #[cfg(debug_assertions)]
    let hal_pass_probe_enabled = hal_pass_probe::is_enabled();
    #[cfg(debug_assertions)]
    if hal_pass_probe_enabled {
        hal_pass_probe::reset();
    }
    layout_tree(widget_tree, root, constraints, theme, text_engine, signals);
    widget_tree.complete_geometry_allocation();
    // SPEC-HAL-366: Publish first-tight retained X before input/paint projections.
    widget_tree.sync_linked_horizontal_scroll_projections();
    resize_profiler::add_layout_time_us(layout_start.elapsed().as_micros() as u64);
    #[cfg(debug_assertions)]
    if hal_pass_probe_enabled {
        let hal_layout_us = layout_start.elapsed().as_micros() as u64;
        if hal_layout_us > 8_000 {
            hal_pass_probe::dump(hal_layout_us);
        }
    }
    // HAL_DUMP_LAYOUT / HAL_ASSERT_CONTAINMENT: Post-layout analysis
    dump_layout_snapshot(widget_tree, root);
    assert_containment(widget_tree, root);
    // 2. Post-layout input projection population (SPEC-HAL-131C)
    post_layout_populate_input_projections(widget_tree);

    // 3. Post-layout anchor resolution (SPEC-HAL-131A Stage 14)
    post_layout_resolve_scroll_anchors(widget_tree);

    // 4. Post-layout focus reveal (scroll focused widget into view)
    if let Some(focus_id) = pending_focus_reveal {
        post_layout_reveal_focused_widget(widget_tree, focus_id);
    }

    // 5. Post-layout scroll→List sync (SPEC-HAL-134)
    let post_layout_scroll_sync = post_layout_sync_scroll_to_lists(widget_tree);
    if let Some(access) = gesture_source.as_ref() {
        if let Some(state) = access.domains.state(
            access
                .domains
                .domain_for_window(access.window)
                .expect("registered geometry scene window"),
        ) {
            for (_, _, watch) in state.gesture_registry.geometry_watches() {
                assert!(
                    widget_tree.absolute_drag_geometry_is_current(&watch),
                    "INV-ABSOLUTE-GEOMETRY-COMPLETE-DOMAIN: unadmitted basis change after retirement checkpoint"
                );
            }
        }
    }
    // SPEC-HAL-141: Exit layout authority scope before transition/paint.
    // Only layout phase may mutate rects via set_rect.
    drop(_layout_authority);

    // SPEC-HAL-021: Mark layout phase ended (debug invariant ANIM-INV-4).
    // Timeline can now be mutated again — transition binding may start animations.
    // Moved here from after paint: layout is done when LayoutAuthority drops.
    // Transition binding (below) legally starts animations post-layout.
    #[cfg(debug_assertions)]
    crate::core::animation::debug_mark_layout_ended();

    if post_layout_scroll_sync.layout_deferred || widget_tree.has_pending_widget_availability() {
        retained_cache.qualified_sdf_requests = None;
        if let Some(runtime) = &mut presentation {
            runtime.expire_frame();
        }
        retained_cache.presentation = presentation;
        return SceneBuildContinuation::Deferred(SceneBuildOutcome {
            scene: None,
            visual_changed: false,
            needs_next_frame: true,
            material_pending: false,
            plot_evidence: Vec::new(),
        });
    }

    if !widget_tree.complete_layout_transition_layout(root) {
        tracing::debug!("HAL078 final layout could not authenticate complete geometry history");
    }

    // =========================================================================
    // SPEC-HAL-078 FIX: Candidate Collection/Binding (AFTER layout)
    // =========================================================================
    // Candidates are now collected AFTER layout so `to_rect` equals authoritative
    // post-layout geometry. This fixes the stale `to_rect` bug.
    //
    // Frame order: Layout → Collect → Bind → Interpolate → Paint
    //
    // SPEC-HAL-145 / SPEC-HAL-078A DIRECT-MANIPULATION GATE: When any pointer
    // is captured, the user is direct-manipulating (SplitPane drag, scrollbar
    // drag, slider drag, text selection drag, etc.). Layout transitions exist
    // to smooth NON-user-driven layout changes (window resize, theme switch,
    // screen transition, panel toggle). Binding transitions during direct
    // manipulation introduces a 300ms-eased lag between input and visual
    // response. Skip the entire binding block; in-flight transitions from
    // earlier frames continue under their existing handles.
    // =========================================================================
    if !any_pointer_captured {
        // FIX 2: Compute conflict flags for CONFLICT-1 enforcement
        // This determines which widgets are inside scroll containers, virtualized, etc.
        let mut conflict_flags = std::collections::HashMap::new();
        widget_tree.compute_conflict_flags(&mut conflict_flags);

        // Collect candidates (compares current frame vs previous frame geometry)
        let mut layout_transition_candidates = Vec::new();
        widget_tree.collect_layout_transition_candidates(&mut layout_transition_candidates);

        // Build authorization context
        let auth_ctx = AuthorizationContext::minimal(widget_tree.layout_transition_store());

        // Unified resolver: authorization → CONFLICT-1 → Stage 1 (hierarchy) → Stage 2 (spatial)
        #[cfg(debug_assertions)]
        let resolved_candidates = resolve_layout_transition_candidates(
            layout_transition_candidates,
            &auth_ctx,
            widget_tree,
            &conflict_flags,
            current_layout_epoch(),
        );
        #[cfg(not(debug_assertions))]
        let resolved_candidates = resolve_layout_transition_candidates(
            layout_transition_candidates,
            &auth_ctx,
            widget_tree,
            &conflict_flags,
        );

        // FIX 3: Reflow safety gate - filter out candidates whose widgets have dirty_layout
        // SPEC-HAL-145/146: If a widget participated in reflow, don't bind transition.
        // dirty_layout == true after layout means widget was skipped or has stale geometry.
        let safe_candidates: Vec<_> = resolved_candidates
            .candidates()
            .iter()
            .filter(|c| {
                if presentation
                    .as_ref()
                    .is_some_and(|runtime| runtime.claimed.contains(&c.id))
                {
                    return false;
                }
                if let Some(node_id) = widget_tree.find_by_id(c.id) {
                    if let Some(node) = widget_tree.node(node_id) {
                        // Only bind if dirty_layout is false (widget was properly laid out)
                        return !node.dirty_layout;
                    }
                }
                false // Unknown widget - don't bind
            })
            .copied()
            .collect();

        // Bind candidates to TransitionLayer (creates real animations)
        if let Err(error) =
            active_layout_transitions.bind_candidates_mounted(&safe_candidates, transition_layer, widget_tree)
        {
            tracing::error!(
                %error,
                "layout transition admission rejected; authoritative layout remains active"
            );
        }
    }
    // =========================================================================
    // SPEC-HAL-145: Transition Interpolation Phase
    // =========================================================================
    // Pre-compute interpolated visual rects for all active layout transitions.
    // This phase runs AFTER layout and BEFORE paint, under TransitionAuthority.
    //
    // Frame order: Layout → Transition → Paint
    // - Layout: set_rect under LayoutAuthority (done above)
    // - Transition: lerp_rect under TransitionAuthority (this phase)
    // - Paint: read pre-computed rects, no authority (below)
    //
    // INV-AUTH-MUTUAL-EXCLUSIVE: Layout and Transition are mutually exclusive.
    // INV-TRANSITION-NO-RECT-ASSIGN: Transition MUST NOT call set_rect.
    // INV-TRANSITION-INTERPOLATE-ONLY: Transition may only lerp between snapshots.
    // =========================================================================
    let interpolated_rects = {
        let _transition_authority = TransitionAuthorityGuard::enter();
        compute_current_layout_interpolated_visual_rects(
            active_layout_transitions, transition_layer.timeline(), widget_tree,
        )
        // TransitionAuthorityGuard drops here - before paint begins
    };
    retained_cache.qualify_sdf_requests_after_layout(widget_tree, root, custom_render_target);
    widget_tree.prepare_cell_edges(root, appearance.as_ref());
    let custom_render_outcome = custom_render_target
        .map_or_else(
            || Ok(Default::default()),
            |target| {
                widget_tree
                    .prepare_custom_render_participants(target, custom_render_presentation_bindings)
            },
        )
        .unwrap_or_else(|error| panic!("SPEC-HAL-266A: custom render preparation failed: {error}"));
    // Keep presentation cache-owned across the yield, including cancellation.
    retained_cache.presentation = presentation;
    SceneBuildContinuation::Paint(ScenePaintContinuation {
        tree_lifetime: widget_tree.snapshot_tree_lifetime(),
        root,
        root_owner: widget_tree.node(root).map(|node| node.id),
        frame_time_ms,
        scale_factor,
        custom_render_target,
        appearance,
        focused_widget,
        keyboard_mode,
        active_drag,
        interpolated_rects,
        front_publication_changed: custom_render_outcome.front_publication_changed,
        plot_evidence: custom_render_outcome.plot_evidence,
        force_scene,
        debug_outlines,
    })
}

#[allow(clippy::too_many_arguments)]
fn resume_scene_paint(
    continuation: ScenePaintContinuation,
    widget_tree: &mut WidgetTree,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    transition_layer: &TransitionLayer,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: TooltipSceneInput<'_>,
    recycled_scene: &mut Option<RecycledSceneStorage>,
    retained_cache: &mut RetainedSceneCache,
) -> SceneBuildOutcome {
    let ScenePaintContinuation {
        tree_lifetime,
        root,
        root_owner,
        frame_time_ms,
        scale_factor,
        custom_render_target,
        appearance,
        focused_widget,
        keyboard_mode,
        debug_outlines,
        active_drag,
        interpolated_rects,
        front_publication_changed,
        plot_evidence,
        force_scene,
    } = continuation;
    assert_eq!(
        widget_tree.snapshot_tree_lifetime(),
        tree_lifetime,
        "scene continuation changed tree"
    );
    assert_eq!(
        widget_tree.node(root).map(|node| node.id),
        root_owner,
        "scene continuation changed root"
    );
    if plot_evidence.iter().any(|plot| plot.pending_surface().is_some()) {
        return SceneBuildOutcome {
            scene: None,
            visual_changed: false,
            needs_next_frame: false,
            material_pending: false,
            plot_evidence,
        };
    }
    let mut presentation = retained_cache.presentation.take();
    // 6. Build scene
    // SPEC-HAL-145: Paint uses pre-computed interpolated rects, no rect authority active.
    // SPEC-HAL-146: Paint phase authority permits visual state mutations (animation reads, caret blink).
    // INV-PAINT-NO-RECT-MUTATE: Paint phase may not mutate rects under any authority.
    let paint_outcome = {
        let _paint_authority = PaintAuthorityGuard::enter();
        paint_tree_reusing(
            widget_tree,
            root,
            theme,
            text_engine,
            icon_registry,
            frame_time_ms,
            transition_layer.timeline(),
            &interpolated_rects,
            scale_factor,
            active_drag.as_ref(),
            tooltip_triggers,
            tooltip_render,
            focused_widget,
            keyboard_mode,
            appearance.as_ref(),
            force_scene
                || front_publication_changed
                || presentation.as_ref().is_some_and(|runtime| runtime.changed),
            recycled_scene,
            retained_cache,
            presentation.as_deref_mut(),
            custom_render_target,
            debug_outlines,
        )
    };

    retained_cache.finish_sdf_scene_paint(paint_outcome.scene.as_ref());

    // SPEC-HAL-086 Stage 8: Set overlay hit regions from paint-collected data
    widget_tree.publish_overlay_hit_entries(paint_outcome.overlay_hit_regions);
    let presentation_changed = presentation.as_ref().is_some_and(|runtime| runtime.changed);
    retained_cache.presentation = presentation;

    // SPEC-HAL-087 Stage 2: Verify tree consistency after scene build
    #[cfg(debug_assertions)]
    widget_tree.verify_all_nodes_consistent();

    SceneBuildOutcome {
        scene: paint_outcome.scene,
        visual_changed: paint_outcome.visual_changed
            || front_publication_changed
            || presentation_changed,
        needs_next_frame: false,
        material_pending: false,
        plot_evidence,
    }
}

#[cfg(test)]
pub(crate) use crate::core::widget_availability::apply_pending_widget_availability as settle_test_widget_availability;

#[cfg(test)]
pub(crate) struct RetainedSceneTestHarness {
    cache: RetainedSceneCache,
    recycled_scene: Option<RecycledSceneStorage>,
    transition_layer: TransitionLayer,
    active_layout_transitions: ActiveLayoutTransitions,
    custom_render_presentation_bindings: CustomRenderPresentationBindings,
    frame_time_ms: u64,
}

#[cfg(test)]
impl RetainedSceneTestHarness {
    pub(crate) fn new() -> Self {
        Self {
            cache: RetainedSceneCache::default(),
            recycled_scene: None,
            transition_layer: TransitionLayer::default(),
            active_layout_transitions: ActiveLayoutTransitions::default(),
            custom_render_presentation_bindings: CustomRenderPresentationBindings::default(),
            frame_time_ms: 0,
        }
    }

    pub(crate) fn advance_transition_time(&mut self, now_secs: f64) {
        self.transition_layer.advance(now_secs);
        self.frame_time_ms = (now_secs * 1000.0) as u64;
    }

    pub(crate) fn checkpoint_gesture_geometry(
        &self,
        tree: &mut WidgetTree,
        router: &mut EventRouter,
        state: &mut crate::core::event::InteractionDomainState,
    ) {
        geometry_effect_checkpoint(
            tree,
            router,
            state,
            Some((&self.active_layout_transitions, &self.transition_layer)),
            None,
        );
    }

    pub(crate) fn animation_context(&mut self) -> crate::core::animation::AnimationContext<'_> {
        self.transition_layer.animation_context()
    }

    pub(crate) fn interpolated_rect(&self, widget_id: TreeWidgetId) -> Option<RectF> {
        let _guard = crate::core::rect_authority::TransitionAuthorityGuard::enter();
        compute_interpolated_visual_rects(
            &self.active_layout_transitions,
            self.transition_layer.timeline(),
        )
        .get(&widget_id)
        .copied()
    }

    pub(crate) fn custom_render_presentation_binding_stats(&self) -> (usize, usize) {
        (
            self.custom_render_presentation_bindings.len(),
            self.custom_render_presentation_bindings.capacity(),
        )
    }

    pub(crate) fn custom_render_presentation_bindings(&self) -> &CustomRenderPresentationBindings {
        &self.custom_render_presentation_bindings
    }

    pub(crate) fn custom_render_presentation_bindings_mut(
        &mut self,
    ) -> &mut CustomRenderPresentationBindings {
        &mut self.custom_render_presentation_bindings
    }

    pub(crate) fn image_dependency_counts(&self) -> Option<(usize, usize)> {
        self.cache
            .image_dependencies
            .as_ref()
            .map(|dependencies| (dependencies.by_owner.len(), dependencies.by_identity.len()))
    }

    pub(crate) fn overlay_fragment(&self, owner: TreeWidgetId) -> Option<Arc<[SceneNode]>> {
        self.cache
            .entries
            .get(&owner)
            .map(|entry| entry.overlay.clone())
    }

    pub(crate) fn local_fragment(&self, owner: TreeWidgetId) -> Option<Arc<[SceneNode]>> {
        self.cache
            .entries
            .get(&owner)
            .map(|entry| Arc::clone(&entry.local))
    }

    pub(crate) fn frame(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
    ) -> RetainedSceneTestFrame {
        self.frame_with_optional_custom_render_target(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            None,
            None,
        )
    }

    pub(crate) fn frame_with_custom_render_target(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
        custom_render_target: CustomRenderTargetSnapshot,
    ) -> RetainedSceneTestFrame {
        self.frame_with_optional_custom_render_target(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            Some(custom_render_target),
            None,
        )
    }

    pub(crate) fn frame_with_appearance(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
        domains: &InteractionDomains,
        window: WindowId,
    ) -> RetainedSceneTestFrame {
        self.frame_with_optional_custom_render_target(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            None,
            Some((domains, window)),
        )
    }

    pub(crate) fn frame_with_gesture_sessions(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
        frame_time: &FrameTime,
        pending_focus_reveal: Option<TreeWidgetId>,
        domains: &mut InteractionDomains,
        window: WindowId,
        router: &mut EventRouter,
        fusion: &mut InputFusion,
    ) -> RetainedSceneTestFrame {
        self.frame_time_ms += frame_time.dt.as_millis() as u64;
        self.transition_layer
            .advance(self.frame_time_ms as f64 / 1000.0);
        let mut tooltip_triggers = Vec::new();
        let outcome = build_scene_reusing(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            self.frame_time_ms,
            frame_time.dt.as_secs_f32(),
            &mut self.transition_layer,
            &mut self.active_layout_transitions,
            pending_focus_reveal,
            1.0,
            None,
            &mut self.custom_render_presentation_bindings,
            None,
            None,
            false,
            &mut tooltip_triggers,
            TooltipSceneInput::Explicit(None),
            None,
            KeyboardAuthorityMode::Navigation,
            None,
            false,
            &mut self.recycled_scene,
            &mut self.cache,
            Some(GestureSceneAccess {
                domains,
                window,
                router,
                fusion,
            }),
            false,
        );
        let scene_built = outcome.scene.is_some();
        let scene_root = outcome
            .scene
            .as_ref()
            .map(|scene| scene.root.clone())
            .unwrap_or_default();
        if let Some(scene) = outcome.scene {
            self.recycled_scene = Some(scene.into_recycled_storage());
        }
        RetainedSceneTestFrame {
            scene_built,
            visual_changed: outcome.visual_changed,
            needs_next_frame: outcome.needs_next_frame,
            cache_entries: self.cache.entries.len(),
            scene_root,
            tooltip_triggers,
            plot_evidence: outcome.plot_evidence,
        }
    }

    pub(crate) fn frame_with_tooltips(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
        manager: &mut crate::model::tooltip::TooltipManager,
        pointer: Point,
        dt: f32,
    ) -> (RetainedSceneTestFrame, bool) {
        self.frame_time_ms += (dt * 1000.0) as u64;
        let target = advance_tooltip_manager(manager, pointer, dt, self.frame_time_ms, false);
        let frame = self.frame_with_scene_inputs(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            None,
            None,
            TooltipSceneInput::Managed {
                visible_target: target.as_ref(),
                pointer,
                viewport: RectF::new(0.0, 0.0, constraints.max_width, constraints.max_height),
            },
            false,
        );
        let pending = if frame.needs_next_frame {
            false
        } else {
            publish_tooltip_triggers(manager, frame.tooltip_triggers.clone(), pointer)
        };
        (frame, pending || manager.needs_tick())
    }

    pub(crate) fn frame_with_optional_custom_render_target(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
        custom_render_target: Option<CustomRenderTargetSnapshot>,
        appearance_source: Option<(&InteractionDomains, WindowId)>,
    ) -> RetainedSceneTestFrame {
        self.frame_with_scene_inputs(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            custom_render_target,
            appearance_source,
            TooltipSceneInput::Explicit(None),
            false,
        )
    }

    fn frame_with_app_context(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        app: &AppContext,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
    ) -> RetainedSceneTestFrame {
        self.frame_with_scene_inputs(
            widget_tree,
            root,
            constraints,
            &app.theme,
            text_engine,
            icon_registry,
            None,
            None,
            TooltipSceneInput::Explicit(None),
            app.debug_outlines_enabled(),
        )
    }

    fn frame_with_scene_inputs(
        &mut self,
        widget_tree: &mut WidgetTree,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        text_engine: &mut dyn TextEngine,
        icon_registry: &crate::core::icon::IconRegistry,
        custom_render_target: Option<CustomRenderTargetSnapshot>,
        appearance_source: Option<(&InteractionDomains, WindowId)>,
        tooltip_render: TooltipSceneInput<'_>,
        debug_outlines: bool,
    ) -> RetainedSceneTestFrame {
        test_begin_frame_layout_tracking();
        let mut tooltip_triggers = Vec::new();
        let scale_factor = custom_render_target
            .map(CustomRenderTargetSnapshot::scale_factor)
            .unwrap_or(1.0);
        let outcome = build_scene_reusing(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            self.frame_time_ms,
            0.0,
            &mut self.transition_layer,
            &mut self.active_layout_transitions,
            None,
            scale_factor,
            custom_render_target,
            &mut self.custom_render_presentation_bindings,
            None,
            None,
            false,
            &mut tooltip_triggers,
            tooltip_render,
            None,
            KeyboardAuthorityMode::Navigation,
            appearance_source,
            false,
            &mut self.recycled_scene,
            &mut self.cache,
            None,
            debug_outlines,
        );
        let scene_built = outcome.scene.is_some();
        let scene_root = outcome
            .scene
            .as_ref()
            .map(|scene| scene.root.clone())
            .unwrap_or_default();
        if let Some(scene) = outcome.scene {
            self.recycled_scene = Some(scene.into_recycled_storage());
        }
        RetainedSceneTestFrame {
            scene_built,
            visual_changed: outcome.visual_changed,
            needs_next_frame: outcome.needs_next_frame,
            cache_entries: self.cache.entries.len(),
            scene_root,
            tooltip_triggers,
            plot_evidence: outcome.plot_evidence,
        }
    }
    /// Internal proof seam over the actual canonical Update/Layout/preparation owner.
    /// Public App::run composition is proved separately at the external boundary.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn frame_with_flat_viewport3d_transfers(
        &mut self,
        widget_tree: &mut WidgetTree,
        app: &mut AppContext,
        root: NodeId,
        constraints: Constraints,
        theme: &Theme,
        renderer: &mut WgpuRenderer,
        icon_registry: &crate::core::icon::IconRegistry,
        dt: f32,
        custom_render_target: Option<CustomRenderTargetSnapshot>,
        frame: Option<crate::core::frame_realization::AppFrameAttemptId>,
    ) -> RetainedSceneTestFrame {
        test_begin_frame_layout_tracking();
        self.frame_time_ms += (f64::from(dt) * 1000.0) as u64;
        self.transition_layer
            .advance(self.frame_time_ms as f64 / 1000.0);
        let mut tooltip_triggers = Vec::new();
        let scale = custom_render_target.map_or(1.0, CustomRenderTargetSnapshot::scale_factor);
        let debug_outlines = app.debug_outlines_enabled();
        let outcome = build_canonical_scene_reusing(
            widget_tree,
            root,
            constraints,
            theme,
            renderer,
            icon_registry,
            self.frame_time_ms,
            dt,
            &mut self.transition_layer,
            &mut self.active_layout_transitions,
            None,
            scale,
            custom_render_target,
            &mut self.custom_render_presentation_bindings,
            Some(&mut app.signals),
            None,
            false,
            &mut tooltip_triggers,
            TooltipSceneInput::Explicit(None),
            None,
            KeyboardAuthorityMode::Navigation,
            None,
            false,
            &mut self.recycled_scene,
            &mut self.cache,
            None,
            None,
            app.flat_viewport3d_transfers
                .as_deref_mut()
                .map(|owner| (owner, frame)),
            debug_outlines,
        );
        let scene_built = outcome.scene.is_some();
        let scene_root = outcome
            .scene
            .as_ref()
            .map(|scene| scene.root.clone())
            .unwrap_or_default();
        if let Some(scene) = outcome.scene {
            self.recycled_scene = Some(scene.into_recycled_storage());
        }
        RetainedSceneTestFrame {
            scene_built,
            visual_changed: outcome.visual_changed,
            needs_next_frame: outcome.needs_next_frame,
            cache_entries: self.cache.entries.len(),
            scene_root,
            tooltip_triggers,
            plot_evidence: outcome.plot_evidence,
        }
    }
}

#[cfg(test)]
pub(crate) struct RetainedSceneTestFrame {
    pub scene_built: bool,
    pub visual_changed: bool,
    pub needs_next_frame: bool,
    pub cache_entries: usize,
    pub scene_root: Vec<SceneNode>,
    pub tooltip_triggers: Vec<crate::model::tooltip::TooltipTrigger>,
    pub(crate) plot_evidence: Vec<crate::plot::PlotPreparationEvidence>,
}

/// SPEC-HAL-042 Phase-2: Executes a full engine frame.
///
/// Canonical frame order:
/// 1. Layout (computes widget sizes and positions)
/// 2. Paint (builds scene graph from widget tree)
/// 3. Render (submits scene to GPU)
///
/// Note: Input processing (InputFusion::process_frame + event routing) must be
/// called BEFORE this function. This function handles layout → paint → render only.
///
/// SPEC-HAL-044: Now delegates to build_scene for layout/paint, then renders.
/// SPEC-HAL-084: DrawPlanBuilder now owns Build phase (SceneGraph consumption).
///
/// # Arguments
///
/// * `compositor` - Compositor with at least one surface created. The surface_id from
///   `compositor.create_surface()` must match what was registered via `renderer.register_render_target()`.
pub fn run_single_frame(
    widget_tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    icon_registry: &crate::core::icon::IconRegistry,
    renderer: &mut WgpuRenderer,
    draw_plan_builder: &mut crate::renderer::DrawPlanBuilder,
    compositor: &crate::compositor::Compositor,
) {
    // SPEC-HAL-161: Signal delivery is forbidden during layout/scene/paint.
    let _frame_critical_section = FrameCriticalSectionGuard::enter();

    // Build scene (layout + paint)
    // Note: frame_time_ms = 0 since this is a single-frame render without ticker
    // SPEC-HAL-021 Stage 3: Empty TransitionLayer for single-frame render (no animations)
    // SPEC-HAL-078 Stage 3: Empty ActiveLayoutTransitions for single-frame render
    let mut transition_layer = crate::core::transition::TransitionLayer::new();
    let mut active_layout_transitions = ActiveLayoutTransitions::new();
    let scene = {
        let text_engine = renderer.text_engine_mut();
        let mut tooltip_triggers = Vec::new();
        build_scene(
            widget_tree,
            root,
            constraints,
            theme,
            text_engine,
            icon_registry,
            0,
            0.016, // Default dt for single-frame render
            &mut transition_layer,
            &mut active_layout_transitions,
            None,  // No focus reveal in single-frame render
            1.0,   // Default DPR for single-frame render
            None,  // No signals in single-frame render
            None,  // No active drag in single-frame render
            false, // No active pointer capture in single-frame render
            &mut tooltip_triggers,
            None, // No tooltip in single-frame render
            None, // No focused widget in single-frame render
            crate::core::event::KeyboardAuthorityMode::Navigation,
        )
    };

    // SPEC-HAL-081B: Use caller's compositor for multi-surface rendering
    // The compositor must have a surface whose ID matches a registered render target.
    let surface_stack = compositor.snapshot_multi();

    // ========================================================================
    // SPEC-HAL-163: Per-surface scene ownership (INV-SCENE-001)
    // ========================================================================
    // INVARIANT: SceneGraphs must NEVER be cloned across surfaces.
    // Any reintroduction of `scene.clone()` here is an invariant violation.
    //
    // Enforcement: Primary surface receives scene via Option::take() (move).
    // All other surfaces receive SceneGraph::new() (empty, valid per INV-SCENE-005).
    // ========================================================================
    let mut surface_scenes = std::collections::HashMap::new();
    let mut scene_slot = Some(scene);
    for surface_desc in surface_stack.iter() {
        let surface_scene = scene_slot
            .take()
            .unwrap_or_else(crate::core::scene::SceneGraph::new);
        surface_scenes.insert(surface_desc.id, surface_scene);
    }

    // SPEC-HAL-081B + SPEC-HAL-163: Build phase — consume scenes to produce DrawPlan
    let frame_counter = renderer.frame_counter();
    let surface_scale_factors: std::collections::HashMap<_, _> = surface_stack
        .iter()
        .map(|desc| (desc.id, renderer.scale_factor(desc.id)))
        .collect();
    let draw_plan = match draw_plan_builder.build_multi(
        &surface_scenes,
        surface_stack,
        renderer.mesh_cache(),
        |surface_id| {
            surface_scale_factors
                .get(&surface_id)
                .copied()
                .unwrap_or(1.0)
        },
        frame_counter,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            tracing::error!(%error, "DrawPlan Build refused");
            return;
        }
    };
    // SPEC-HAL-081B: Compile phase — GPU buffer uploads (multi-target)
    match renderer.compile_multi(&draw_plan) {
        Ok(crate::renderer::RendererCompileOutcome::Ready(compiled)) => {
            // SPEC-HAL-081B: Execute phase — command encoding per target
            let outputs = renderer.render_multi(compiled);
            // Platform-layer submission (INV-RG-013) for each target
            for output in outputs {
                renderer.submit_and_present(output);
            }
            renderer.advance_frame();
        }
        Ok(crate::renderer::RendererCompileOutcome::NoTargets) => {
            // Frame skipped (no registered targets)
        }
        Ok(crate::renderer::RendererCompileOutcome::Pending(identity)) => {
            tracing::warn!(
                ?identity,
                "Compile pending; no frame encoded by this native single-frame driver"
            );
        }
        Err(e) => {
            tracing::error!("Multi-surface compile error: {:?}", e);
        }
    }
}

// ============================================================================
// SPEC-HAL-161: Signal Delivery Phase
// ============================================================================

/// SPEC-HAL-161: Deliver pending cross-window signals.
/// SPEC-HAL-162: Validates target lifecycle before delivery.
///
/// # Phase Placement (BINDING)
///
/// This function MUST be called in the **Signal Delivery Phase**:
/// - AFTER: `input_fusion.process_frame()` + `event_router.route_input_event()` loop
/// - AFTER: teardown phase (SPEC-HAL-162)
/// - BEFORE: `run_single_frame()` (layout + scene + paint)
///
/// # Invariants Enforced
///
/// - INV-XWIN-003: Phase-correct delivery (only in this phase)
/// - INV-XWIN-004: No re-entrancy (bus.drain_pending() asserts this)
/// - INV-XWIN-005: Deterministic FIFO ordering
/// - INV-LIFE-002: No post-mortem delivery (target must be Alive)
/// - INV-LIFE-003: Validity checked at delivery time
/// - INV-LIFE-004: Dropped signals cause no side effects
///
/// # Example Wiring (in RedrawRequested handler)
///
/// ```ignore
/// // 1. Frame timing
/// let frame_time = ticker.begin_frame();
///
/// // 2. Input processing
/// input_fusion.process_frame(&frame_time, &mut ticker, &mut fused_events);
/// for fused in fused_events.drain(..) {
///     event_router.route_input_event(...);
/// }
///
/// // 3. Teardown phase (SPEC-HAL-162)
/// // lifecycle.process_pending_teardowns();
///
/// // 4. Signal Delivery Phase (SPEC-HAL-161 + SPEC-HAL-162)
/// deliver_signals(&mut signal_bus, &lifecycle, |envelope| {
///     // Handle signal - may enqueue deferred mutations
///     // MUST NOT call routing/layout/scene/paint
/// });
///
/// // 5. Layout + Paint + Render
/// run_single_frame(...);
/// ```
///
/// # Handler Contract
///
/// The handler closure MUST NOT:
/// - Call `event_router.route_input_event()` (INV-XWIN-004)
/// - Call `layout_tree()` or `run_single_frame()` (INV-XWIN-004)
/// - Build scene graphs (INV-XWIN-004)
/// - Invoke compositor operations (INV-XWIN-004)
///
/// The handler MAY:
/// - Update application state
/// - Enqueue deferred mutations
/// - Send response signals via `signal_bus.enqueue()`
pub fn deliver_signals<F>(
    signal_bus: &mut crate::core::signal_bus::SignalBus,
    lifecycle: &crate::core::lifecycle::LifecycleRegistry,
    mut handler: F,
) where
    F: FnMut(crate::core::signal_bus::SignalEnvelope),
{
    // INV-XWIN-003: Phase correctness — must not be in layout/scene/paint.
    debug_assert!(
        !in_frame_critical_section(),
        "INV-XWIN-003 VIOLATION: deliver_signals() called during layout/scene/paint. \
         Signal delivery must occur BEFORE run_single_frame()."
    );

    // INV-XWIN-004: drain_pending() asserts no re-entrancy in debug builds.
    let signals = signal_bus.drain_pending();

    // INV-XWIN-005: Deliver in FIFO order (Vec maintains enqueue order).
    // INV-LIFE-002/003: Validate target at delivery time, drop if not alive.
    for envelope in signals {
        // INV-LIFE-003: Check validity at delivery time, not enqueue time.
        if !lifecycle.is_target_alive(&envelope.target) {
            // INV-LIFE-002: No post-mortem delivery.
            // INV-LIFE-004: Silent drop in release, log in debug.
            #[cfg(debug_assertions)]
            tracing::debug!(
                "SPEC-HAL-162: Signal dropped — target {:?} not alive (source: {:?})",
                envelope.target,
                envelope.source
            );
            continue;
        }
        handler(envelope);
    }
}

/// Perform layout on the entire widget tree
/// SPEC-HAL-024: Layout computes ONLY relative coordinates
/// SPEC-HAL-024 Stage 2: Also computes absolute_xy for each node after layout
/// SPEC-HAL-024 Stage 3: Also computes absolute_paint_rect for each node after layout
/// SPEC-HAL-026: Honors dirty flags for incremental layout
/// SPEC-HAL-088 Stage 2: Also computes viewport for each node after absolute rects
pub fn layout_tree(
    tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    signals: Option<&mut SignalStore>,
) -> Size {
    let can_reuse_root_geometry = tree.node(root).is_some_and(|node| {
        !node.dirty_layout
            && node.last_constraints == Some(constraints)
            && tree.get_cached_size(root).is_some()
    });
    if can_reuse_root_geometry {
        return tree.get_cached_size(root).unwrap_or(Size::zero());
    }

    // SINGLE-LAYOUT-PER-FRAME INVARIANT: Count and assert
    let call_count = layout_tree_call_enter();
    resize_profiler::inc_layout_pass(); // Track in profiler

    // Assert single layout per frame (violations indicate scheduler bug)
    if call_count > 1 {
        eprintln!(
            "[LAYOUT_INVARIANT_VIOLATION] layout_tree called {} times this frame! \
             This should be exactly 1. Check for secondary layout triggers.",
            call_count
        );
        #[cfg(debug_assertions)]
        debug_assert!(
            call_count == 1,
            "SINGLE-LAYOUT-PER-FRAME INVARIANT VIOLATED: layout_tree called {} times",
            call_count
        );
    }

    // DEBUG: Increment layout epoch for paint-without-layout detection
    #[cfg(any(debug_assertions, test))]
    increment_layout_epoch();

    // SPEC-HAL-026: the Phase-3 "force root dirty every frame" safety net was
    // removed here. It masked off-pool tree-children of virtualized widgets
    // retaining dirty_layout=true, which made §7 Rule 4's walk-up in
    // mark_layout_dirty early-exit before reaching root. The virtualized
    // widgets no longer leave persistent off-pool dirty children — ListView
    // lays out ScrollArea/Footer in the allocation pass; TreeViewBody and
    // PropertyEditorBody (v2) remove stale child widgets via remove_node and
    // lay out the eligible pool (INV-CHILD-BOUNDED) — so mark_layout_dirty
    // reaches root for every real change and the net is redundant. If any
    // surface shows stale layout, revert this and fix the widget that leaves
    // the persistent leftover (the real §8 target); do not reinstate the net
    // as a permanent fix.

    let size = layout_node(tree, root, constraints, theme, text_engine, signals);

    // SPEC-HAL-024 Stage 2: absolute_xy (layout-only, no scroll)
    compute_absolute_positions(tree, root, Point::zero());

    // SPEC-HAL-024 Stage 3B: absolute_paint_rect (visual position + scroll)
    compute_absolute_paint_rects(tree, root, Point::zero());

    // SPEC-HAL-088 Stage 2: viewport propagation (ancestor clipping)
    tree.compute_viewports(root);

    size
}

/// SPEC-HAL-024: Layout computes ONLY relative coordinates, no absolutes
/// SPEC-HAL-026: Skips layout for clean widgets with unchanged constraints
fn layout_node(
    tree: &mut WidgetTree,
    node_id: NodeId,
    constraints: Constraints,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    signals: Option<&mut SignalStore>,
) -> Size {
    // SPEC-HAL-026: Check if we can skip layout (clean widget with same constraints)
    // SPEC-HAL-141: Access geometry through WidgetTree getter (private field).
    {
        let skip_result = tree.node(node_id).and_then(|node| {
            if !node.dirty_layout {
                if let Some(last) = node.last_constraints {
                    if constraints == last {
                        return tree.get_cached_size(node_id);
                    }
                }
            }
            None
        });
        if let Some(cached_size) = skip_result {
            return cached_size;
        }
    }

    // SPEC-HAL-026: Remember size before layout for change detection
    // SPEC-HAL-141: Use getter for private geometry field
    let size_before = tree.get_cached_size(node_id).unwrap_or(Size::zero());
    let parent_id = tree.node(node_id).and_then(|n| n.parent);

    // Take/restore pattern: temporarily extract widget from tree
    let (mut widget, children, rect) = match tree.take_widget(node_id) {
        Some(data) => data,
        None => {
            // SPEC-HAL-158 INV-ZERO-2: Illegal zero-size return (ERROR classification)
            // Missing node in layout_node is a layout error, not an intentional zero.
            debug_assert!(
                false,
                "INV-ZERO-2 VIOLATION: layout_node() failed to take widget for NodeId {:?}",
                node_id
            );
            tracing::error!(
                "INV-ZERO-2: layout_node() returning zero for missing NodeId {:?}",
                node_id
            );
            return Size::zero();
        }
    };

    // Create context with mutable tree reference
    let mut cx = match signals {
        Some(s) => LayoutContext::new_with_signals(tree, text_engine, theme, s),
        None => LayoutContext::new(tree, text_engine, theme),
    };
    // SPEC-HAL-134: Engine-controlled pre-layout hook.
    // INV-PHASE-TRANSITION-1: Phase transitions happen in pre_layout(), not in layout().
    widget.pre_layout();

    // SPEC-HAL-154: Sealing enforcement is handled by LayoutContext dispatchers
    // (layout_child, layout_child_full, relayout_child_with_size, relayout_child_with_constraints)
    // which are the actual paths through which widgets get laid out.
    // Root widget sealing happens when it's laid out as a child of the window.

    // SPEC-HAL-086 Stage 2: Call layout with children slice, extract size from LayoutResult
    let result = widget.layout(&mut cx, constraints, &children);
    let size = result.size;

    // Update rect with new size (relative to parent)
    let new_rect = RectF::new(rect.x, rect.y, size.width, size.height);

    // Restore widget back into tree
    tree.restore_widget(node_id, widget, new_rect);

    // SPEC-HAL-026: Update cached values and clear dirty flag
    // SPEC-HAL-141: Use guarded setter for geometry field
    tree.set_cached_size(node_id, size);
    // Store content_size for scroll extent aggregation by parent widgets
    tree.set_content_size(node_id, result.content_size);
    // SPEC-HAL-086 Stage 5: Store scroll_offset from layout result for position computation
    if let Some(node) = tree.node_mut(node_id) {
        node.last_constraints = Some(constraints);
        node.dirty_layout = false;
        // DEBUG: Stamp layout epoch to prove this node was laid out this frame
        #[cfg(any(debug_assertions, test))]
        {
            node.layout_epoch = current_layout_epoch();
        }
    }
    tree.set_layout_scroll_offset(node_id, result.scroll_offset);
    tree.mark_paint_dirty(node_id);

    // SPEC-HAL-026: Size change propagation - mark parent dirty if size changed
    if size != size_before {
        if let Some(pid) = parent_id {
            tree.mark_layout_dirty(pid);
        }
    }

    size
}

/// SPEC-HAL-024 Stage 2: Compute absolute_xy for all nodes in the tree.
///
/// This is called after layout completes, traversing the tree to compute
/// each node's absolute position from parent.absolute_xy + node.rect.(x, y).
///
/// SPEC-HAL-086 Stage 7: Scroll offset is NO LONGER applied here.
/// Scroll transforms are now part of the scene graph via ScrollNode.
/// This function computes canonical absolute positions (unscrolled).
///
/// Note: layout_scroll_offset is still stored for use in hit-testing,
/// but is not applied during position computation.
pub fn compute_absolute_positions(tree: &mut WidgetTree, node_id: NodeId, parent_abs: Point) {
    // Get the node's relative rect and compute its absolute position
    let (abs_xy, children) = {
        let rect = match tree.get_rect(node_id) {
            Some(r) => r,
            None => return,
        };
        let abs_xy = Point::new(parent_abs.x + rect.x, parent_abs.y + rect.y);
        (abs_xy, tree.children(node_id))
    };

    // Store the absolute position (SPEC-HAL-141: uses guarded setter)
    tree.set_absolute_xy(node_id, abs_xy);

    // SPEC-HAL-086 Stage 7: Children use parent's absolute position directly.
    // Scroll offset is NOT applied here - it's in the scene graph.

    // Recurse into children with the computed parent_abs
    for child_id in children {
        compute_absolute_positions(tree, child_id, abs_xy);
    }
}

/// SPEC-HAL-024 Stage 3B: Compute `absolute_paint_rect` for all nodes.
///
/// This function mirrors paint traversal semantics:
/// - Uses layout-relative `rect` plus a propagated `parent_abs` position.
/// - `absolute_paint_rect` is the **logical** rect used by paint.
///
/// SPEC-HAL-086 Stage 7: Scroll offsets are NO LONGER applied here.
/// Scroll transforms are now part of the scene graph via ScrollNode.
/// The batcher applies scroll offsets during primitive insertion.
///
/// Note: Does NOT use `absolute_xy`. This function is independent of `compute_absolute_positions`.
pub fn compute_absolute_paint_rects(tree: &mut WidgetTree, node_id: NodeId, parent_abs: Point) {
    compute_absolute_paint_rects_recursive(tree, node_id, parent_abs);
    tree.record_layout_transition_traversal(node_id);
}

fn compute_absolute_paint_rects_recursive(tree: &mut WidgetTree, node_id: NodeId, parent_abs: Point) {
    // Get this node's rect, widget_id, and children in a single immutable borrow.
    let (rect, widget_id, children) = {
        let node = match tree.node(node_id) {
            Some(n) => n,
            None => return,
        };
        // SPEC-HAL-141: Use getter for private geometry field
        let rect = tree.get_rect(node_id).unwrap_or(RectF::zero());
        let widget_id = node.id;
        let children = tree.children(node_id);

        (rect, widget_id, children)
    };

    // Compute this node's absolute paint rect from parent_abs + relative rect.
    let abs_rect = RectF::new(
        parent_abs.x + rect.x,
        parent_abs.y + rect.y,
        rect.width,
        rect.height,
    );

    // Write absolute_paint_rect (SPEC-HAL-141: uses guarded setter).
    // SPEC-HAL-088 Stage 2: viewport is computed separately by compute_viewports().
    tree.set_absolute_paint_rect(node_id, abs_rect);

    // SPEC-HAL-078 Stage 0: Record authoritative geometry for transition tracking.
    // LT-INV-5: Keyed strictly by TreeWidgetId.
    tree.record_layout_transition(widget_id, abs_rect);

    // SPEC-HAL-086 Stage 7: Children use parent's absolute position directly.
    // Scroll offset is NOT applied here - it's applied during scene building/batching.
    let child_parent_abs = Point::new(abs_rect.x, abs_rect.y);

    // Recurse into children with the computed parent_abs.
    for child_id in children {
        compute_absolute_paint_rects_recursive(tree, child_id, child_parent_abs);
    }
}

/// Paint the entire widget tree into a scene graph
///
/// SPEC-HAL-035: Caller MUST invoke `tree.recompute_dirty_flags()` before calling
/// this function to enable the widget diff optimization. If not called, all nodes
/// will be treated as dirty (safe but suboptimal).
///
/// SPEC-HAL-024 Stage 7: Uses `absolute_paint_rect` as canonical geometry.
/// Caller MUST have called `compute_absolute_paint_rects()` after layout.
///
/// SPEC-HAL-101 Stage-4C: Added frame_time_ms for caret blinking.
/// SPEC-HAL-078 Stage 3: Added active_layout_transitions for paint-time interpolation.
/// SPEC-HAL-145: Now receives pre-computed interpolated rects (transition phase already completed).
pub fn paint_tree(
    tree: &WidgetTree,
    root: NodeId,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    timeline: &crate::core::animation::Timeline,
    interpolated_rects: &InterpolatedVisualRects,
    scale_factor: f32,
    active_drag: Option<&crate::core::drag_drop::DragSession>,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: Option<&TooltipRenderState>,
    focused_widget: Option<crate::utils::id::TreeWidgetId>,
    keyboard_mode: crate::core::event::KeyboardAuthorityMode,
) -> (SceneGraph, Vec<crate::core::widget_tree::OverlayHitRegion>) {
    paint_tree_fresh(
        tree,
        root,
        theme,
        text_engine,
        icon_registry,
        frame_time_ms,
        timeline,
        interpolated_rects,
        scale_factor,
        active_drag,
        tooltip_triggers,
        tooltip_render,
        focused_widget,
        keyboard_mode,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_tree_fresh(
    tree: &WidgetTree,
    root: NodeId,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    timeline: &crate::core::animation::Timeline,
    interpolated_rects: &InterpolatedVisualRects,
    scale_factor: f32,
    active_drag: Option<&crate::core::drag_drop::DragSession>,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: Option<&TooltipRenderState>,
    focused_widget: Option<crate::utils::id::TreeWidgetId>,
    keyboard_mode: crate::core::event::KeyboardAuthorityMode,
    debug_outlines: bool,
) -> (SceneGraph, Vec<crate::core::widget_tree::OverlayHitRegion>) {
    let mut scene_builder = SceneBuilder::with_scale_factor(scale_factor);

    // Set clear color from theme (fixes black background issue)
    scene_builder.set_clear_color(theme.colors.surfaces.base0.into());

    // Collect overlay hit regions during paint (scrollbars, etc.)
    let mut overlay_hit_regions = Vec::new();

    let has_main_affordances = tree.has_control_subtree_main_affordances();
    if has_main_affordances {
        tree.assert_control_subtree_orders_current();
        let mut cache = RetainedSceneCache::default();
        let mut environment = RetainedPaintEnvironment {
            tree,
            theme,
            text_engine,
            icon_registry,
            frame_time_ms,
            timeline,
            interpolated_rects,
            scale_factor,
            focused_widget,
            keyboard_mode,
            appearance: None,
            debug_outlines,
            cache: &mut cache,
            cleaned_nodes: Vec::new(),
            recomposed_fragments: 0,
        };
        let fragment = environment.build_fragment(root, 0).unwrap_or_default();
        assert!(
            fragment.main_affordances.is_empty(),
            "INV-359-MAIN-AFFORDANCE-ONCE: unconsumed export at scene root"
        );
        scene_builder.push_retained_fragment(fragment.composed);
        overlay_hit_regions.extend(
            fragment.overlay_hit_regions.into_iter().map(|entry| entry.region),
        );
        tooltip_triggers.extend(fragment.tooltip_triggers);
        #[cfg(debug_assertions)]
        scene_builder.extend_retained_key_widget_map(&fragment.key_widget_map);
    }

    {
        let mut cx = PaintContext::new(
            tree,
            &mut scene_builder,
            theme,
            text_engine,
            icon_registry,
            frame_time_ms,
            timeline,
            interpolated_rects,
            &mut overlay_hit_regions,
            tooltip_triggers,
            scale_factor,
            focused_widget,
            keyboard_mode,
        );

        // SPEC-HAL-024 Stage 7: Paint uses pre-computed absolute_paint_rect
        // SPEC-HAL-035: Start with ancestor_dirty=false; root determines its own dirtiness
        // A fresh snapshot emits the current immutable outline presentation.
        if !has_main_affordances {
            paint_node(
                root,
                &mut cx,
                tree.any_widget_needs_animation() || debug_outlines,
                0,
                debug_outlines,
            );
        }

        // SPEC-HAL-241: Paint drag proxy ghost after all widgets
        if let Some(drag) = active_drag {
            if drag.ghost_alpha > 0.001 {
                if let Some(node_id) = cx.tree.find_node_by_widget_id(drag.source_widget) {
                    let ghost_rect = RectF::new(
                        drag.display_pos.x + 8.0,
                        drag.display_pos.y + 8.0,
                        drag.source_size.width,
                        drag.source_size.height,
                    );
                    if let Some(node) = cx.tree.node(node_id) {
                        node.widget
                            .paint_drag_proxy(&mut cx, ghost_rect, drag.ghost_alpha);
                    }
                }
            }
        }

        // SPEC-HAL-226A: Paint tooltip overlay from manager state.
        if let Some(tooltip) = tooltip_render {
            crate::widgets::tooltip_surface::TooltipSurface::paint_standalone(
                &mut cx,
                tooltip.anchor,
                &tooltip.content,
            );
        }
    }

    (scene_builder.build(), overlay_hit_regions)
}

#[derive(Clone, Default)]
struct RetainedWidgetScene {
    local: Arc<[SceneNode]>,
    overlay: Arc<[SceneNode]>,
    overlay_eligibility: MainAffordanceEligibility,
    own_overlay_hit_regions: Vec<OverlayHitEntry>,
    own_tooltip_triggers: Vec<crate::model::tooltip::TooltipTrigger>,
    composed: Arc<[SceneNode]>,
    main_affordances: Vec<MainAffordanceExport>,
    overlay_hit_regions: Vec<OverlayHitEntry>,
    tooltip_triggers: Vec<crate::model::tooltip::TooltipTrigger>,
    #[cfg(debug_assertions)]
    own_key_widget_map: HashMap<SceneNodeKey, u64>,
    #[cfg(debug_assertions)]
    key_widget_map: HashMap<SceneNodeKey, u64>,
}

struct PaintedLocalFragments {
    local: Arc<[SceneNode]>,
    overlay: Arc<[SceneNode]>,
    overlay_eligibility: MainAffordanceEligibility,
    overlay_hit_regions: Vec<OverlayHitEntry>,
    tooltip_triggers: Vec<crate::model::tooltip::TooltipTrigger>,
    #[cfg(debug_assertions)]
    key_widget_map: HashMap<SceneNodeKey, u64>,
}

#[derive(Clone)]
struct MainAffordanceExport {
    member: crate::core::paint::ControlSubtreeMember,
    nodes: Arc<[SceneNode]>,
}

#[derive(Clone, Copy, Default)]
struct MainAffordanceEligibility {
    violation: Option<&'static str>,
}

impl MainAffordanceEligibility {
    fn capture(nodes: &[SceneNode], has_hits: bool, has_tooltips: bool) -> Self {
        Self {
            violation: if has_hits {
                Some("overlay-origin hit regions")
            } else if has_tooltips {
                Some("overlay-origin tooltip triggers")
            } else {
                Self::scene_violation(nodes)
            },
        }
    }

    fn scene_violation(nodes: &[SceneNode]) -> Option<&'static str> {
        use crate::core::paint::LAYER_OVERLAY;
        for node in nodes {
            let z = match node {
                SceneNode::RetainedFragment(n) => {
                    if n.control_subtree_member.is_some() {
                        return Some("source composition scope");
                    }
                    if let Some(reason) = Self::scene_violation(&n.children) {
                        return Some(reason);
                    }
                    continue;
                }
                SceneNode::Clip(n) => {
                    if let Some(reason) = Self::scene_violation(&n.children) {
                        return Some(reason);
                    }
                    continue;
                }
                SceneNode::Scroll(n) => {
                    if let Some(reason) = Self::scene_violation(&n.children) {
                        return Some(reason);
                    }
                    continue;
                }
                SceneNode::Layer(_) | SceneNode::ScrollbarV(_) | SceneNode::ScrollbarH(_) => {
                    return Some("surface or input output");
                }
                SceneNode::Rect(n) => n.z_index,
                SceneNode::BorderRect(n) => n.z_index,
                SceneNode::RoundedRect(n) => n.z_index,
                SceneNode::Text(n) => n.z_index,
                SceneNode::Image(n) => n.z_index,
                SceneNode::Circle(n) => n.z_index,
                SceneNode::Ellipse(n) => n.z_index,
                SceneNode::Line(n) => n.z_index,
                SceneNode::Polyline(n) => n.z_index,
                SceneNode::LuminousContour(n) => n.z_index(),
                SceneNode::LocalAmbientGlow(n) => n.z_index(),
                SceneNode::AnalyticSurface(n) => n.z_index(),
                SceneNode::FrostedGlass(n) => n.z_index(),
                SceneNode::Path(n) => n.z_index,
                SceneNode::Shadow(n) => n.z_index,
                SceneNode::GradientRect(n) => n.z_index,
                SceneNode::ColorPickerSv(n) => n.z_index,
                SceneNode::RadialGauge(n) => n.z_index,
            };
            if z >= LAYER_OVERLAY {
                return Some("global overlay pass");
            }
        }
        None
    }

    fn assert_exportable(self) {
        assert!(
            self.violation.is_none(),
            "INV-359-MAIN-AFFORDANCE-OUTPUT: complete current overlay must be decorative main output ({:?})",
            self.violation
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RetainedPaintContextKey {
    scale_factor_bits: u32,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: KeyboardAuthorityMode,
    debug_outlines: bool,
}

struct SdfOutputMaterialBasis {
    input: crate::renderer::wgpu_renderer::SdfMaterialFrameInput,
    pending: bool,
}

#[derive(Default)]
struct RetainedSceneCache {
    entries: HashMap<TreeWidgetId, RetainedWidgetScene>,
    context_key: Option<RetainedPaintContextKey>,
    structure_generation: u64,
    transient_state: Option<RetainedTransientState>,
    appearance: Option<FrameworkAppearanceSnapshot>,
    presentation: Option<Box<PresentationRuntime>>,
    interpolated_rects: InterpolatedVisualRects,
    image_dependencies: Option<Box<RetainedImageDependencies>>,
    image_wake: Option<winit::event_loop::EventLoopProxy<()>>,
    qualified_sdf_requests: Option<Box<Vec<TargetQualifiedButtonSdfRequest>>>,
    sdf_material_staging: Option<Box<crate::renderer::wgpu_renderer::SdfMaterialStaging>>,
    selected_sdf_materials: Option<Box<HashMap<TreeWidgetId, Option<SdfMaterialPublication>>>>,
    // P13 must populate this from genuine first Image references, before emission.
    // Reused retained scenes keep these references until their old E is removed.
    sdf_scene_custody: SdfSceneCustody,
    sdf_paint_inputs: Option<Box<HashMap<TreeWidgetId, crate::core::widget::ButtonSdfPaintInput>>>,
    sdf_scene_images: Option<Box<HashSet<ImageBindingIdentity>>>,
    sdf_output_bases: Option<Box<HashMap<TreeWidgetId, SdfOutputMaterialBasis>>>,
}

#[derive(Debug, Clone, PartialEq)]
struct TargetQualifiedButtonSdfRequest {
    request: crate::core::widget_tree::CapturedButtonSdfRequest,
    key: crate::renderer::sdf_material_status::SdfMaterialKey,
    // Keep current frame provenance even when the resource key remains equal.
    metrics: crate::renderer::custom_render_prepare::CustomRenderTargetMetrics,
}

/// Read the same enclosing scopes as retained composition, starting at the
/// subtree being painted. The Button's own child clip and scroll do not apply
/// to its local face. Both returned rects are in composed logical coordinates.
fn button_sdf_composition_placement(
    tree: &WidgetTree,
    root: NodeId,
    owner: NodeId,
    face: RectF,
) -> Option<(RectF, RectF)> {
    let mut ancestors = Vec::new();
    let mut id = owner;
    loop {
        let node = tree.node(id)?;
        if !tree.composite_presentation_visible(id) {
            return None;
        }
        if id == root {
            break;
        }
        id = node.parent?;
        ancestors.push(id);
    }

    let mut clip: Option<RectF> = None;
    let mut scroll = (0.0, 0.0);
    for id in ancestors.into_iter().rev() {
        let node = tree.node(id)?;
        let allocation = tree.get_absolute_paint_rect(id)?;
        let widget_clip = node
            .widget
            .clip_bounds()
            .map(|local| local.translate(allocation.x, allocation.y));
        for source in [tree.child_clip_source(id), widget_clip].into_iter().flatten() {
            let projected = source.translate(-scroll.0, -scroll.1);
            clip = Some(match clip {
                Some(enclosing) => enclosing.intersection(&projected)?,
                None => projected,
            });
        }
        // An owner's scrollport is fixed in the enclosing scope. Its offset
        // shifts only subsequent descendants, in the scene's outer-to-inner order.
        if let Some((x, y)) = node.widget.scroll_offset() {
            scroll = (scroll.0 + x, scroll.1 + y);
        }
    }
    let composed = face.translate(-scroll.0, -scroll.1);
    let visible = match clip {
        Some(clip) => composed.intersection(&clip)?,
        None => composed,
    };
    Some((composed, visible))
}

impl RetainedSceneCache {
    fn active_sdf_size_replacement_keys(
        &mut self,
        current: &[crate::renderer::wgpu_renderer::SdfMaterialFrameInput],
    ) -> Vec<crate::renderer::sdf_material_status::SdfMaterialKey> {
        use crate::renderer::custom_render_prepare::CustomRenderActivity;
        use crate::renderer::sdf_material_status::SdfMaterialStatus;

        let Some(bases) = self.sdf_output_bases.as_mut() else {
            return Vec::new();
        };
        let by_owner: HashMap<_, _> = current
            .iter()
            .map(|input| (input.key.owner(), input))
            .collect();
        let mut eligible = Vec::new();
        bases.retain(|owner, basis| {
            let Some(input) = by_owner.get(owner).copied() else {
                return false;
            };
            let old = &basis.input;
            let metrics = input.key.metrics();
            let same_request = input.source.has_pressed_cell()
                && input.source == old.source
                && input.key.tree_lifetime() == old.key.tree_lifetime()
                && input.key.metrics().surface_id == old.key.metrics().surface_id
                && input.key.revision() == old.key.revision()
                && metrics.scale_factor == old.key.metrics().scale_factor
                && matches!(metrics.activity, CustomRenderActivity::Active { .. });
            let failed = self
                .selected_sdf_materials
                .as_ref()
                .and_then(|selected| selected.get(owner))
                .and_then(Option::as_ref)
                .is_some_and(|publication| {
                    matches!(publication.status(), SdfMaterialStatus::Unavailable { .. })
                });
            if !same_request || failed {
                return false;
            }
            if basis.pending
                || metrics.final_logical_rect.size() != old.key.metrics().final_logical_rect.size()
            {
                basis.pending = true;
                eligible.push(input.key.clone());
            }
            true
        });
        if bases.is_empty() {
            self.sdf_output_bases = None;
        }
        eligible
    }

    fn cancel_obsolete_sdf_composition(
        &mut self,
        tree: &mut WidgetTree,
        eligible: &[crate::renderer::sdf_material_status::SdfMaterialKey],
    ) {
        for key in eligible {
            let owner = key.owner();
            // The current attempt must rebuild this local output and its composed
            // ancestors before it may submit; previous plan/output clones keep
            // their independent CPU/sampling protection.
            let mut node_id = tree.find_node_by_widget_id(owner);
            while let Some(id) = node_id {
                let Some(node) = tree.node(id) else { break };
                let ancestor = node.id;
                node_id = node.parent;
                self.entries.remove(&ancestor);
                self.replace_image_dependencies(ancestor, Vec::new());
            }
            tree.mark_paint_dirty_by_widget_id(owner);
            for (old, identity) in self.sdf_scene_custody.references() {
                if old.owner() == owner
                    && old.tree_lifetime() == key.tree_lifetime()
                    && old.metrics().surface_id == key.metrics().surface_id
                {
                    if let Some(images) = self.sdf_scene_images.as_mut() {
                        images.remove(&identity);
                    }
                    self.sdf_scene_custody.release_after_removal(&old, identity);
                }
            }
        }
    }

    fn note_sdf_target_output(&mut self, surface: SurfaceId) {
        let mut output = Vec::new();
        for qualified in self
            .qualified_sdf_requests
            .iter()
            .flat_map(|requests| requests.iter())
        {
            if qualified.key.metrics().surface_id != surface {
                continue;
            }
            let request = qualified.request.request();
            let Some(input) = self
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&qualified.key.owner()))
                .filter(|input| {
                    input.key == qualified.key && input.requested_revision == request.revision
                })
            else {
                continue;
            };
            let Some(_) = input.image.as_ref().filter(|image| {
                image.binding.availability().is_ok()
                    && self
                        .sdf_scene_images
                        .as_ref()
                        .is_some_and(|images| images.contains(&image.binding.identity()))
            }) else {
                continue;
            };
            output.push(crate::renderer::wgpu_renderer::SdfMaterialFrameInput {
                key: qualified.key.clone(),
                request_revision: request.revision,
                source: request.source,
                pressed: request.pressed,
                hovered: request.hovered,
            });
        }
        if output.is_empty() {
            return;
        }
        let bases = self.sdf_output_bases.get_or_insert_with(Default::default);
        for input in output {
            bases.insert(
                input.key.owner(),
                SdfOutputMaterialBasis {
                    input,
                    pending: false,
                },
            );
        }
    }

    fn prepare_sdf_paint_inputs(&mut self, tree: &mut WidgetTree) {
        use crate::core::widget::{ButtonSdfPaintImage, ButtonSdfPaintInput};
        use crate::renderer::sdf_material_status::SdfMaterialStatus;

        if self.selected_sdf_materials.is_none() && self.sdf_paint_inputs.is_none() {
            return;
        }
        let previous = self.sdf_paint_inputs.take().unwrap_or_default();
        let mut next = HashMap::new();
        for current in self.current_sdf_frame_inputs(tree) {
            let owner = current.key.owner();
            let publication = self
                .selected_sdf_materials
                .as_ref()
                .and_then(|selected| selected.get(&owner))
                .and_then(Option::as_ref)
                .filter(|publication| {
                    publication.key() == &current.key
                        || self.sdf_material_staging.as_ref().is_some_and(|staging| {
                            staging.can_continue_selection(&current, publication.key())
                        })
                });
            let was_active = previous
                .get(&owner)
                .is_some_and(|input| input.key.tree_lifetime() == current.key.tree_lifetime());
            // Initial pending admission preserves the existing valid selection.
            // Once selected, missing renewed evidence omits SDF rather than
            // falling back to another material or reusing an obsolete image.
            if publication.is_none() && !was_active {
                continue;
            }
            let mut input = ButtonSdfPaintInput {
                requested_revision: current.request_revision,
                key: publication
                    .map(|publication| publication.key().clone())
                    .unwrap_or(current.key),
                image: None,
            };
            if let Some(SdfMaterialStatus::Ready { binding }) = publication.map(|p| p.status()) {
                let identity = binding.identity();
                if let Some(staging) = &self.sdf_material_staging {
                    if !self.sdf_scene_custody.has_reference(&input.key, identity) {
                        match staging.retain_image_reference(&input.key, identity) {
                            Ok(reference) => self.sdf_scene_custody.retain_reference(reference),
                            Err(error) => tracing::error!(
                                ?error,
                                ?owner,
                                "SDF Image acquisition rejected before Paint"
                            ),
                        }
                    }
                    if self.sdf_scene_custody.has_reference(&input.key, identity) {
                        if let Some((coverage, uv)) = staging.paint_mapping(
                            &input.key,
                            identity,
                            current.pressed,
                            current.hovered,
                        ) {
                            input.image = Some(ButtonSdfPaintImage {
                                binding: binding.clone(),
                                coverage,
                                uv,
                            });
                        }
                    }
                }
            }
            if previous.get(&owner) != Some(&input) {
                tree.mark_paint_dirty_by_widget_id(owner);
            }
            next.insert(owner, input);
        }
        for (owner, input) in previous.iter() {
            if !next.contains_key(owner) {
                // Discard only this owner's old local E. Its old composed root
                // remains protected until the normal retained Paint replaces it.
                self.entries.remove(owner);
                if tree.is_live_mount(input.key.tree_lifetime(), *owner) {
                    tree.mark_paint_dirty_by_widget_id(*owner);
                }
            }
        }
        if !next.is_empty() {
            self.sdf_paint_inputs = Some(Box::new(next));
        }
    }

    fn finish_sdf_scene_paint(&mut self, scene: Option<&SceneGraph>) {
        if self.sdf_scene_custody.references().is_empty() {
            self.sdf_scene_images = None;
            return;
        }
        if let Some(scene) = scene {
            fn collect(nodes: &[SceneNode], identities: &mut HashSet<ImageBindingIdentity>) {
                for node in nodes {
                    match node {
                        SceneNode::Image(image) => {
                            if let crate::core::scene::ImageTextureSource::Admitted(identity) =
                                image.texture_source
                            {
                                identities.insert(identity);
                            }
                        }
                        SceneNode::RetainedFragment(node) => collect(&node.children, identities),
                        SceneNode::Clip(node) => collect(&node.children, identities),
                        SceneNode::Layer(node) => collect(&node.children, identities),
                        SceneNode::Scroll(node) => collect(&node.children, identities),
                        _ => {}
                    }
                }
            }
            let mut identities = HashSet::new();
            collect(&scene.root, &mut identities);
            self.sdf_scene_images = Some(Box::new(identities));
        }
        for (key, identity) in self.sdf_scene_custody.references() {
            let owner = key.owner();
            let retained = self.entries.contains_key(&owner)
                && self
                    .image_dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.by_owner.get(&owner))
                    .is_some_and(|bindings| {
                        bindings
                            .iter()
                            .any(|binding| binding.identity() == identity)
                    });
            let in_scene = self
                .sdf_scene_images
                .as_ref()
                .is_some_and(|identities| identities.contains(&identity));
            let replaced_eligibility = self
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&owner))
                .is_some_and(|input| {
                    input.key != key
                        && input.image.as_ref().is_some_and(|image| {
                            image.binding.identity() == identity
                                && self.sdf_scene_custody.has_reference(&input.key, identity)
                        })
                });
            if (!retained && !in_scene) || replaced_eligibility {
                // Both replacement and composition have finished before this
                // release. Prior frame/plan/output clones retain their own use.
                self.sdf_scene_custody.release_after_removal(&key, identity);
                if !self.entries.contains_key(&owner) {
                    self.replace_image_dependencies(owner, Vec::new());
                }
            }
        }
    }

    fn commit_sdf_materials_before_paint(
        &mut self,
        renderer: &mut WgpuRenderer,
        tree: &mut WidgetTree,
    ) {
        if self.sdf_material_staging.is_none() && self.selected_sdf_materials.is_none() {
            return;
        }
        let current = self.current_sdf_frame_inputs(tree);
        let publications = self
            .sdf_material_staging
            .as_ref()
            .map(|staging| staging.publications_before_paint(renderer, &current))
            .unwrap_or_default();
        let mut selected = self.selected_sdf_materials.take().unwrap_or_default();
        selected.retain(|_, publication| {
            let Some(publication) = publication else {
                return false;
            };
            let key = publication.key();
            if current.iter().any(|input| {
                &input.key == key
                    || self
                        .sdf_material_staging
                        .as_ref()
                        .is_some_and(|staging| staging.can_continue_selection(input, key))
            }) {
                return true;
            }
            // Withdrawal/changed target is intentional absence, not Unavailable.
            // A removed or prior-tree owner has no current paint to invalidate.
            if tree.is_live_mount(key.tree_lifetime(), key.owner()) {
                tree.mark_paint_dirty_by_widget_id(key.owner());
            }
            false
        });
        for input in &current {
            let owner = input.key.owner();
            let next = publications
                .iter()
                .find(|(publication, _)| publication.key() == &input.key)
                .cloned()
                .or_else(|| {
                    let old = selected.get(&owner)?.as_ref()?;
                    self.sdf_material_staging.as_ref()?.continuing_publication(
                        renderer,
                        input,
                        old.key(),
                    )
                });
            let Some((publication, identity)) = next else {
                // Missing initial evidence has no invented status. A committed
                // image without renewed resource evidence must cease projection.
                if selected.remove(&owner).is_some() {
                    tree.mark_paint_dirty_by_widget_id(owner);
                }
                continue;
            };
            let committed_key = publication.key().clone();
            let slot = selected.entry(owner).or_insert(None);
            if let Err(error) =
                tree.publish_sdf_material(slot, &committed_key, identity, publication)
            {
                tracing::error!(?error, ?owner, "SDF pre-Paint material commit rejected");
            }
        }
        selected.retain(|_, publication| publication.is_some());
        if !selected.is_empty() {
            self.selected_sdf_materials = Some(selected);
        }
    }

    fn current_sdf_frame_inputs(
        &self,
        tree: &WidgetTree,
    ) -> Vec<crate::renderer::wgpu_renderer::SdfMaterialFrameInput> {
        self.qualified_sdf_requests
            .iter()
            .flat_map(|requests| requests.iter())
            .filter(|qualified| tree.is_current_button_sdf_request(&qualified.request))
            .map(|qualified| {
                let request = qualified.request.request();
                crate::renderer::wgpu_renderer::SdfMaterialFrameInput {
                    key: qualified.key.clone(),
                    request_revision: request.revision,
                    source: request.source,
                    pressed: request.pressed,
                    hovered: request.hovered,
                }
            })
            .collect()
    }

    fn admit_sdf_materials_after_paint(
        &mut self,
        renderer: &mut WgpuRenderer,
        tree: &WidgetTree,
    ) -> bool {
        if self.sdf_material_staging.is_none() {
            return false;
        }
        let current = self.current_sdf_frame_inputs(tree);
        let eligible = self.active_sdf_size_replacement_keys(&current);
        let cold: Vec<_> = current
            .into_iter()
            .filter(|input| !eligible.iter().any(|key| key == &input.key))
            .collect();
        let staging = self
            .sdf_material_staging
            .as_mut()
            .expect("existing Frame A owner");
        let changed = staging.admit_after_paint(renderer, &cold);
        changed || staging.needs_followup()
    }

    /// Only the final-layout, pre-preparation boundary supplies these inputs.
    /// This freezes target metadata; it neither prepares nor admits a material.
    fn qualify_sdf_requests_after_layout(
        &mut self,
        tree: &WidgetTree,
        root: NodeId,
        target: Option<CustomRenderTargetSnapshot>,
    ) {
        let Some(target) = target else {
            self.qualified_sdf_requests = None;
            return;
        };
        let mut qualified = Vec::new();
        for request in tree.snapshot_button_sdf_requests() {
            if !tree.is_current_button_sdf_request(&request) {
                continue;
            }
            let Some(node_id) = tree.find_by_id(request.owner()) else {
                continue;
            };
            if tree.node(node_id).is_none_or(|node| node.dirty_layout) {
                continue;
            }
            let (Some(rect), Some(viewport)) = (
                tree.get_absolute_paint_rect(node_id),
                tree.get_viewport(node_id),
            ) else {
                continue;
            };
            let Some((composed, visible)) =
                button_sdf_composition_placement(tree, root, node_id, rect)
            else {
                continue;
            };
            let Ok(metrics) = target.metrics_for_visible_rect(rect, viewport, Some(visible))
            else {
                continue;
            };
            let Ok(key) = crate::renderer::sdf_material_status::SdfMaterialKey::new(
                request.tree_lifetime().clone(),
                request.owner(),
                metrics.surface_id,
                if request.request().source.uses_basis_revision() {
                    request.request().basis_revision
                } else {
                    request.request().revision
                },
                metrics,
            ) else {
                continue;
            };
            qualified.push(TargetQualifiedButtonSdfRequest {
                request,
                key: key.with_composited_placement(composed, visible),
                metrics,
            });
        }
        if qualified.is_empty() {
            self.qualified_sdf_requests = None;
        } else if self.qualified_sdf_requests.as_deref() != Some(&qualified) {
            self.qualified_sdf_requests = Some(Box::new(qualified));
        }
    }
}

#[cfg(test)]
#[path = "app/sdf_button_gpu_tests.rs"]
mod sdf_button_gpu_tests;

#[cfg(test)]
#[path = "app/input_value_typography_tests.rs"]
mod input_value_typography_tests;

#[cfg(test)]
#[path = "../tests/app/sdf_target_qualification_tests.rs"]
mod sdf_target_qualification_tests;

struct RetainedImageDependencies {
    invalidations: Arc<ImageBindingInvalidations>,
    by_identity: HashMap<ImageBindingIdentity, HashSet<TreeWidgetId>>,
    by_owner: HashMap<TreeWidgetId, Vec<ImageBinding>>,
}

impl RetainedImageDependencies {
    fn new(wake: Option<&winit::event_loop::EventLoopProxy<()>>) -> Self {
        let wake = wake.cloned().map(|proxy| {
            Box::new(move || {
                let _ = proxy.send_event(());
            }) as Box<dyn Fn() + Send + Sync>
        });
        Self {
            invalidations: Arc::new(ImageBindingInvalidations::new(wake)),
            by_identity: HashMap::new(),
            by_owner: HashMap::new(),
        }
    }

    fn replace(&mut self, owner: TreeWidgetId, bindings: Vec<ImageBinding>) {
        if let Some(previous) = self.by_owner.remove(&owner) {
            for binding in previous {
                let identity = binding.identity();
                if let Some(owners) = self.by_identity.get_mut(&identity) {
                    owners.remove(&owner);
                    if owners.is_empty() {
                        self.by_identity.remove(&identity);
                    }
                }
            }
        }
        let mut unique = Vec::new();
        for binding in bindings {
            if unique.contains(&binding) {
                continue;
            }
            self.by_identity
                .entry(binding.identity())
                .or_default()
                .insert(owner);
            binding.observe(&self.invalidations);
            unique.push(binding);
        }
        if !unique.is_empty() {
            self.by_owner.insert(owner, unique);
        }
    }

    fn invalidate(&self, tree: &mut WidgetTree) {
        for identity in self.invalidations.take() {
            if let Some(owners) = self.by_identity.get(&identity) {
                for owner in owners {
                    tree.mark_paint_dirty_by_widget_id(*owner);
                }
            }
        }
    }
}

impl RetainedSceneCache {
    fn replace_image_dependencies(&mut self, owner: TreeWidgetId, bindings: Vec<ImageBinding>) {
        if bindings.is_empty() && self.image_dependencies.is_none() {
            return;
        }
        let dependencies = self.image_dependencies.get_or_insert_with(|| {
            Box::new(RetainedImageDependencies::new(self.image_wake.as_ref()))
        });
        dependencies.replace(owner, bindings);
        if dependencies.by_owner.is_empty() {
            self.image_dependencies = None;
        }
    }
}

pub(crate) fn drain_requested_images_for_reused_scene(renderer: &mut WgpuRenderer) {
    if renderer.has_pending_image_admissions() || renderer.has_pending_image_unregistrations() {
        renderer.drain_pending_image_registrations();
    }
}

struct NativeStartupOutput {
    required: bool,
    attempt_id: crate::core::frame_realization::AppFrameAttemptId,
    primary: SurfaceId,
}

impl NativeStartupOutput {
    fn new(
        attempt_id: crate::core::frame_realization::AppFrameAttemptId,
        primary: SurfaceId,
        primary_visible: bool,
        startup_ready: bool,
        primary_output_ready: bool,
    ) -> Self {
        Self {
            required: cfg!(not(target_arch = "wasm32"))
                && !primary_visible
                && startup_ready
                && primary_output_ready,
            attempt_id,
            primary,
        }
    }

    fn reject(&self, cause: std::fmt::Arguments<'_>) {
        if self.required {
            panic!(
                "SPEC-HAL-018 §16: native startup attempt {:?}, expected primary {:?}: {cause}",
                self.attempt_id, self.primary
            );
        }
    }

    fn require_success<T, E: std::fmt::Debug>(
        &self,
        operation: &str,
        result: Result<T, E>,
    ) -> Result<T, E> {
        result.inspect_err(|cause| self.reject(format_args!("{operation}: {cause:?}")))
    }

    fn require_primary(&self, mut outputs: impl Iterator<Item = SurfaceId>) {
        if self.required && !outputs.any(|surface| surface == self.primary) {
            self.reject(format_args!("Render produced no primary output"));
        }
    }
}

struct CompiledAppAttempt {
    id: crate::core::frame_realization::AppFrameAttemptId,
    plots: Vec<crate::plot::PlotPreparationEvidence>,
    recorder: Option<AppFrameRecorder>,
    primary_output_ready: bool,
    startup_ready: bool,
    should_present: bool,
    render_time: Instant,
    build_scene_started: Instant,
    compile_started: Instant,
    draw_plan_us: u128,
}

impl CompiledAppAttempt {
    fn native_startup_output(
        &self,
        primary_visible: bool,
        base_surface_id: SurfaceId,
    ) -> NativeStartupOutput {
        NativeStartupOutput::new(
            self.id,
            base_surface_id,
            primary_visible,
            self.startup_ready,
            self.primary_output_ready,
        )
    }

    fn check_compile_outcome<T>(
        &mut self,
        result: &Result<
            crate::renderer::RendererCompileOutcome<T>,
            crate::renderer::RendererCompileError,
        >,
        primary_visible: bool,
        base_surface_id: SurfaceId,
    ) {
        let startup = self.native_startup_output(primary_visible, base_surface_id);
        match result {
            Ok(crate::renderer::RendererCompileOutcome::NoTargets) => {
                if let Some(recorder) = &mut self.recorder {
                    recorder.unavailable(AppFrameCompileOutcome::NoTargets);
                }
                startup.reject(format_args!("Compile returned NoTargets"));
            }
            Err(error) => {
                if let Some(recorder) = &mut self.recorder {
                    recorder.unavailable(AppFrameCompileOutcome::Failed(error.clone()));
                }
                startup.reject(format_args!("Compile failed: {error:?}"));
            }
            _ => {}
        }
    }
}

struct SuspendedAppCompile {
    identity: crate::renderer::RendererPreparationIdentity,
    attempt: CompiledAppAttempt,
}

fn cancel_suspended_app_compile(
    renderer: &mut WgpuRenderer,
    suspended: &mut Option<SuspendedAppCompile>,
) {
    if let Some(waiting) = suspended.take() {
        renderer.cancel_pending_preparation(&waiting.identity);
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_input_constraints(
    physical: winit::dpi::PhysicalSize<u32>,
    platform_density: f32,
    scale: f32,
    visible: Option<Size>,
) -> Constraints {
    let logical = Size::new(physical.width as f32 / scale, physical.height as f32 / scale);
    Constraints::loose(visible.map_or(logical, |visible| Size::new(
        logical.width.min(visible.width * platform_density / scale),
        logical.height.min(visible.height * platform_density / scale),
    )))
}

// Both immediate and asynchronously qualified frames use this same Encode and
// sealed submission path. No requested scene is rebuilt to settle a Promise.
fn finish_compiled_app_attempt(
    renderer: &mut WgpuRenderer,
    compiled: crate::renderer::sdf_frame_custody::SdfCompiledFrame,
    attempt: &mut CompiledAppAttempt,
    window: &winit::window::Window,
    base_surface_id: SurfaceId,
    primary_visible: &mut bool,
    required_text_faults: &mut RequiredTextFaultGuard,
    retained_scene_cache: &mut RetainedSceneCache,
    context: &mut AppContext,
) {
    let startup = attempt.native_startup_output(*primary_visible, base_surface_id);
    let compile_us = attempt.compile_started.elapsed().as_micros();
    resize_profiler::add_compile_time_us(compile_us as u64);
    required_text_faults.compiled(attempt.id, compiled.plan());
    let plot_images: Vec<_> = if attempt.primary_output_ready {
        attempt.plots.iter().filter_map(|plot| {
            let front = plot.front.as_ref()?;
            let binding = renderer.compiled_plot_image(compiled.plan(), base_surface_id, front.texture_id());
            Some((front.clone(), binding))
        }).collect()
    } else { Vec::new() };
    if let Some(recorder) = &mut attempt.recorder { recorder.compiled_frame(&compiled); }
    let render_start = Instant::now();
    let outputs = startup
        .require_success("Render failed", renderer.render_sdf_frame(compiled))
        .unwrap_or_else(|error| {
            tracing::error!(
                ?error,
                "SDF output custody failed; frame cancelled before submission"
            );
            Vec::new()
        });
    startup.require_primary(outputs.iter().map(|output| output.surface_id()));
    let render_us = render_start.elapsed().as_micros();
    resize_profiler::add_render_time_us(render_us as u64);
    let submit_start = Instant::now();
    for output in outputs {
        let surface_id = output.surface_id();
        required_text_faults.assert_output_allowed(attempt.id, surface_id);
        if surface_id == base_surface_id {
            let mut first_refusal = None;
            for (front, binding) in &plot_images {
                let Some(required) = front.required_binding() else {
                    continue;
                };
                let expected = required.identity();
                let availability = (Some(expected) == *binding).then(|| required.availability());
                if availability != Some(Ok(())) {
                    first_refusal.get_or_insert((front.texture_id(), expected, *binding, availability));
                    front.withdraw();
                    tracing::warn!(
                        texture_id = front.texture_id(),
                        "Current Plot image refused before target submission"
                    );
                }
            }
            if let Some((texture_id, expected, compiled, availability)) = first_refusal {
                renderer.gpu_resources().image_admission.notify();
                startup.reject(format_args!(
                    "Plot texture {texture_id} refused: required binding {expected:?}, compiled binding {compiled:?}, availability check {availability:?}"
                ));
                continue;
            }
        }
        if let Some(recorder) = &mut attempt.recorder {
            recorder.output_produced(surface_id);
        }
        if !*primary_visible && attempt.startup_ready && surface_id == base_surface_id {
            crate::platform::window::prepare_startup_visibility(window);
            if let Err(error) = startup.require_success(
                "Startup submission failed",
                renderer.submit_sdf_output_startup(output, || window.set_visible(true)),
            ) {
                tracing::error!(
                    ?error,
                    "Startup Image custody validation failed; target not presented"
                );
                continue;
            }
            crate::platform::window::finish_startup_visibility(window);
            *primary_visible = true;
        } else if let Err(error) = renderer.submit_sdf_output(output) {
            tracing::error!(
                ?error,
                ?surface_id,
                "SDF submission rejected; target not presented"
            );
            continue;
        }
        required_text_faults.submitted_and_presented(attempt.id, surface_id);
        if surface_id == base_surface_id {
            for (front, binding) in &plot_images {
                if let Some(binding) = binding {
                    front.record_submitted(*binding);
                }
            }
        }
        retained_scene_cache.note_sdf_target_output(surface_id);
        context
            .ui_scale
            .set_output(surface_id, UiScaleTargetOutput::Submitted);
        if let Some(runtime) = &mut retained_scene_cache.presentation {
            runtime.submitted(attempt.id, surface_id, renderer.frame_epoch());
        }
        if let Some(recorder) = &mut attempt.recorder {
            recorder.submitted_and_presented(surface_id);
        }
    }
    let submit_us = submit_start.elapsed().as_micros();
    resize_profiler::add_submit_time_us(submit_us as u64);
    renderer.advance_frame();
    tracing::debug!(
        build_scene_us = attempt.build_scene_started.elapsed().as_micros(),
        draw_plan_us = attempt.draw_plan_us,
        compile_us,
        render_us,
        submit_us,
        "frame: pipeline stages"
    );
}

fn publish_framework_appearance(
    tree: &mut WidgetTree,
    cache: &mut RetainedSceneCache,
    next: Option<&FrameworkAppearanceSnapshot>,
) {
    let previous = cache.appearance.as_ref();
    if previous == next {
        return;
    }
    let mut owners: Vec<_> = previous
        .into_iter()
        .chain(next)
        .flat_map(FrameworkAppearanceSnapshot::owners)
        .collect();
    owners.sort_unstable_by_key(|owner| owner.0);
    owners.dedup();
    for owner in owners {
        if previous.map(|snapshot| snapshot.for_owner(owner))
            != next.map(|snapshot| snapshot.for_owner(owner))
        {
            if let Some(node) = tree.find_by_id(owner) {
                tree.mark_paint_dirty(node);
            }
        }
    }
    cache.appearance = next.cloned();
}

#[derive(Clone, PartialEq)]
struct RetainedTransientState {
    drag: Option<RetainedDragState>,
    tooltip: Option<TooltipRenderState>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RetainedDragState {
    source_widget: TreeWidgetId,
    display_x_bits: u32,
    display_y_bits: u32,
    width_bits: u32,
    height_bits: u32,
    alpha_bits: u32,
}

impl RetainedDragState {
    fn capture(drag: &crate::core::drag_drop::DragSession) -> Self {
        Self {
            source_widget: drag.source_widget,
            display_x_bits: drag.display_pos.x.to_bits(),
            display_y_bits: drag.display_pos.y.to_bits(),
            width_bits: drag.source_size.width.to_bits(),
            height_bits: drag.source_size.height.to_bits(),
            alpha_bits: drag.ghost_alpha.to_bits(),
        }
    }
}

struct RetainedPaintOutcome {
    scene: Option<SceneGraph>,
    overlay_hit_regions: Vec<OverlayHitEntry>,
    visual_changed: bool,
}

struct RetainedPaintEnvironment<'a> {
    tree: &'a WidgetTree,
    theme: &'a Theme,
    text_engine: &'a mut dyn TextEngine,
    icon_registry: &'a crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    timeline: &'a crate::core::animation::Timeline,
    interpolated_rects: &'a InterpolatedVisualRects,
    scale_factor: f32,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: KeyboardAuthorityMode,
    appearance: Option<&'a FrameworkAppearanceSnapshot>,
    debug_outlines: bool,
    cache: &'a mut RetainedSceneCache,
    cleaned_nodes: Vec<NodeId>,
    recomposed_fragments: usize,
}

impl RetainedPaintEnvironment<'_> {
    fn build_fragment(&mut self, node_id: NodeId, depth: usize) -> Option<RetainedWidgetScene> {
        let node = self.tree.node(node_id)?;
        if !self.tree.composite_presentation_visible(node_id) {
            return None;
        }

        #[cfg(debug_assertions)]
        debug_assert!(
            node.layout_epoch > 0 && !node.dirty_layout,
            "SPEC-HAL-354: retained paint requires valid completed geometry for {:?}",
            node_id
        );

        let widget_id = node.id;
        let cached = self.cache.entries.get(&widget_id).cloned();
        if !node.dirty_composition {
            if let Some(cached) = cached {
                return Some(cached);
            }
        }

        let repaint_local = node.dirty_paint || cached.is_none();
        let painted = if repaint_local {
            self.paint_local_fragments(node_id, depth)?
        } else {
            let cached = cached.as_ref()?;
            PaintedLocalFragments {
                local: cached.local.clone(),
                overlay: cached.overlay.clone(),
                overlay_eligibility: cached.overlay_eligibility,
                overlay_hit_regions: cached.own_overlay_hit_regions.clone(),
                tooltip_triggers: cached.own_tooltip_triggers.clone(),
                #[cfg(debug_assertions)]
                key_widget_map: cached.own_key_widget_map.clone(),
            }
        };
        let local = painted.local;
        let overlay = painted.overlay;
        let overlay_eligibility = painted.overlay_eligibility;
        let own_overlay_hit_regions = painted.overlay_hit_regions;
        let own_tooltip_triggers = painted.tooltip_triggers;
        #[cfg(debug_assertions)]
        let own_key_widget_map = painted.key_widget_map;

        let (
            abs_rect,
            children,
            spec088_clip,
            clip_bounds,
            clip_rounded,
            scroll_offset,
        ) = {
            let node = self.tree.node(node_id)?;
            (
                self.tree
                    .get_absolute_paint_rect(node_id)
                    .unwrap_or(RectF::zero()),
                self.tree.control_subtree_children(node_id),
                self.tree.child_clip_source(node_id),
                node.widget.clip_bounds(),
                node.widget.clip_bounds_rounded(),
                node.widget.scroll_offset(),
            )
        };

        let mut builder = SceneBuilder::with_scale_factor(self.scale_factor);
        builder.push_retained_fragment(local.clone());
        let mut main_affordances = Vec::new();
        let mut overlay_hit_regions = own_overlay_hit_regions.clone();
        let mut tooltip_triggers = own_tooltip_triggers.clone();
        #[cfg(debug_assertions)]
        let mut key_widget_map = own_key_widget_map.clone();

        if let Some(source) = spec088_clip {
            builder.begin_clip(source, None);
        }
        let absolute_clip = clip_bounds.map(|clip_rect| {
            RectF::new(
                abs_rect.x + clip_rect.x,
                abs_rect.y + clip_rect.y,
                clip_rect.width,
                clip_rect.height,
            )
        });
        if let Some(abs_clip) = absolute_clip {
            #[cfg(debug_assertions)]
            if std::env::var("HALCYON_DEBUG_CLIPS").is_ok() {
                paint_clip_debug_outline(&mut builder, abs_clip);
            }
            builder.begin_clip(abs_clip, clip_rounded.map(Into::into));
        }
        if let Some(offset) = scroll_offset {
            builder.begin_scroll(offset);
        }

        for (child_id, member) in children {
            if let Some(child_rect) = self.tree.get_absolute_paint_rect(child_id) {
                if child_rect.width <= 0.0 && child_rect.height <= 0.0 {
                    continue;
                }
            }
            if let Some(child) = self.build_fragment(child_id, depth + 1) {
                builder.push_control_subtree_fragment(child.composed.clone(), member);
                for export in child.main_affordances {
                    if export.member.scope == widget_id {
                        builder.push_control_subtree_fragment(export.nodes, Some(export.member));
                    } else {
                        let mut scoped = SceneBuilder::with_scale_factor(self.scale_factor);
                        if let Some(source) = spec088_clip {
                            scoped.begin_clip(source, None);
                        }
                        if let Some(clip) = absolute_clip {
                            scoped.begin_clip(clip, clip_rounded.map(Into::into));
                        }
                        if let Some(offset) = scroll_offset {
                            scoped.begin_scroll(offset);
                        }
                        scoped.push_retained_fragment(export.nodes);
                        if scroll_offset.is_some() {
                            scoped.end_scroll();
                        }
                        if absolute_clip.is_some() {
                            scoped.end_clip();
                        }
                        if spec088_clip.is_some() {
                            scoped.end_clip();
                        }
                        main_affordances.push(MainAffordanceExport {
                            member: export.member,
                            nodes: Arc::from(scoped.build().root),
                        });
                    }
                }
                overlay_hit_regions.extend(child.overlay_hit_regions);
                tooltip_triggers.extend(child.tooltip_triggers.iter().cloned());
                #[cfg(debug_assertions)]
                merge_retained_key_widget_map(&mut key_widget_map, &child.key_widget_map);
            }
        }

        if scroll_offset.is_some() {
            builder.end_scroll();
        }
        if clip_bounds.is_some() {
            builder.end_clip();
        }
        if spec088_clip.is_some() {
            builder.end_clip();
        }
        if let Some(member) = self.tree.control_subtree_main_affordance(node_id) {
            overlay_eligibility.assert_exportable();
            main_affordances.push(MainAffordanceExport {
                member,
                nodes: overlay.clone(),
            });
        } else {
            builder.push_retained_fragment(overlay.clone());
        }

        let composed = Arc::<[SceneNode]>::from(builder.build().root);
        let retained = RetainedWidgetScene {
            local,
            overlay,
            overlay_eligibility,
            own_overlay_hit_regions,
            own_tooltip_triggers,
            composed,
            main_affordances,
            overlay_hit_regions,
            tooltip_triggers,
            #[cfg(debug_assertions)]
            own_key_widget_map,
            #[cfg(debug_assertions)]
            key_widget_map,
        };
        self.cache.entries.insert(widget_id, retained.clone());
        self.cleaned_nodes.push(node_id);
        self.recomposed_fragments += 1;
        Some(retained)
    }

    fn paint_local_fragments(
        &mut self,
        node_id: NodeId,
        depth: usize,
    ) -> Option<PaintedLocalFragments> {
        let node = self.tree.node(node_id)?;
        let widget_id = node.id;
        let abs_rect = self
            .tree
            .get_absolute_paint_rect(node_id)
            .unwrap_or(RectF::zero());
        let visual_rect = self
            .interpolated_rects
            .get(&widget_id)
            .copied()
            .unwrap_or(abs_rect);
        let children = self.tree.children(node_id);

        let mut local_builder = SceneBuilder::with_scale_factor(self.scale_factor);
        let mut local_overlay_hit_regions = Vec::new();
        let mut local_tooltip_triggers = Vec::new();
        {
            let mut cx = PaintContext::new_with_overlay_output(
                self.tree,
                &mut local_builder,
                self.theme,
                self.text_engine,
                self.icon_registry,
                self.frame_time_ms,
                self.timeline,
                self.interpolated_rects,
                OverlayHitOutput::Retained(&mut local_overlay_hit_regions),
                &mut local_tooltip_triggers,
                self.scale_factor,
                self.focused_widget,
                self.keyboard_mode,
            );
            crate::core::deferred_wiring::assert_sealed_for_phase(node.widget.as_ref(), "paint");
            cx.set_current_node(node_id);
            cx.set_appearance_snapshot(self.appearance);
            cx.set_button_sdf_material(
                self.cache
                    .sdf_paint_inputs
                    .as_ref()
                    .and_then(|inputs| inputs.get(&widget_id)),
            );
            cx.scene.set_current_painting_widget(widget_id.0);
            resize_profiler::inc_local_paint();
            node.widget.paint(&mut cx, visual_rect, &children);
            cx.paint_focus_indicator_for(widget_id, visual_rect);
            if self.debug_outlines {
                paint_debug_outline(&mut cx, visual_rect, depth);
            }
            cx.scene.clear_current_painting_widget();
        }
        let mut image_dependencies = local_builder.take_admitted_image_dependencies();
        let local_scene = local_builder.build();
        #[cfg(debug_assertions)]
        let mut own_key_widget_map = local_scene.key_widget_map().clone();
        let local = Arc::<[SceneNode]>::from(local_scene.root);

        let mut overlay_builder = SceneBuilder::with_scale_factor(self.scale_factor);
        let mut overlay_hit_regions = Vec::new();
        let mut overlay_tooltip_triggers = Vec::new();
        {
            let mut cx = PaintContext::new_with_overlay_output(
                self.tree,
                &mut overlay_builder,
                self.theme,
                self.text_engine,
                self.icon_registry,
                self.frame_time_ms,
                self.timeline,
                self.interpolated_rects,
                OverlayHitOutput::Retained(&mut overlay_hit_regions),
                &mut overlay_tooltip_triggers,
                self.scale_factor,
                self.focused_widget,
                self.keyboard_mode,
            );
            cx.set_current_node(node_id);
            cx.set_appearance_snapshot(self.appearance);
            cx.scene.set_current_painting_widget(widget_id.0);
            node.widget.paint_overlay(&mut cx, abs_rect, widget_id);
            cx.scene.clear_current_painting_widget();
        }
        image_dependencies.extend(overlay_builder.take_admitted_image_dependencies());
        self.cache
            .replace_image_dependencies(widget_id, image_dependencies);
        let overlay_scene = overlay_builder.build();
        #[cfg(debug_assertions)]
        merge_retained_key_widget_map(&mut own_key_widget_map, overlay_scene.key_widget_map());
        let overlay = Arc::<[SceneNode]>::from(overlay_scene.root);
        let overlay_eligibility = MainAffordanceEligibility::capture(
            &overlay,
            !overlay_hit_regions.is_empty(),
            !overlay_tooltip_triggers.is_empty(),
        );

        local_overlay_hit_regions.extend(overlay_hit_regions);
        local_tooltip_triggers.extend(overlay_tooltip_triggers);
        Some(PaintedLocalFragments {
            local,
            overlay,
            overlay_eligibility,
            overlay_hit_regions: local_overlay_hit_regions,
            tooltip_triggers: local_tooltip_triggers,
            #[cfg(debug_assertions)]
            key_widget_map: own_key_widget_map,
        })
    }
}

#[cfg(debug_assertions)]
fn merge_retained_key_widget_map(
    destination: &mut HashMap<SceneNodeKey, u64>,
    source: &HashMap<SceneNodeKey, u64>,
) {
    for (key, widget_id) in source {
        let previous = destination.insert(*key, *widget_id);
        debug_assert!(
            previous.is_none(),
            "SPEC-HAL-083: retained SceneNodeKey {:?} was emitted more than once",
            key
        );
    }
}

fn paint_debug_outline(cx: &mut PaintContext, visual_rect: RectF, depth: usize) {
    let one_px = 1.0 / cx.scale_factor;
    let thick = 3.0 * one_px;
    let outline_color = match depth % 4 {
        0 => crate::core::paint::Color::from_srgb(1.0, 0.0, 0.0, 0.8),
        1 => crate::core::paint::Color::from_srgb(0.0, 0.8, 0.0, 0.8),
        2 => crate::core::paint::Color::from_srgb(0.0, 0.4, 1.0, 0.8),
        _ => crate::core::paint::Color::from_srgb(1.0, 0.6, 0.0, 0.8),
    };
    cx.scene.push_rect(
        RectF::new(visual_rect.x, visual_rect.y, visual_rect.width, thick),
        outline_color,
        99,
        None,
    );
    cx.scene.push_rect(
        RectF::new(
            visual_rect.x,
            visual_rect.y + visual_rect.height - thick,
            visual_rect.width,
            thick,
        ),
        outline_color,
        99,
        None,
    );
    cx.scene.push_rect(
        RectF::new(visual_rect.x, visual_rect.y, thick, visual_rect.height),
        outline_color,
        99,
        None,
    );
    cx.scene.push_rect(
        RectF::new(
            visual_rect.x + visual_rect.width - thick,
            visual_rect.y,
            thick,
            visual_rect.height,
        ),
        outline_color,
        99,
        None,
    );
}

#[cfg(debug_assertions)]
fn paint_clip_debug_outline(builder: &mut SceneBuilder, rect: RectF) {
    let color = crate::core::paint::Color::from_srgb(1.0, 0.0, 1.0, 0.8).into();
    let width = 2.0;
    builder.push_rect(
        RectF::new(rect.x, rect.y, rect.width, width),
        color,
        100,
        None,
    );
    builder.push_rect(
        RectF::new(rect.x, rect.y + rect.height - width, rect.width, width),
        color,
        100,
        None,
    );
    builder.push_rect(
        RectF::new(rect.x, rect.y, width, rect.height),
        color,
        100,
        None,
    );
    builder.push_rect(
        RectF::new(rect.x + rect.width - width, rect.y, width, rect.height),
        color,
        100,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_tree_reusing(
    tree: &mut WidgetTree,
    root: NodeId,
    theme: &Theme,
    text_engine: &mut dyn TextEngine,
    icon_registry: &crate::core::icon::IconRegistry,
    frame_time_ms: u64,
    timeline: &crate::core::animation::Timeline,
    interpolated_rects: &InterpolatedVisualRects,
    scale_factor: f32,
    active_drag: Option<&RetainedDragState>,
    tooltip_triggers: &mut Vec<crate::model::tooltip::TooltipTrigger>,
    tooltip_render: TooltipSceneInput<'_>,
    focused_widget: Option<TreeWidgetId>,
    keyboard_mode: KeyboardAuthorityMode,
    appearance: Option<&FrameworkAppearanceSnapshot>,
    force_scene: bool,
    recycled_scene: &mut Option<RecycledSceneStorage>,
    retained_cache: &mut RetainedSceneCache,
    mut presentation: Option<&mut PresentationRuntime>,
    presentation_target: Option<CustomRenderTargetSnapshot>,
    debug_outlines: bool,
) -> RetainedPaintOutcome {
    tree.assert_control_subtree_orders_current();
    let structure_generation = tree.structure_generation();
    if retained_cache.structure_generation != structure_generation {
        retained_cache.entries.clear();
        retained_cache.image_dependencies = None;
        retained_cache.structure_generation = structure_generation;
    }
    if let Some(dependencies) = &retained_cache.image_dependencies {
        dependencies.invalidate(tree);
    }
    let context_key = RetainedPaintContextKey {
        scale_factor_bits: scale_factor.to_bits(),
        focused_widget,
        keyboard_mode,
        debug_outlines,
    };
    if let Some(previous) = retained_cache.context_key {
        if previous.focused_widget != focused_widget || previous.keyboard_mode != keyboard_mode {
            for owner in previous.focused_widget.into_iter().chain(focused_widget) {
                if let Some(node) = tree.find_by_id(owner) {
                    tree.mark_paint_dirty(node);
                }
            }
        }
    }
    if retained_cache.context_key.map(|key| key.scale_factor_bits)
        != Some(context_key.scale_factor_bits)
        || cfg!(debug_assertions) && std::env::var("HALCYON_DEBUG_CLIPS").is_ok()
    {
        tree.mark_subtree_paint_dirty(root);
    }
    retained_cache.context_key = Some(context_key);
    for (owner, rect) in &retained_cache.interpolated_rects {
        if interpolated_rects.get(owner) != Some(rect) {
            tree.mark_paint_dirty_by_widget_id(*owner);
        }
    }
    for (owner, rect) in interpolated_rects {
        if retained_cache.interpolated_rects.get(owner) != Some(rect) {
            tree.mark_paint_dirty_by_widget_id(*owner);
        }
    }
    retained_cache
        .interpolated_rects
        .clone_from(interpolated_rects);
    tree.invalidate_changed_combo_popup_hits();

    let drag_state = active_drag.cloned();

    let root_widget_id = tree.node(root).map(|node| node.id);
    let root_is_clean = tree.node(root).is_some_and(|node| !node.dirty_composition);
    if !force_scene && root_is_clean {
        if let Some(root_scene) =
            root_widget_id.and_then(|widget_id| retained_cache.entries.get(&widget_id))
        {
            let transient_state = RetainedTransientState {
                drag: drag_state,
                tooltip: tooltip_render.resolve(&root_scene.tooltip_triggers, theme, text_engine),
            };
            if retained_cache.transient_state.as_ref() == Some(&transient_state) {
                tooltip_triggers.extend(root_scene.tooltip_triggers.iter().cloned());
                return RetainedPaintOutcome {
                    scene: None,
                    overlay_hit_regions: root_scene.overlay_hit_regions.clone(),
                    visual_changed: false,
                };
            }
        }
    }

    let (root_scene, cleaned_nodes, recomposed_fragments) = {
        let mut environment = RetainedPaintEnvironment {
            tree,
            theme,
            text_engine,
            icon_registry,
            frame_time_ms,
            timeline,
            interpolated_rects,
            scale_factor,
            focused_widget,
            keyboard_mode,
            appearance,
            debug_outlines,
            cache: retained_cache,
            cleaned_nodes: Vec::new(),
            recomposed_fragments: 0,
        };
        let root_scene = environment.build_fragment(root, 0).unwrap_or_default();
        assert!(
            root_scene.main_affordances.is_empty(),
            "INV-359-MAIN-AFFORDANCE-ONCE: unconsumed export at scene root"
        );
        (
            root_scene,
            environment.cleaned_nodes,
            environment.recomposed_fragments,
        )
    };

    for node_id in cleaned_nodes {
        if let Some(node) = tree.node_mut(node_id) {
            node.dirty_paint = false;
            node.dirty_composition = false;
            node.dirty_flags = WidgetDirty::Unchanged;
        }
    }

    let root_scene = match presentation.as_deref_mut() {
        Some(runtime) => runtime.compose(
            tree,
            retained_cache,
            root,
            &root_scene,
            text_engine,
            scale_factor,
            presentation_target,
            interpolated_rects,
            appearance,
        ),
        None => root_scene,
    };
    tooltip_triggers.extend(root_scene.tooltip_triggers.iter().cloned());
    let tooltip_render = tooltip_render.resolve(tooltip_triggers, theme, text_engine);
    let transient_state = RetainedTransientState {
        drag: drag_state,
        tooltip: tooltip_render.clone(),
    };
    let transient_changed = retained_cache.transient_state.as_ref() != Some(&transient_state);
    retained_cache.transient_state = Some(transient_state);
    let mut overlay_hit_regions = root_scene.overlay_hit_regions;
    let mut scene_builder = recycled_scene.take().map_or_else(
        || SceneBuilder::with_scale_factor(scale_factor),
        |storage| SceneBuilder::with_recycled_storage(scale_factor, storage),
    );
    scene_builder.set_clear_color(theme.colors.surfaces.base0.into());
    scene_builder.push_retained_fragment(root_scene.composed);
    #[cfg(debug_assertions)]
    scene_builder.extend_retained_key_widget_map(&root_scene.key_widget_map);

    {
        let mut cx = PaintContext::new_with_overlay_output(
            tree,
            &mut scene_builder,
            theme,
            text_engine,
            icon_registry,
            frame_time_ms,
            timeline,
            interpolated_rects,
            OverlayHitOutput::Retained(&mut overlay_hit_regions),
            tooltip_triggers,
            scale_factor,
            focused_widget,
            keyboard_mode,
        );

        if let Some(drag) = active_drag {
            let ghost_alpha = f32::from_bits(drag.alpha_bits);
            if ghost_alpha > 0.001 {
                if let Some(node_id) = cx.tree.find_node_by_widget_id(drag.source_widget) {
                    let ghost_rect = RectF::new(
                        f32::from_bits(drag.display_x_bits) + 8.0,
                        f32::from_bits(drag.display_y_bits) + 8.0,
                        f32::from_bits(drag.width_bits),
                        f32::from_bits(drag.height_bits),
                    );
                    if let Some(node) = cx.tree.node(node_id) {
                        node.widget
                            .paint_drag_proxy(&mut cx, ghost_rect, ghost_alpha);
                    }
                }
            }
        }

        if let Some(tooltip) = tooltip_render.as_ref() {
            crate::widgets::tooltip_surface::TooltipSurface::paint_standalone(
                &mut cx,
                tooltip.anchor,
                &tooltip.content,
            );
        }
    }

    RetainedPaintOutcome {
        scene: Some(scene_builder.build()),
        overlay_hit_regions,
        visual_changed: recomposed_fragments > 0 || transient_changed,
    }
}

/// THE HALCYON COORDINATE MODEL - SPEC-HAL-024 Stage 7
/// Paint traversal uses `absolute_paint_rect` as the single source of truth.
/// Scroll offset is already baked into `absolute_paint_rect` by `compute_absolute_paint_rects()`.
///
/// SPEC-HAL-035: ancestor_dirty indicates whether any ancestor is dirty.
/// If this node AND all ancestors are Unchanged, we can skip painting entirely.
fn paint_node(
    node_id: NodeId,
    cx: &mut PaintContext,
    ancestor_dirty: bool,
    depth: usize,
    debug_outlines: bool,
) {
    if depth == 0 {
        cx.tree.assert_control_subtree_orders_current();
    }
    // Get node
    let node = match cx.tree.node(node_id) {
        Some(n) => n,
        None => return,
    };

    if !cx.tree.composite_presentation_visible(node_id) {
        return;
    }

    // DEBUG: INV-PAINT-REQUIRES-LAYOUT-EPOCH assertion.
    // Reused geometry may originate in an earlier completed layout epoch.
    #[cfg(debug_assertions)]
    {
        debug_assert!(
            node.layout_epoch > 0 && !node.dirty_layout,
            "INV-PAINT-REQUIRES-LAYOUT-EPOCH VIOLATION: node {:?} painted with invalid \
             retained geometry (layout_epoch={}, dirty_layout={})",
            node_id,
            node.layout_epoch,
            node.dirty_layout
        );
    }

    // SPEC-HAL-035: Check dirty flags for skip optimization
    let dirty_flags = node.dirty_flags;
    let this_dirty = dirty_flags != WidgetDirty::Unchanged;
    let mut subtree_dirty = ancestor_dirty || this_dirty;

    // Force repaint for widgets that need continuous animation frames.
    // needs_animation() == true means the widget's visual output changes
    // every frame (live data, ongoing animation).
    subtree_dirty = subtree_dirty || node.widget.needs_animation();

    // SPEC-HAL-035: If neither this node nor any ancestor is dirty, skip entirely
    // This is a conservative check - we only skip when we're certain nothing changed
    if !subtree_dirty {
        return;
    }

    // SPEC-HAL-024 Stage 7: Use pre-computed absolute_paint_rect as canonical geometry
    // SPEC-HAL-086 Stage 7: This rect is in absolute logical coordinates WITHOUT scroll applied.
    // Scroll transforms are now in the scene graph via ScrollNode, not baked into positions.
    // SPEC-HAL-141: Use getter for private geometry field
    let abs_rect = cx
        .tree
        .get_absolute_paint_rect(node_id)
        .unwrap_or(RectF::zero());

    // SPEC-HAL-078 Stage 3: Store widget ID for paint-time interpolation lookup
    let widget_id = node.id;

    // HAL088 §17: a declaring allocation, not an inherited viewport aggregate.
    let spec088_clip = cx.tree.child_clip_source(node_id);

    // Get children before calling widget.paint (to avoid borrow conflicts)
    let children = cx.tree.children(node_id);

    // Check if widget needs clipping and/or scrolling for its children
    let (clip_bounds, clip_rounded, scroll_offset) = if let Some(node) = cx.tree.node(node_id) {
        let bounds = node.widget.clip_bounds();
        // SPEC-R-020 Stage 5: Query rounded corners only if clipping is active
        let rounded = if bounds.is_some() {
            node.widget.clip_bounds_rounded()
        } else {
            None
        };
        // SPEC-HAL-086 Stage 7: Get scroll offset for scene graph scroll node
        let scroll = node.widget.scroll_offset();
        (bounds, rounded, scroll)
    } else {
        (None, None, None)
    };

    // SPEC-HAL-078 Stage 3: Compute visual rect for paint (may be interpolated)
    // LT-INV-30: Layout remains authoritative (abs_rect unchanged)
    // LT-INV-31: Interpolation is paint-only (only affects visual_rect)
    // LT-INV-32: Input & hit-testing use non-interpolated geometry
    let visual_rect = cx.visual_rect_for_paint(widget_id, abs_rect);

    // SPEC Section 6: Call widget.paint with visual rect (may be interpolated)
    // Widget must paint ONLY itself, NOT its children
    // LT-INV-33: No mutation of WidgetTree during paint
    if let Some(node) = cx.tree.node(node_id) {
        // SPEC-HAL-154: Centralized paint guard (debug-only).
        crate::core::deferred_wiring::assert_sealed_for_phase(node.widget.as_ref(), "paint");

        // SPEC-HAL-190: Set current node for authority lookup.
        cx.set_current_node(node_id);

        // SPEC-HAL-083 Phase-9: Set current painting widget for key emission tracking
        #[cfg(debug_assertions)]
        cx.scene.set_current_painting_widget(widget_id.0);

        node.widget.paint(cx, visual_rect, &children);

        // SPEC-HAL-083 Phase-9: Clear current painting widget after paint
        #[cfg(debug_assertions)]
        cx.scene.clear_current_painting_widget();
    }

    // SPEC-HAL-328 §12: Emit the framework focus indicator inside the same
    // scroll-node / clip scope as the widget's own primitives. paint_node's
    // ancestors have already pushed `begin_scroll` / `begin_clip` wrappings
    // for any scroll containers between root and this node, so the indicator
    // primitive gets the same scroll translation as everything the widget
    // just painted. No-op if widget_id is not the focused widget or if the
    // widget opts out via `paints_own_focus_indicator() == true`.
    cx.paint_focus_indicator_for(widget_id, visual_rect);

    // HAL-354 §10: Paint the App-owned widget outline overlay.
    if debug_outlines {
        let one_px = 1.0 / cx.scale_factor;
        let thick = 3.0 * one_px; // 3 physical pixels — visible in screenshots
        let outline_color = match depth % 4 {
            0 => crate::core::paint::Color::from_srgb(1.0, 0.0, 0.0, 0.8), // Red
            1 => crate::core::paint::Color::from_srgb(0.0, 0.8, 0.0, 0.8), // Green
            2 => crate::core::paint::Color::from_srgb(0.0, 0.4, 1.0, 0.8), // Blue
            _ => crate::core::paint::Color::from_srgb(1.0, 0.6, 0.0, 0.8), // Orange
        };
        // Top
        cx.scene.push_rect(
            RectF::new(visual_rect.x, visual_rect.y, visual_rect.width, thick),
            outline_color,
            99,
            None,
        );
        // Bottom
        cx.scene.push_rect(
            RectF::new(
                visual_rect.x,
                visual_rect.y + visual_rect.height - thick,
                visual_rect.width,
                thick,
            ),
            outline_color,
            99,
            None,
        );
        // Left
        cx.scene.push_rect(
            RectF::new(visual_rect.x, visual_rect.y, thick, visual_rect.height),
            outline_color,
            99,
            None,
        );
        // Right
        cx.scene.push_rect(
            RectF::new(
                visual_rect.x + visual_rect.width - thick,
                visual_rect.y,
                thick,
                visual_rect.height,
            ),
            outline_color,
            99,
            None,
        );
    }

    // This child boundary precedes this owner's ScrollNode.
    if let Some(source) = spec088_clip {
        cx.scene.begin_clip(source, None);
    }

    // SPEC Section 5: Paint traversal applies clipping, not widgets
    // If widget needs clipping, wrap children in a clip node
    if let Some(clip_rect) = clip_bounds {
        // Convert relative clip bounds to absolute coordinates
        let abs_clip = RectF::new(
            abs_rect.x + clip_rect.x,
            abs_rect.y + clip_rect.y,
            clip_rect.width,
            clip_rect.height,
        );

        // Stage 42H: Debug overlay for clip visualization.
        // INV-42H-4 (Clip Stack Validity): Visualize clip rects to verify correctness.
        // Feature-gated: only in debug builds with explicit env var.
        #[cfg(debug_assertions)]
        if std::env::var("HALCYON_DEBUG_CLIPS").is_ok() {
            // Draw colored outline for clip rect (z-order 100 to appear on top)
            // Using 4 edge rectangles since PaintSink has simplified push_border_rect
            let outline_color = crate::core::paint::Color::from_srgb(1.0, 0.0, 1.0, 0.8); // Magenta
            let bw = 2.0; // border width
            // Top edge
            cx.scene.push_rect(
                RectF::new(abs_clip.x, abs_clip.y, abs_clip.width, bw),
                outline_color,
                100,
                cx.scene_key,
            );
            // Bottom edge
            cx.scene.push_rect(
                RectF::new(
                    abs_clip.x,
                    abs_clip.y + abs_clip.height - bw,
                    abs_clip.width,
                    bw,
                ),
                outline_color,
                100,
                cx.scene_key,
            );
            // Left edge
            cx.scene.push_rect(
                RectF::new(abs_clip.x, abs_clip.y, bw, abs_clip.height),
                outline_color,
                100,
                cx.scene_key,
            );
            // Right edge
            cx.scene.push_rect(
                RectF::new(
                    abs_clip.x + abs_clip.width - bw,
                    abs_clip.y,
                    bw,
                    abs_clip.height,
                ),
                outline_color,
                100,
                cx.scene_key,
            );
        }

        // SPEC-R-020 Stage 5: Pass rounded corners to scene builder
        cx.scene.begin_clip(abs_clip, clip_rounded);
    }

    // SPEC-HAL-086 Stage 7: If widget has scroll offset, wrap children in ScrollNode
    // ScrollNode applies translation during batcher flattening, not during layout
    if let Some(offset) = scroll_offset {
        cx.scene.begin_scroll(offset);
    }

    // SPEC-HAL-024 Stage 7: Children already have their own absolute_paint_rect
    // SPEC-HAL-086 Stage 7: Children's absolute_paint_rect no longer has scroll baked in.
    // Scroll transform is applied by ScrollNode during batcher flattening.
    // SPEC-HAL-035: Propagate subtree_dirty to children
    for (child_id, member) in cx.tree.control_subtree_children(node_id) {
        // Skip children whose rect has been collapsed to zero
        // by a content multiplexer (e.g. TabSwitcher hides inactive tabs by
        // setting rect to zero). Layout was still run on these nodes (so
        // INV-PAINT-REQUIRES-LAYOUT-EPOCH holds), but they must not paint.
        if let Some(child_rect) = cx.tree.get_absolute_paint_rect(child_id) {
            if child_rect.width <= 0.0 && child_rect.height <= 0.0 {
                continue;
            }
        }
        if let Some(member) = member {
            cx.scene.begin_control_subtree_member(member);
        }
        paint_node(child_id, cx, subtree_dirty, depth + 1, debug_outlines);
        if member.is_some() {
            cx.scene.end_control_subtree_member();
        }
    }

    // SPEC-HAL-086 Stage 7: End scroll transform if we started one
    if scroll_offset.is_some() {
        cx.scene.end_scroll();
    }

    // End clipping if we started it
    if clip_bounds.is_some() {
        cx.scene.end_clip();
    }

    // SPEC-HAL-088 Stage 5: End viewport clip if we started one
    if spec088_clip.is_some() {
        cx.scene.end_clip();
    }

    // Paint overlay elements (e.g., scrollbars) after children
    // Pass tree-managed widget_id (node.id) for scene nodes that need hit-test routing
    if let Some(node) = cx.tree.node(node_id) {
        let tree_widget_id = node.id;
        node.widget.paint_overlay(cx, abs_rect, tree_widget_id);
    }
}

// ============================================================================
// SPEC-HAL-044: UI Engine - Dedicated UI Thread Abstraction
// ============================================================================

/// SPEC-HAL-044: Configuration for spawning a UI thread.
///
/// Contains only Send-able configuration. The widget tree is built inside
/// the UI thread via a builder closure, so widgets don't need to be Send.
pub struct UiThreadConfig {
    /// Application input policy installed before the first input and paint.
    pub ui_interaction_policy: crate::core::event::UiInteractionPolicy,
    /// Theme for widget rendering
    pub theme: Theme,
    /// Text engine for layout/paint (must be Send)
    pub text_engine: Box<dyn TextEngine + Send>,
    /// Initial layout constraints
    pub initial_constraints: Constraints,
    /// SPEC-HAL-018 Stage 14: Optional initial screen builder.
    /// If provided, this closure is called inside the UI thread to create
    /// the initial screen, which is then pushed onto the screen stack.
    /// The closure runs in the UI thread, so the Screen doesn't need to be Send.
    pub initial_screen: Option<Box<dyn FnOnce() -> Box<dyn Screen> + Send>>,
    /// SPEC-HAL-266: Initial device pixel ratio from the OS window.
    pub initial_scale_factor: f32,
}

impl UiThreadConfig {
    pub fn new(
        theme: Theme,
        text_engine: Box<dyn TextEngine + Send>,
        initial_constraints: Constraints,
    ) -> Self {
        Self {
            ui_interaction_policy: Default::default(),
            theme,
            text_engine,
            initial_constraints,
            initial_screen: None,
            initial_scale_factor: 1.0,
        }
    }

    pub fn with_ui_interaction_policy(
        mut self,
        policy: crate::core::event::UiInteractionPolicy,
    ) -> Self {
        self.ui_interaction_policy = policy;
        self
    }

    pub(crate) fn interaction_domains(&self) -> InteractionDomains {
        InteractionDomains::with_ui_interaction_policy(self.ui_interaction_policy)
    }
}

/// SPEC-HAL-044/045: Spawn a UI thread that runs the engine loop.
///
/// The widget tree is built inside the UI thread via the `builder` closure,
/// so widgets don't need to implement Send. Only the configuration and
/// swapchain cross the thread boundary.
///
/// # Arguments
/// - `config`: Send-able configuration (theme, text engine, constraints)
/// - `builder`: Closure that builds the widget tree inside the UI thread
/// - `input_rx`: Receiver for raw input events from platform thread
/// - `scene_swapchain`: Triple-buffer swapchain for scene frames (SPEC-HAL-045)
///
/// # Returns
/// A `JoinHandle` for the spawned UI thread.
pub fn spawn_ui_thread<F>(
    config: UiThreadConfig,
    builder: F,
    input_rx: Receiver<RawInputEvent>,
    scene_swapchain: Arc<SceneSwapchain>,
) -> std::thread::JoinHandle<()>
where
    F: FnOnce() -> (WidgetTree, NodeId) + Send + 'static,
{
    std::thread::spawn(move || {
        run_ui_loop(config, builder, input_rx, scene_swapchain);
    })
}

/// Spawn the dedicated UI loop with ordered native cursor feedback.
///
/// The host must drain the returned output on its native event-loop thread,
/// independently of rendering, and restore it before window destruction.
/// Existing scene and input channel contracts are unchanged.
pub fn spawn_ui_thread_with_cursor_output<F>(
    config: UiThreadConfig,
    builder: F,
    input_rx: Receiver<RawInputEvent>,
    scene_swapchain: Arc<SceneSwapchain>,
) -> (std::thread::JoinHandle<()>, UiThreadCursorOutput)
where
    F: FnOnce() -> (WidgetTree, NodeId) + Send + 'static,
{
    let (sender, output) = cursor_output::channel();
    let thread = std::thread::spawn(move || {
        run_ui_loop_with_cursor_output(config, builder, input_rx, scene_swapchain, Some(sender));
    });
    (thread, output)
}

/// Apply layout-produced focus/capture requests through the existing interaction owner.
/// The immutable appearance publication observes them on the next frame.
pub(crate) fn apply_deferred_focus_requests(
    widget_tree: &mut WidgetTree,
    event_router: &mut EventRouter,
    domain_state: &mut crate::core::event::InteractionDomainState,
    text_engine: &mut dyn TextEngine,
) {
    for focus_node in widget_tree.take_deferred_focus_requests() {
        let Some(widget_id) = widget_tree.widget_id_for_node(focus_node) else {
            continue;
        };
        event_router.request_focus_with_text_engine(
            widget_tree,
            widget_id,
            domain_state,
            Some(&mut *text_engine),
        );
        let is_capture_eligible = widget_tree
            .node(focus_node)
            .is_some_and(|node| node.widget.capture_eligible());
        if is_capture_eligible {
            event_router.enter_capture_with_text_engine(
                widget_tree,
                widget_id,
                focus_node,
                domain_state,
                Some(&mut *text_engine),
            );
        }
    }
}

pub(crate) fn handle_window_focus_changed<'a>(
    interaction_domains: &'a mut InteractionDomains,
    event_router: &mut EventRouter,
    widget_tree: &mut WidgetTree,
    window_id: WindowId,
    focused: bool,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
) -> Option<&'a mut crate::core::event::InteractionDomainState> {
    if !focused {
        if let Some(state) = interaction_domains.resolve_window_mut(window_id) {
            // Preserve lifecycle delivery before domain teardown erases its owner.
            if let Some(owner) = state.focus_manager.focused() {
                state.focus_manager.remember_focus(owner);
            }
            event_router.cancel_all_pointer_sessions_with_tree(
                widget_tree,
                crate::core::event::CancellationReason::SystemCancel,
                state,
            );
            if state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture {
                event_router.interrupt_physical_keyboard_capture(
                    widget_tree,
                    state,
                    crate::core::event::KeyboardCaptureInterruption {
                        reason: crate::core::event::KeyboardCaptureInterruptionReason::FocusLost,
                        key: None,
                    },
                    text_engine,
                );
            }
            if state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture {
                state.keyboard_mode = crate::core::event::KeyboardAuthorityMode::Navigation;
                event_router.notify_capture_leave(widget_tree, state, text_engine);
            }
            event_router.clear_focus(widget_tree, state);
        }
    }
    interaction_domains.handle_focus_changed(window_id, focused)
}

fn dispatch_engine_action(
    event: &FusedInputEvent,
    binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
    event_router: &mut EventRouter,
    widget_tree: &mut WidgetTree,
    active_root: NodeId,
    domain_state: &mut crate::core::event::InteractionDomainState,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
    animation: Option<crate::core::animation::AnimationContext<'_>>,
) -> bool {
    use crate::core::event::KeyboardAuthorityMode;
    crate::core::widget_availability::observe_keyboard_boundary(widget_tree, domain_state, event);
    // KeyDown: control keys (arrows, Tab, Enter, Escape, Space, F-keys)
    if let FusedInputEvent::KeyDown {
        time,
        key_code,
        physical_key,
        modifiers,
        repeat,
        ..
    } = event
    {
        let consumed = match domain_state.keyboard_mode {
            KeyboardAuthorityMode::Capture => {
                // SPEC-HAL-328 §3.2 / INV-KBD-MODE-4: Escape unconditionally
                // releases capture. No widget consultation. No veto.
                if *key_code == crate::core::event::KeyCode::Escape {
                    event_router.handle_engine_action(
                        widget_tree,
                        crate::runtime::actions::ActionId::RELEASE_CAPTURE,
                        Some(active_root),
                        domain_state,
                        text_engine,
                        animation,
                    )
                } else if modifiers.ctrl || modifiers.alt || modifiers.meta {
                    // SPEC-HAL-328 §2.4: Modifier combos fall through so
                    // route_input_event can offer them to the widget first
                    // and then to the App-scope binding registry.
                    false
                } else {
                    // Plain keys (and Tab/Shift+Tab) fall through to
                    // route_input_event for widget-first dispatch (§3.7).
                    false
                }
            }
            KeyboardAuthorityMode::Navigation => {
                let action_id = binding_registry.lookup_key(*key_code, *physical_key, *modifiers);
                if domain_state.ui_interaction_policy()
                    == crate::core::event::UiInteractionPolicy::PointerDriven
                    && action_id.is_some_and(|id| {
                        matches!(
                            id,
                            crate::runtime::actions::ActionId::FOCUS_NEXT
                                | crate::runtime::actions::ActionId::FOCUS_PREV
                                | crate::runtime::actions::ActionId::FOCUS_LEFT
                                | crate::runtime::actions::ActionId::FOCUS_RIGHT
                                | crate::runtime::actions::ActionId::FOCUS_UP
                                | crate::runtime::actions::ActionId::FOCUS_DOWN
                                | crate::runtime::actions::ActionId::ACTIVATE
                                | crate::runtime::actions::ActionId::ENTER_WIDGET
                        )
                    })
                {
                    return false;
                }
                match action_id {
                    // INV-112-K3: a held key cannot arm a newly focused Button.
                    Some(id)
                        if *repeat
                            && matches!(
                                id,
                                crate::runtime::actions::ActionId::ACTIVATE
                                    | crate::runtime::actions::ActionId::ENTER_WIDGET
                            )
                            && domain_state
                                .focus_manager
                                .focused()
                                .and_then(|owner| widget_tree.find_node_by_widget_id(owner))
                                .and_then(|node| widget_tree.node(node))
                                .is_some_and(|node| {
                                    node.widget.as_any().is::<crate::widgets::button::Button>()
                                }) =>
                    {
                        true
                    }
                    // SPEC-HAL-328 §3.7: Spatial navigation (FOCUS_LEFT/
                    // RIGHT/UP/DOWN) is routed widget-first via
                    // route_input_event so widgets can override arrow
                    // semantics (Slider value steps, RadioGroup member
                    // selection, etc.). Non-spatial engine actions
                    // (ACTIVATE, ENTER_WIDGET, ESCAPE, etc.) consume
                    // here.
                    Some(id) if id.is_engine_action() && !id.is_spatial_navigation() => {
                        use crate::runtime::actions::ActionId;

                        let activating = !*repeat
                            && matches!(id, ActionId::ACTIVATE | ActionId::ENTER_WIDGET);
                        let activation_mount = if activating {
                            domain_state.focus_manager.focused_mount(widget_tree)
                        } else {
                            None
                        };
                        let handled = event_router.handle_engine_action(
                            widget_tree,
                            id,
                            Some(active_root),
                            domain_state,
                            text_engine,
                            animation,
                        );
                        if handled {
                            let mount = if matches!(id, ActionId::FOCUS_NEXT | ActionId::FOCUS_PREV) {
                                domain_state.focus_manager.focused_mount(widget_tree)
                            } else {
                                activation_mount
                            };
                            event_router.note_keyboard_focus_cue(mount, domain_state);
                        }
                        handled
                    }
                    _ => false,
                }
            }
        };

        // SPEC-HAL-176: When this Key was consumed at engine level (mode
        // switch, ENTER_WIDGET, ACTIVATE, EXIT_SCOPE, Escape-release-capture,
        // etc.), arm the paired-TextInput suppression. The platform layer
        // (window.rs convert_window_event) timestamps Key and TextInput from
        // a single physical press with the same `Instant`, so a time match
        // on the next TextInput identifies the paired residue.
        if consumed {
            domain_state.swallow_text_at = Some(*time);
        }

        return consumed;
    }

    // SPEC-HAL-176: TextInput is dispatched to focused input controls
    // (TextField, TextArea) for typing. When the paired KeyDown of this
    // press was consumed at engine level above, suppress the residue here
    // so that e.g. pressing `i` in Navigation mode performs the mode
    // switch without also typing `i` into the focused TextField. Match
    // by timestamp — paired Key and TextInput share the same `Instant`
    // assigned by the platform layer.
    if let FusedInputEvent::TextInput { time, .. } = event {
        let armed = domain_state.swallow_text_at;
        // Clear the flag on every TextInput, whether matched or not — the
        // paired TextInput always arrives immediately after its Key, so a
        // non-matching TextInput means the armed press never had a paired
        // TextInput (e.g. Escape) and the flag is stale.
        domain_state.swallow_text_at = None;
        if armed == Some(*time) {
            return true;
        }
    }

    false
}

/// SPEC-HAL-044/045: Run the UI thread loop (called from within the UI thread).
///
/// This is an infinite loop that:
/// 1. Drains all available `RawInputEvent` from `input_rx`
/// 2. Determines `FrameReason` based on input
/// 3. Processes frame timing via ticker
/// 4. Processes input fusion and routes events
/// 5. Builds scene if not throttled
/// 6. Publishes `SceneFrame` to triple-buffer swapchain (SPEC-HAL-045)
///
/// The loop exits when the input channel is disconnected.
fn cancel_relative_overlay_takeover(
    app_root: &AppRoot,
    tree: &mut WidgetTree,
    event_router: &mut EventRouter,
    domain_state: &mut crate::core::event::InteractionDomainState,
) {
    let Some(binding) = domain_state
        .relative_pointer_authority
        .as_ref()
        .and_then(|authority| authority.active())
        .map(|activation| activation.binding())
    else {
        return;
    };
    if app_root.overlay_root.is_empty() {
        return;
    }
    let owner_in_modal = app_root
        .active_modal_input_root_node()
        .is_some_and(|modal_root| tree.is_descendant_of_node(binding.capture_owner(), modal_root));
    if !owner_in_modal {
        event_router.cancel_active_relative_with_tree(
            tree,
            domain_state,
            RelativePointerCancellationReason::CaptureLost,
            None,
        );
    }
}

mod ui_loop;
pub use self::ui_loop::run_ui_loop;
use self::ui_loop::run_ui_loop_with_cursor_output;

// ============================================================================
// SPEC-HAL-018: App Framework - Stage 1
// Screens, Navigation Stack, Modal Stack, App Context, Global Shortcuts
// ============================================================================

/// SPEC-HAL-018: Application-level events.
///
/// Stage 1: only a simple custom marker; extended by later SPECs when
/// concrete app-level events are introduced.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Custom named event for extensibility.
    Custom(&'static str),
}

/// SPEC-HAL-018 Stage 9: Navigation commands queued by screens/modals.
///
/// Screens request navigation via ScreenContext helpers (nav_push_screen, etc.).
/// Commands are queued in AppContext and processed by AppRoot::process_nav_queue()
/// once per frame, after input handling but before rebuild_active_view.
///
/// This decouples navigation requests from immediate execution, allowing
/// screens to request navigation without directly touching ScreenStack/ModalStack.
pub enum NavCommand {
    PushScreen(Box<dyn Screen>),
    PopScreen,
    ReplaceScreen(Box<dyn Screen>),
    ClearScreens,
    OpenModal(Box<dyn Screen>),
    CloseModal,
    CloseAllModals,
}

#[derive(Debug, Clone)]
pub struct ChildWindowRequest {
    pub id: WindowId,
    pub parent: WindowId,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub initial_value: String,
}

#[derive(Debug, Clone)]
pub struct ChildWindowSceneState {
    pub window_id: WindowId,
    pub parent_window_id: WindowId,
    pub title: String,
    pub bounds: RectF,
    pub edit_buffer: String,
    pub hovered_action: Option<ChildWindowAction>,
    pub armed_action: Option<ChildWindowAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildWindowAction {
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
pub struct ChildWindowActionRegion {
    pub action: ChildWindowAction,
    pub rect: RectF,
}

pub struct ChildWindowPaintContext<'a> {
    pub theme: &'a Theme,
    pub text_engine: &'a mut dyn TextEngine,
    pub scene: &'a mut dyn crate::core::paint::PaintSink,
    pub action_regions: &'a mut Vec<ChildWindowActionRegion>,
}

/// HAL373: a send-only notification for the canonical native App event loop.
///
/// Publish producer-owned completion state before calling [`Self::wake`]. The
/// notification carries no payload and grants no UI, source or resource access.
/// Clones share one native proxy; they do not create additional platform senders.
///
/// Only native `App::run` issues this capability:
/// ```compile_fail
/// use halcyon::core::app::UiWakeHandle;
/// let forged = UiWakeHandle {};
/// ```
#[derive(Clone, Debug)]
pub struct UiWakeHandle {
    proxy: Arc<winit::event_loop::EventLoopProxy<()>>,
}

/// The native loop no longer accepts wake notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UiWakeError {
    #[error("the native UI event loop is closed")]
    EventLoopClosed,
}

impl UiWakeHandle {
    /// Queue the existing unit `UserEvent(())` on this App's native event loop.
    ///
    /// `Ok(())` reports enqueueing only, not a frame, result installation or
    /// presentation. Hidden-window retention and redraw stay with `App::run`;
    /// consumers retain responsibility for mount/source eligibility on UI.
    pub fn wake(&self) -> Result<(), UiWakeError> {
        self.proxy
            .send_event(())
            .map_err(|_| UiWakeError::EventLoopClosed)
    }
}

/// SPEC-HAL-018: Global application context.
///
/// In the full SPEC this also holds settings, workspace state, data bus,
/// trading API, session info, etc. Stage 1 only models `theme` so it can
/// be wired to existing engine code without pulling in placeholder types.
///
/// Stage 9: Added nav_queue for deferred navigation commands.
/// SPEC-HAL-020 Stage 1: Added registry for action system (placeholder, unused yet).
pub struct AppContext {
    pub theme: Theme,
    // INV-354-OUTLINE-OWNER: presentation belongs only to this App lifetime.
    debug_outlines: bool,
    developer_shortcuts_enabled: bool,
    ui_waker: Option<UiWakeHandle>,
    ui_scale: UiScaleState,
    ui_scale_runtime: bool,
    /// SPEC-HAL-018 Stage 9: Queue of navigation commands to be processed by AppRoot.
    nav_queue: Vec<NavCommand>,
    #[cfg(target_arch = "wasm32")]
    web_navigation: Option<web_navigation::WebNavigationBinding>,
    /// SPEC-HAL-020 Stage 1: Action registry (placeholder, not yet used in event routing).
    pub registry: ActionRegistry,
    /// SPEC-HAL-031: Reactive signals for targeted widget invalidation.
    pub signals: SignalStore,
    /// SPEC-HAL-161: Main-loop-owned bus for cross-window/domain signal delivery.
    pub signal_bus: crate::core::signal_bus::SignalBus,
    /// SPEC-HAL-269: Shared toast manager for non-modal notifications.
    pub toast_manager: Rc<RefCell<ToastManager>>,
    /// SPEC-HAL-270: Persistent notification log.
    pub notification_manager: Rc<RefCell<NotificationManager>>,
    /// SPEC-HAL-333 INV-G1: application-supplied menu model. None = no app menu.
    menu_model: Option<crate::model::menu_model::MenuModel>,
    /// SPEC-HAL-333 INV-G1: cleared by set_menu_model; AppRoot installs once.
    menu_model_installed: bool,
    /// Exit request flag - when true, App::run will exit the event loop.
    exit_requested: bool,
    /// Theme changed flag - when true, engine syncs theme and marks tree dirty.
    theme_changed: bool,
    /// Screen invalidation requested during handle_input/flush (deferred because entry is popped).
    screen_invalidate_requested: bool,
    /// Runtime child OS windows requested by screens. App::run owns creation.
    pending_child_windows: Vec<ChildWindowRequest>,
    /// Monotonic runtime window ID allocator. WindowId::PRIMARY remains 0.
    next_runtime_window_id: u64,
    /// SPEC-HAL-295: GPU resources for widgets that perform custom rendering.
    /// None until the renderer is created (populated by App::run).
    gpu_resources: Option<crate::renderer::wgpu_renderer::GpuResources>,
    frame_realization_observer: Option<AppFrameObserver>,
    // INV-354-G1-RESULT-OWNER: only the application owns link/result reservations.
    flat_viewport3d_transfers: Option<Box<crate::three_d::transfer::FlatViewport3DTransfers>>,
    flat_viewport3d_transfer_admission_error: Option<crate::three_d::FlatViewport3DTransferError>,
}

impl Drop for AppContext {
    fn drop(&mut self) {
        if let Some(owner) = self.flat_viewport3d_transfers.as_deref_mut() {
            owner.shutdown();
        }
    }
}

impl AppContext {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            debug_outlines: false,
            developer_shortcuts_enabled: true,
            ui_waker: None,
            ui_scale: UiScaleState::initial(UiScale::ONE),
            ui_scale_runtime: false,
            nav_queue: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            web_navigation: None,
            registry: ActionRegistry::new(),
            signals: SignalStore::new(),
            signal_bus: crate::core::signal_bus::SignalBus::new(),
            toast_manager: Rc::new(RefCell::new(ToastManager::new())),
            notification_manager: Rc::new(RefCell::new(NotificationManager::new())),
            menu_model: None,
            menu_model_installed: false,
            exit_requested: false,
            theme_changed: false,
            screen_invalidate_requested: false,
            pending_child_windows: Vec::new(),
            next_runtime_window_id: 1,
            gpu_resources: None,
            frame_realization_observer: None,
            flat_viewport3d_transfers: None,
            flat_viewport3d_transfer_admission_error: None,
        }
    }

    fn from_config(config: &AppConfig) -> Self {
        let mut context = Self::new(config.theme.clone());
        context.developer_shortcuts_enabled = config.developer_shortcuts_enabled;
        context
    }

    /// Set this App's widget outlines (HAL354 §10). Equal values are a no-op.
    /// The next ordinary frame snapshots the value and invalidates paint only.
    pub fn set_debug_outlines_enabled(&mut self, enabled: bool) {
        if self.debug_outlines == enabled {
            return;
        }
        self.debug_outlines = enabled;
        if let Some(wake) = &self.ui_waker {
            if let Err(error) = wake.wake() {
                tracing::debug!(%error, "outline presentation changed after native loop closed");
            }
        }
    }

    /// Read this App's owned outline presentation value (HAL354 §10).
    pub fn debug_outlines_enabled(&self) -> bool {
        self.debug_outlines
    }

    /// The canonical native App's wake-only capability, when one is attached.
    /// Manually constructed contexts and AppRoots have no native sender.
    pub fn ui_waker(&self) -> Option<UiWakeHandle> {
        self.ui_waker.clone()
    }

    /// Explicit finite G1 admission for a manually constructed AppRoot.
    /// Standard App::run uses the equivalent AppConfig builder. HAL-354 G1
    /// reservations have no numeric default and are disabled in AppContext::new.
    pub fn new_with_flat_viewport3d_transfer_capacity(
        theme: Theme,
        capacity: std::num::NonZeroUsize,
    ) -> Result<Self, crate::three_d::FlatViewport3DTransferError> {
        let owner = crate::three_d::transfer::FlatViewport3DTransfers::new(capacity)?;
        let mut app = Self::new(theme);
        app.flat_viewport3d_transfers = Some(Box::new(owner));
        Ok(app)
    }

    fn flat_viewport3d_transfer_owner(
        &mut self,
    ) -> Result<
        &mut crate::three_d::transfer::FlatViewport3DTransfers,
        crate::three_d::FlatViewport3DTransferError,
    > {
        if let Some(error) = &self.flat_viewport3d_transfer_admission_error {
            return Err(error.clone());
        }
        self.flat_viewport3d_transfers
            .as_deref_mut()
            .ok_or(crate::three_d::FlatViewport3DTransferError::Disabled)
    }

    /// Declare one exact separately mounted producer and ordinary flat recipient.
    /// HAL-354 G1 admits only the current concrete mounts, one writer and one
    /// reserved application-owned result slot; this operation borrows no peer
    /// into either Widget and transfers no semantic authority.
    pub fn admit_flat_viewport3d_link(
        &mut self,
        tree: &mut WidgetTree,
        source: TreeWidgetId,
        target: TreeWidgetId,
    ) -> Result<crate::three_d::FlatViewport3DLinkId, crate::three_d::FlatViewport3DTransferError>
    {
        self.flat_viewport3d_transfer_owner()?
            .admit(tree, source, target)
    }

    /// Retire the exact link without erasing an unconsumed terminal receipt.
    /// A successor mount requires a new explicit admission.
    pub fn retire_flat_viewport3d_link(
        &mut self,
        link: &crate::three_d::FlatViewport3DLinkId,
    ) -> Result<(), crate::three_d::FlatViewport3DTransferError> {
        self.flat_viewport3d_transfer_owner()?.retire(link)
    }

    /// Clone an owned G1 mechanical result without consuming it.
    /// The application retains this source-qualified receipt after both Widgets
    /// and the originating Screen disappear. Prepared renderer status is a
    /// separate historical observation through ordinary scoped tree access.
    pub fn flat_viewport3d_link_result(
        &mut self,
        link: &crate::three_d::FlatViewport3DLinkId,
    ) -> Result<
        Option<crate::three_d::FlatViewport3DLinkResult>,
        crate::three_d::FlatViewport3DTransferError,
    > {
        self.flat_viewport3d_transfer_owner()?.result(link)
    }

    /// Consume one terminal G1 receipt, reopening a live link or releasing a
    /// retired reservation. A pending AcceptedForCutoff candidate is not consumed.
    pub fn take_flat_viewport3d_link_result(
        &mut self,
        link: &crate::three_d::FlatViewport3DLinkId,
    ) -> Result<
        Option<crate::three_d::FlatViewport3DLinkResult>,
        crate::three_d::FlatViewport3DTransferError,
    > {
        self.flat_viewport3d_transfer_owner()?.take_result(link)
    }

    /// Read the latest completed canonical App attempt from an ordinary callback.
    /// None unless configured with `AppConfig::with_frame_realization_observer`.
    pub fn frame_realization_observer(&self) -> Option<&AppFrameObserver> {
        self.frame_realization_observer.as_ref()
    }

    /// SPEC-HAL-024 E.3: queue scale for the post-input App checkpoint.
    /// Non-default requests require canonical `App::run`; dedicated UI threads stay at 1.
    pub fn request_ui_scale(&mut self, scale: UiScale) -> Result<UiScaleReceipt, UiScaleRejection> {
        if !self.ui_scale_runtime && scale != UiScale::ONE {
            return Err(UiScaleRejection::UnsupportedRuntime);
        }
        self.ui_scale.request(scale)
    }

    /// SPEC-HAL-024 E.3: read requested, admitted and selected target scale state.
    /// `Submitted` reports submission, without promising synchronized scanout.
    pub fn ui_scale_status(&self, surface: Option<SurfaceId>) -> UiScaleStatus {
        self.ui_scale.status(surface)
    }

    /// Set a new theme at runtime. Triggers full tree rebuild next frame.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.theme_changed = true;
    }

    /// Check and clear the theme-changed flag.
    pub fn take_theme_changed(&mut self) -> bool {
        std::mem::take(&mut self.theme_changed)
    }

    /// SPEC-HAL-295: Get GPU resources for custom widget rendering.
    ///
    /// Returns None if the renderer hasn't been created yet (headless mode).
    pub fn gpu_resources(&self) -> Option<&crate::renderer::wgpu_renderer::GpuResources> {
        self.gpu_resources.as_ref()
    }

    /// SPEC-HAL-295: Set GPU resources (called by App::run after renderer creation).
    pub fn set_gpu_resources(&mut self, gpu: crate::renderer::wgpu_renderer::GpuResources) {
        self.gpu_resources = Some(gpu);
    }

    /// Request screen rebuild. Works even when screen entry is popped.
    pub fn request_invalidate(&mut self) {
        self.screen_invalidate_requested = true;
    }

    /// Check and clear the invalidate flag.
    pub fn take_invalidate_requested(&mut self) -> bool {
        std::mem::take(&mut self.screen_invalidate_requested)
    }

    pub fn request_child_window(
        &mut self,
        parent: WindowId,
        title: impl Into<String>,
        width: u32,
        height: u32,
        initial_value: impl Into<String>,
    ) -> WindowId {
        let id = WindowId(self.next_runtime_window_id);
        self.next_runtime_window_id += 1;
        self.pending_child_windows.push(ChildWindowRequest {
            id,
            parent,
            title: title.into(),
            width,
            height,
            initial_value: initial_value.into(),
        });
        id
    }

    pub fn take_child_window_requests(&mut self) -> Vec<ChildWindowRequest> {
        std::mem::take(&mut self.pending_child_windows)
    }

    /// SPEC-HAL-018 Stage 9: Queue a navigation command for processing.
    ///
    /// Called by ScreenContext helpers (nav_push_screen, nav_pop_screen, etc.).
    /// Commands are processed by AppRoot::process_nav_queue() once per frame.
    pub fn queue_nav(&mut self, cmd: NavCommand) {
        self.nav_queue.push(cmd);
    }

    /// SPEC-HAL-018 Stage 9: Take all queued navigation commands.
    ///
    /// Called by AppRoot::process_nav_queue() to drain the queue.
    pub fn take_nav_queue(&mut self) -> Vec<NavCommand> {
        std::mem::take(&mut self.nav_queue)
    }

    /// Request application exit.
    ///
    /// Sets a flag that App::run checks each frame. When true, the event loop
    /// will exit cleanly. Use this from Screen::handle_input for "Escape to exit"
    /// or similar functionality.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    /// Check if exit has been requested.
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// SPEC-HAL-269: Push a toast notification.
    pub fn push_toast(
        &mut self,
        message: impl Into<String>,
        duration_ms: u64,
        action: Option<ToastAction>,
    ) -> ToastHandle {
        self.toast_manager
            .borrow_mut()
            .push(message, duration_ms, action)
    }

    /// SPEC-HAL-270: Push a persistent notification to the log.
    pub fn push_notification(
        &mut self,
        level: NotificationLevel,
        message: impl Into<String>,
        title: Option<impl Into<String>>,
        action: Option<NotificationAction>,
        timestamp_ms: u64,
    ) -> NotificationHandle {
        self.notification_manager
            .borrow_mut()
            .push(level, message, title, action, timestamp_ms)
    }

    /// SPEC-HAL-333 INV-G1: supply or replace the application menu model.
    /// AppRoot installs it on the active host on the next frame.
    pub fn set_menu_model(&mut self, model: crate::model::menu_model::MenuModel) {
        self.menu_model = Some(model);
        self.menu_model_installed = false;
    }

    /// SPEC-HAL-333 INV-A5: runtime mutation access to the menu model.
    /// Dirty item ids accumulate in the model and are mirrored to the host
    /// each frame by AppRoot.
    pub fn menu_model_mut(&mut self) -> Option<&mut crate::model::menu_model::MenuModel> {
        self.menu_model.as_mut()
    }
}

fn trace_native_layout_transition_members(
    phase: &'static str,
    attempt_sequence: u64,
    frame_number: u64,
    tree: &WidgetTree,
    active: &ActiveLayoutTransitions,
    layer: &TransitionLayer,
    painted_rects: Option<&InterpolatedVisualRects>,
) {
    if !tracing::enabled!(target: "halcyon::native_layout_transition_pacer", tracing::Level::TRACE) {
        return;
    }
    for (owner, state) in active.iter() {
        use crate::core::animation::AnimationSample;
        let (status, progress) = match layer.timeline().sample::<f32>(state.handle) {
            AnimationSample::Active(value) => ("Active", value),
            AnimationSample::Completed(value) => ("Completed", value),
            AnimationSample::Cancelled => ("Cancelled", 0.0),
            AnimationSample::Missing => ("Missing", 0.0),
            AnimationSample::TypeMismatch => ("TypeMismatch", 0.0),
        };
        let current = tree.layout_transition_store().get_current(*owner);
        let canonical = tree.find_by_id(*owner)
            .and_then(|node| tree.get_absolute_paint_rect(node));
        let painted = painted_rects.and_then(|rects| rects.get(owner).copied().or(canonical));
        let finite = [state.from.x, state.from.y, state.from.width, state.from.height,
            state.to.x, state.to.y, state.to.width, state.to.height]
            .iter().all(|value| value.is_finite());
        tracing::trace!(target: "halcyon::native_layout_transition_pacer",
            event = "member", phase, attempt_sequence, frame_number,
            owner = owner.0, handle = state.handle.id.0, status, progress = f64::from(progress),
            from_x = f64::from(state.from.x), from_y = f64::from(state.from.y),
            from_w = f64::from(state.from.width), from_h = f64::from(state.from.height),
            to_x = f64::from(state.to.x), to_y = f64::from(state.to.y),
            to_w = f64::from(state.to.width), to_h = f64::from(state.to.height),
            finite, current_available = current.is_some(), current_matches = current == Some(state.to),
            canonical_matches = canonical == Some(state.to),
            paint_rect_available = painted.is_some(),
            projected = painted_rects.is_some_and(|rects| rects.contains_key(owner)),
            paint_x = ?painted.map(|rect| rect.x), paint_y = ?painted.map(|rect| rect.y),
            paint_w = ?painted.map(|rect| rect.width), paint_h = ?painted.map(|rect| rect.height));
    }
}

#[cfg(test)]
#[path = "../tests/app/ui_wake_tests.rs"]
mod ui_wake_tests;

/// SPEC-HAL-018: Per-screen context.
///
/// Stage 1: gives access to app context, screen stack, and modal stack.
/// Global shortcuts are *not* exposed through `ScreenContext` yet to avoid
/// the self-borrow pattern that made the first attempt unworkable.
/// Shortcuts are accessed through `AppRoot` directly.
///
/// Stage 44: Now includes optional tree access for directive polling in handle_input.
struct CompositeEditorScreenResources<'a> {
    router: &'a mut EventRouter,
    domain: &'a mut crate::core::event::InteractionDomainState,
    text_engine: &'a mut dyn crate::core::text_engine::TextEngine,
    frame_time_ms: u64,
}

impl CompositeEditorScreenResources<'_> {
    fn reborrow(&mut self) -> CompositeEditorScreenResources<'_> {
        CompositeEditorScreenResources {
            router: &mut *self.router,
            domain: &mut *self.domain,
            text_engine: &mut *self.text_engine,
            frame_time_ms: self.frame_time_ms,
        }
    }
}

pub struct ScreenContext<'a> {
    pub app: &'a mut AppContext,
    pub screens: &'a mut ScreenStack,
    pub modals: &'a mut ModalStack,
    /// Stage 44: Widget tree access for directive polling.
    /// Only available in handle_input (None during lifecycle methods).
    pub tree: Option<&'a mut WidgetTree>,
    composite_editors: Option<CompositeEditorScreenResources<'a>>,
}

impl ScreenContext<'_> {
    /// Synchronous qualified operations during the engine's post-route flush.
    /// Other callbacks return WrongPhase; tree access is suspended in the closure.
    pub fn with_composite_editors<R>(
        &mut self,
        apply: impl FnOnce(&mut crate::core::composite_text_edit::CompositeEditorOwnerContext<'_>) -> R,
    ) -> Result<R, crate::core::composite_text_edit::CompositeEditOutcome> {
        use crate::core::composite_text_edit::{CompositeEditOutcome, CompositeEditorOwnerContext};
        let resources = self
            .composite_editors
            .as_mut()
            .ok_or(CompositeEditOutcome::WrongPhase)?;
        let tree = self.tree.take().ok_or(CompositeEditOutcome::Unavailable)?;
        let mut owner = CompositeEditorOwnerContext {
            tree: &mut *tree,
            router: &mut *resources.router,
            domain: &mut *resources.domain,
            text_engine: &mut *resources.text_engine,
            frame_time_ms: resources.frame_time_ms,
        };
        let result = apply(&mut owner);
        drop(owner);
        self.tree = Some(tree);
        Ok(result)
    }

    pub fn request_widget_enabled(
        &mut self,
        node: NodeId,
        enabled: bool,
    ) -> Result<(), crate::core::widget_tree::WidgetAvailabilityError> {
        self.tree
            .as_deref_mut()
            .ok_or(crate::core::widget_tree::WidgetAvailabilityError::TreeUnavailable)?
            .request_widget_enabled(node, enabled)
    }

    pub fn request_widget_read_only(
        &mut self,
        node: NodeId,
        read_only: bool,
    ) -> Result<(), crate::core::widget_tree::WidgetAvailabilityError> {
        self.tree
            .as_deref_mut()
            .ok_or(crate::core::widget_tree::WidgetAvailabilityError::TreeUnavailable)?
            .request_widget_read_only(node, read_only)
    }
}

// SPEC-HAL-018 Stage 9: Navigation helper methods for ScreenContext
impl<'a> ScreenContext<'a> {
    /// Construct the ordinary screen context from its application-owned borrows.
    /// This does not grant the engine's post-route composite editor capability.
    pub fn new(
        app: &'a mut AppContext,
        screens: &'a mut ScreenStack,
        modals: &'a mut ModalStack,
        tree: Option<&'a mut WidgetTree>,
    ) -> Self {
        Self {
            app,
            screens,
            modals,
            tree,
            composite_editors: None,
        }
    }
    /// Queue a push_screen navigation command.
    ///
    /// The screen will be pushed onto the stack by AppRoot::process_nav_queue()
    /// after input handling completes. Lifecycle (on_leave/on_enter) is handled
    /// by AppRoot, not by the caller.
    pub fn nav_push_screen(&mut self, screen: Box<dyn Screen>) {
        self.app.queue_nav(NavCommand::PushScreen(screen));
    }

    /// Queue a pop_screen navigation command.
    ///
    /// The top screen will be popped by AppRoot::process_nav_queue().
    pub fn nav_pop_screen(&mut self) {
        self.app.queue_nav(NavCommand::PopScreen);
    }

    /// Queue a replace_screen navigation command.
    ///
    /// The top screen will be replaced by AppRoot::process_nav_queue().
    pub fn nav_replace_screen(&mut self, screen: Box<dyn Screen>) {
        self.app.queue_nav(NavCommand::ReplaceScreen(screen));
    }

    /// Queue a clear_screens navigation command.
    ///
    /// All screens will be cleared by AppRoot::process_nav_queue().
    pub fn nav_clear_screens(&mut self) {
        self.app.queue_nav(NavCommand::ClearScreens);
    }

    /// Queue an open_modal navigation command.
    ///
    /// The modal will be opened by AppRoot::process_nav_queue().
    pub fn nav_open_modal(&mut self, modal: Box<dyn Screen>) {
        self.app.queue_nav(NavCommand::OpenModal(modal));
    }

    /// Queue a close_modal navigation command.
    ///
    /// The top modal will be closed by AppRoot::process_nav_queue().
    pub fn nav_close_modal(&mut self) {
        self.app.queue_nav(NavCommand::CloseModal);
    }

    /// Queue a close_all_modals navigation command.
    ///
    /// All modals will be closed by AppRoot::process_nav_queue().
    pub fn nav_close_all_modals(&mut self) {
        self.app.queue_nav(NavCommand::CloseAllModals);
    }

    /// SPEC-HAL-018 Stage 12: Request a rebuild of the current screen.
    ///
    /// Call this from handle_input when screen state changes require a rebuild.
    /// The rebuild will happen on the next frame in rebuild_active_view.
    pub fn request_screen_rebuild(&mut self) {
        self.app.request_invalidate();
        self.screens.mark_top_rebuild();
    }

    /// SPEC-HAL-018 Stage 12B: Alias for request_screen_rebuild().
    ///
    /// Shorthand for requesting a rebuild of the current screen.
    /// Equivalent to `request_screen_rebuild()`.
    pub fn invalidate(&mut self) {
        self.request_screen_rebuild();
    }

    /// SPEC-HAL-018 Stage 12: Request a rebuild of the top modal.
    ///
    /// Call this from handle_input when modal state changes require a rebuild.
    /// The rebuild will happen on the next frame in rebuild_active_view.
    pub fn request_modal_rebuild(&mut self) {
        self.modals.mark_top_rebuild();
    }

    /// Request application exit.
    ///
    /// Call this from handle_input to exit the application (e.g., on Escape key).
    /// The exit will happen at the end of the current frame.
    pub fn request_exit(&mut self) {
        self.app.request_exit();
    }

    pub fn request_child_window(
        &mut self,
        title: impl Into<String>,
        width: u32,
        height: u32,
        initial_value: impl Into<String>,
    ) -> WindowId {
        self.app
            .request_child_window(WindowId::PRIMARY, title, width, height, initial_value)
    }

    /// Switch to a different theme at runtime.
    pub fn set_theme(&mut self, theme: Theme) {
        self.app.set_theme(theme);
    }

    /// Toggle between light and dark themes, preserving density and Button material policy.
    pub fn toggle_theme(&mut self) {
        let mut theme = self.app.theme.clone();
        theme.toggle();
        self.app.set_theme(theme);
    }

    /// SPEC-HAL-269: Push a toast notification.
    pub fn push_toast(
        &mut self,
        message: impl Into<String>,
        duration_ms: u64,
        action: Option<ToastAction>,
    ) -> ToastHandle {
        self.app.push_toast(message, duration_ms, action)
    }

    /// SPEC-HAL-270: Push a persistent notification to the log.
    pub fn push_notification(
        &mut self,
        level: NotificationLevel,
        message: impl Into<String>,
        title: Option<impl Into<String>>,
        action: Option<NotificationAction>,
        timestamp_ms: u64,
    ) -> NotificationHandle {
        self.app
            .push_notification(level, message, title, action, timestamp_ms)
    }

    /// SPEC-HAL-270: Read access to the notification log.
    pub fn notification_manager(&self) -> std::cell::Ref<'_, NotificationManager> {
        self.app.notification_manager.borrow()
    }
}

/// SPEC-HAL-018 Stage 4: Build-time context for Screen::build().
///
/// This context is used ONLY during the build phase (constructing widget trees).
/// It deliberately excludes screens/modals to avoid borrow conflicts.
/// Navigation must happen in event handlers, not during build.
///
/// Stage 10: Extended with tree access for widget mounting.
pub struct ScreenFrameContext<'a> {
    pub app: &'a mut AppContext,
    /// SPEC-HAL-018 Stage 10: Tree access for mounting widgets.
    pub tree: &'a mut WidgetTree,
    /// SPEC-HAL-018 Stage 10: Parent node to mount screen widgets under.
    pub parent_node: NodeId,
}

impl<'a> ScreenFrameContext<'a> {
    pub fn request_widget_enabled(
        &mut self,
        node: NodeId,
        enabled: bool,
    ) -> Result<(), crate::core::widget_tree::WidgetAvailabilityError> {
        self.tree.request_widget_enabled(node, enabled)
    }

    pub fn request_widget_read_only(
        &mut self,
        node: NodeId,
        read_only: bool,
    ) -> Result<(), crate::core::widget_tree::WidgetAvailabilityError> {
        self.tree.request_widget_read_only(node, read_only)
    }

    /// SPEC-HAL-018 Stage 10: Mount a widget as a child of the current parent node.
    ///
    /// Called by Screen::build() implementations to add their root widget
    /// (and its subtree) to the tree under ScreenHost.
    ///
    /// Returns the NodeId of the newly mounted widget.
    pub fn mount_child(&mut self, widget: Box<dyn crate::core::widget::Widget>) -> NodeId {
        self.tree.add_child(self.parent_node, widget, None, 0)
    }
}

/// SPEC-HAL-018: A full-screen UI state.
///
/// Each screen owns its own widget tree and exposes its root widget via
/// `build()`. Navigation and app-level logic go through `ScreenContext`.
///
/// Stage 4: `build()` uses `ScreenFrameContext` (build-time only, no navigation).
/// Lifecycle methods (`on_enter`, `on_leave`, `handle_app_event`) use full `ScreenContext`.
///
/// # Incremental Rebuild Pattern (Stage 12B/15)
///
/// The engine rebuilds a screen when **either**:
/// - The stack entry's `needs_rebuild` flag is set (via `ScreenContext::invalidate()`), **or**
/// - `screen.take_rebuild_flag()` returns true.
///
/// By default, `take_rebuild_flag()` returns true (always rebuild).
/// For screens that want incremental rebuild behavior, implement the pattern:
///
/// ```ignore
/// struct MyScreen {
///     // Screen state
///     counter: u32,
///     // Internal rebuild flag
///     needs_rebuild: bool,
/// }
///
/// impl MyScreen {
///     fn new() -> Self {
///         Self {
///             counter: 0,
///             needs_rebuild: true, // Initial build required
///         }
///     }
/// }
///
/// impl Screen for MyScreen {
///     fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
///         // Reset state on screen entry
///         self.counter = 0;
///         self.needs_rebuild = true;
///     }
///
///     fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
///         // When state changes, mark for rebuild and notify context
///         if some_state_change {
///             self.counter += 1;
///             self.needs_rebuild = true;
///             cx.invalidate(); // or cx.request_screen_rebuild()
///             return true;
///         }
///         false
///     }
///
///     fn mark_rebuild(&mut self) {
///         self.needs_rebuild = true;
///     }
///
///     fn take_rebuild_flag(&mut self) -> bool {
///         std::mem::take(&mut self.needs_rebuild)
///     }
///
///     fn build<'a>(&'a mut self, cx: &mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
///         // Build widgets based on current state
///     }
/// }
/// ```
pub trait Screen {
    /// Build and return the root widget for this screen.
    ///
    /// The returned reference must point into widget state owned by the
    /// screen itself.
    ///
    /// Stage 4: Uses `ScreenFrameContext` which only provides app context.
    /// Navigation (push/pop/replace) must NOT happen inside build().
    fn build<'a>(
        &'a mut self,
        cx: &'a mut ScreenFrameContext<'a>,
    ) -> &'a mut dyn crate::core::widget::Widget;

    /// Called when this screen becomes the active (top) screen.
    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    /// Called when this screen is no longer the active screen.
    fn on_leave(&mut self, _cx: &mut ScreenContext<'_>) {}

    /// Whether this screen's required initial content is ready for presentation.
    ///
    /// Called read-only after ordinary action settlement, before scene preparation.
    /// Pending producers must use their existing wake ingress. An error aborts
    /// startup; readiness alone does not certify rendering or presentation.
    fn startup_ready(&self) -> Result<bool, String> {
        Ok(true)
    }

    /// Handle application-level events not routed through the widget tree.
    fn handle_app_event(&mut self, _event: &AppEvent, _cx: &mut ScreenContext<'_>) {}

    /// Handle a lifecycle-approved cross-window/domain signal.
    ///
    /// Called in the Signal Delivery Phase after input routing and before
    /// layout/scene/paint. Return true if the signal mutated screen state or was
    /// otherwise consumed.
    fn handle_signal(
        &mut self,
        _signal: &crate::core::signal_bus::SignalEnvelope,
        _cx: &mut ScreenContext<'_>,
    ) -> bool {
        false
    }

    /// Paint a runtime child OS window scene without creating a second widget tree.
    ///
    /// SPEC-HAL-151 still constrains the retained widget tree to the primary
    /// window. This hook gives screens a bounded, surface-scoped render path for
    /// child windows while keeping layout, focus, and input authority unchanged.
    fn paint_child_window_scene(
        &self,
        _window: &ChildWindowSceneState,
        _cx: &mut ChildWindowPaintContext<'_>,
    ) -> bool {
        false
    }

    /// SPEC-HAL-018 Stage 7: Handle fused input events before widget tree routing.
    ///
    /// Called on modals (top-first) and base screen before global shortcuts.
    /// Return true if the event was fully consumed and should not propagate.
    ///
    /// Navigation via cx is allowed here (unlike build()).
    fn handle_input(
        &mut self,
        _event: &crate::core::event::FusedInputEvent,
        _cx: &mut ScreenContext<'_>,
    ) -> bool {
        false
    }

    /// Flush pending actions after widget tree routing, before build_scene.
    ///
    /// Called once per frame after all events have been routed through the widget tree.
    /// Use this to poll pending actions from widgets and apply them to models,
    /// ensuring same-frame application of discrete actions (e.g., auto-size).
    ///
    /// This runs after route_input_event but before build_scene, so any model
    /// changes made here will be reflected in the same frame's render.
    ///
    /// Do NOT call request_redraw() from this method.
    fn flush_pending_actions(&mut self, _cx: &mut ScreenContext<'_>) {}

    /// SPEC-HAL-018 Stage 8: Return true if this modal covers the given logical point.
    ///
    /// This is used for modal hit-test shielding. When a modal is open, pointer
    /// events that fall outside the modal's coverage area are consumed (blocked)
    /// rather than routed to widgets underneath.
    ///
    /// Default: full-screen coverage (all points covered).
    ///
    /// Override for:
    /// - Dialog boxes: return true only inside the dialog rect
    /// - Bottom sheets: return true only in the sheet area
    /// - Transparent overlays that should pass through clicks
    /// - Custom modal shapes
    ///
    /// Base screens should leave default behavior untouched.
    fn covers_point(&self, _logical_pos: Point) -> bool {
        true
    }

    /// SPEC-HAL-127 Stage 4: Return the dismiss policy for this modal.
    ///
    /// S4.INV1: OverlayRoot uses this to decide when to dismiss the modal.
    /// Modal code must not decide dismissal itself.
    ///
    /// Default: modal_default() - dismiss on Escape only, not outside click.
    /// Override for palettes/popups that should dismiss on outside click.
    fn dismiss_policy(&self) -> DismissPolicy {
        DismissPolicy::modal_default()
    }

    /// SPEC-HAL-018 Stage 12: Mark this screen for rebuild.
    ///
    /// Called when screen state changes require a rebuild. Override to track
    /// internal state for incremental rebuild behavior.
    fn mark_rebuild(&mut self) {}

    /// SPEC-HAL-018 Stage 12: Check and consume the rebuild flag.
    ///
    /// Default returns true (always rebuild). Screens that want incremental
    /// behavior should override to track and return their internal needs_rebuild state.
    fn take_rebuild_flag(&mut self) -> bool {
        true
    }

    /// Per-frame update hook with tree access.
    /// Runs every frame regardless of rebuild flag.
    /// Use for data-driven value updates on retained widgets.
    /// Do NOT modify tree structure here. Value updates on existing widgets
    /// and non-structural relative-pointer semantic-cancel intents are allowed.
    fn update(&mut self, _tree: &mut crate::core::widget_tree::WidgetTree) {}
}

/// SPEC-HAL-018 Stage 12: Wrapper for screens/modals with rebuild tracking.
///
/// Each entry tracks whether the screen needs to be rebuilt on the next frame.
/// This enables incremental rebuilds: only screens that changed are rebuilt.
pub(crate) struct ScreenEntry {
    screen: Box<dyn Screen>,
    needs_rebuild: bool,
    /// The NodeId of this screen's mounted root widget (if any).
    /// Used for per-screen conditional rebuild: we remove only this node
    /// when rebuilding, not all children of ScreenHost.
    mounted_node: Option<NodeId>,
}

impl ScreenEntry {
    fn new(screen: Box<dyn Screen>) -> Self {
        Self {
            screen,
            // New screens always need initial build
            needs_rebuild: true,
            // No mounted widget yet
            mounted_node: None,
        }
    }

    /// SPEC-HAL-127 Stage 3: Read-only covers_point check for shielding.
    ///
    /// S3.INV3: Delegates to Screen::covers_point without mutation.
    pub(crate) fn covers_point(&self, pos: Point) -> bool {
        self.screen.covers_point(pos)
    }
}

/// SPEC-HAL-018: Stack of screens for navigation.
///
/// Represents the primary "where am I" navigation stack: dashboards,
/// detail views, etc.
pub struct ScreenStack {
    stack: Vec<ScreenEntry>,
}

impl ScreenStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Get mutable reference to the current (top) screen.
    pub fn current_mut(&mut self) -> Option<&mut Box<dyn Screen>> {
        self.stack.last_mut().map(|e| &mut e.screen)
    }

    /// Push a new screen onto the stack.
    pub fn push(&mut self, screen: Box<dyn Screen>) {
        self.stack.push(ScreenEntry::new(screen));
    }

    /// Pop the top screen from the stack.
    pub fn pop(&mut self) -> Option<Box<dyn Screen>> {
        self.stack.pop().map(|e| e.screen)
    }

    /// Replace the top screen with a new one.
    pub fn replace(&mut self, screen: Box<dyn Screen>) -> Option<Box<dyn Screen>> {
        let old = self.stack.pop().map(|e| e.screen);
        self.stack.push(ScreenEntry::new(screen));
        old
    }

    /// Clear all screens, returning them in reverse order (top-to-bottom).
    ///
    /// Stage 6: Returns screens so AppRoot can call on_leave on each.
    pub fn drain_reverse(&mut self) -> Vec<Box<dyn Screen>> {
        let mut result: Vec<_> = self.stack.drain(..).map(|e| e.screen).collect();
        result.reverse();
        result
    }

    /// SPEC-HAL-018 Stage 12/15: Mark the top screen for rebuild.
    ///
    /// Called when screen state changes require a rebuild.
    /// Stage 15: Also calls screen.mark_rebuild() to sync internal flag.
    pub fn mark_top_rebuild(&mut self) {
        if let Some(entry) = self.stack.last_mut() {
            entry.needs_rebuild = true;
            entry.screen.mark_rebuild();
        }
    }

    /// SPEC-HAL-018 Stage 12: Pop the top entry (screen + rebuild flag).
    ///
    /// Used by rebuild_active_view for conditional rebuild logic.
    pub(crate) fn pop_entry(&mut self) -> Option<ScreenEntry> {
        self.stack.pop()
    }

    /// SPEC-HAL-018 Stage 12: Push an entry back (screen + rebuild flag).
    ///
    /// Used by rebuild_active_view after building.
    pub(crate) fn push_entry(&mut self, entry: ScreenEntry) {
        self.stack.push(entry);
    }
}

impl Default for ScreenStack {
    fn default() -> Self {
        Self::new()
    }
}

/// SPEC-HAL-018: Stack of modal overlays.
///
/// Stage 1 models modals as screens layered above the active screen,
/// which matches the conceptual SPEC design.
pub struct ModalStack {
    stack: Vec<ScreenEntry>,
}

impl ModalStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Get mutable reference to the top modal.
    pub fn top_mut(&mut self) -> Option<&mut Box<dyn Screen>> {
        self.stack.last_mut().map(|e| &mut e.screen)
    }

    /// SPEC-HAL-127 Stage 3: Read-only access to top entry for shielding checks.
    ///
    /// S3.INV3: This method allows shielding checks without ModalStack mutation.
    pub(crate) fn top_entry(&self) -> Option<&ScreenEntry> {
        self.stack.last()
    }

    /// Open a new modal on top of the stack (raw, no lifecycle).
    ///
    /// Note: AppRoot::open_modal wraps this with lifecycle calls.
    pub fn open(&mut self, modal: Box<dyn Screen>) {
        self.stack.push(ScreenEntry::new(modal));
    }

    /// Stage 7: Re-add a modal entry without lifecycle (for pop-modify-push pattern).
    ///
    /// Used internally when temporarily removing a modal to call methods on it
    /// without borrow conflicts, then putting it back.
    pub(crate) fn open_entry(&mut self, entry: ScreenEntry) {
        self.stack.push(entry);
    }

    /// Close the top modal. Returns the closed modal if any.
    ///
    /// Stage 6: Returns the modal so AppRoot can call on_leave.
    pub fn close_top(&mut self) -> Option<Box<dyn Screen>> {
        self.stack.pop().map(|e| e.screen)
    }

    /// SPEC-HAL-018 Stage 12: Pop the top entry (modal + rebuild flag).
    ///
    /// Used by rebuild_active_view for conditional rebuild logic.
    pub(crate) fn pop_entry(&mut self) -> Option<ScreenEntry> {
        self.stack.pop()
    }

    /// Close all modals, returning them in reverse order (top-to-bottom).
    ///
    /// Stage 6: Returns modals so AppRoot can call on_leave on each.
    pub fn drain_reverse(&mut self) -> Vec<Box<dyn Screen>> {
        let mut result: Vec<_> = self.stack.drain(..).map(|e| e.screen).collect();
        result.reverse();
        result
    }

    /// Iterate over all modals mutably (bottom-to-top order).
    ///
    /// Stage 4: Used by AppRoot::rebuild_active_view to call build() on each modal.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn Screen>> {
        self.stack.iter_mut().map(|e| &mut e.screen)
    }

    /// SPEC-HAL-018 Stage 12/15: Mark the top modal for rebuild.
    ///
    /// Called when modal state changes require a rebuild.
    /// Stage 15: Also calls screen.mark_rebuild() to sync internal flag.
    pub fn mark_top_rebuild(&mut self) {
        if let Some(entry) = self.stack.last_mut() {
            entry.needs_rebuild = true;
            entry.screen.mark_rebuild();
        }
    }

    /// SPEC-HAL-018 Stage 12: Drain all entries, returning them in reverse order.
    ///
    /// Used by close_all_modals to get mounted_nodes for cleanup.
    pub(crate) fn drain_entries_reverse(&mut self) -> Vec<ScreenEntry> {
        let mut result: Vec<_> = self.stack.drain(..).collect();
        result.reverse();
        result
    }

    /// SPEC-HAL-038 Stage 3: Get mounted nodes of all modals in stack order.
    ///
    /// Returns NodeIds in bottom-to-top order, suitable for focus scope sync.
    /// Only includes modals that have been mounted (have mounted_node set).
    pub fn mounted_nodes(&self) -> Vec<NodeId> {
        self.stack
            .iter()
            .filter_map(|entry| entry.mounted_node)
            .collect()
    }
}

impl Default for ModalStack {
    fn default() -> Self {
        Self::new()
    }
}

/// SPEC-HAL-018: Keyboard shortcut (key + modifier state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: crate::core::event::BindingKey,
    pub modifiers: crate::core::event::Modifiers,
}

impl Shortcut {
    pub fn new(key: crate::core::event::KeyCode, modifiers: crate::core::event::Modifiers) -> Self {
        Self {
            key: key.into(),
            modifiers,
        }
    }

    /// Convenience: shortcut with no modifiers.
    pub fn key(key: crate::core::event::KeyCode) -> Self {
        Self {
            key: key.into(),
            modifiers: crate::core::event::Modifiers::default(),
        }
    }
}

/// SPEC-HAL-018: Shortcut handler.
///
/// Uses a higher-ranked lifetime so a single handler can operate with any
/// temporary `ScreenContext<'_>` constructed by the caller.
pub type ShortcutHandler = Box<dyn for<'a> Fn(&mut ScreenContext<'a>) -> bool>;

/// SPEC-HAL-018: Global keyboard shortcuts registry.
///
/// Stage 1: simple linear scan, no grouping or categories.
pub struct GlobalShortcuts {
    bindings: Vec<(Shortcut, ShortcutHandler)>,
}

impl GlobalShortcuts {
    fn contains_key(&self, event: &crate::core::event::KeyEvent) -> bool {
        self.bindings.iter().any(|(shortcut, _)| {
            shortcut.key.matches(event.key_code, event.physical_key)
                && shortcut.modifiers == event.modifiers
        })
    }
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Bind a keyboard shortcut to a handler.
    ///
    /// Handler returns true if the shortcut was consumed.
    pub fn bind<F>(&mut self, shortcut: Shortcut, handler: F)
    where
        F: for<'a> Fn(&mut ScreenContext<'a>) -> bool + 'static,
    {
        self.bindings.push((shortcut, Box::new(handler)));
    }

    /// Try to handle a key event. Returns true if handled.
    ///
    /// Note: This is not yet wired into the engine loop; Stage 2 will
    /// integrate this with `FusedInputEvent` routing.
    pub fn try_handle(
        &self,
        key: crate::core::event::KeyCode,
        modifiers: crate::core::event::Modifiers,
        cx: &mut ScreenContext<'_>,
    ) -> bool {
        self.try_handle_key(
            key,
            crate::core::event::PhysicalKeyCode::Unknown,
            modifiers,
            cx,
        )
    }

    fn try_handle_key(
        &self,
        key: crate::core::event::KeyCode,
        physical: crate::core::event::PhysicalKeyCode,
        modifiers: crate::core::event::Modifiers,
        cx: &mut ScreenContext<'_>,
    ) -> bool {
        for (shortcut, handler) in &self.bindings {
            if shortcut.key.matches(key, physical) && shortcut.modifiers == modifiers {
                if handler(cx) {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for GlobalShortcuts {
    fn default() -> Self {
        Self::new()
    }
}

/// SPEC-HAL-018: Application root container.
///
/// Owns global context, screen stack, modal stack, and shortcuts.
/// Stage 3: Also owns the active render root NodeId.
pub struct AppRoot {
    pub context: AppContext,
    pub screens: ScreenStack,
    pub modals: ModalStack,
    pub shortcuts: GlobalShortcuts,
    /// SPEC-HAL-127: The single authoritative overlay stack.
    ///
    /// Stage 1: Structure only. Not wired to input routing yet.
    /// ModalStack remains the storage of modal screens.
    /// OverlayRoot will eventually become the authority for overlay ordering
    /// and input capture semantics.
    pub overlay_root: OverlayRoot,
    root_node: NodeId,
    /// SPEC-HAL-018 Stage 12: The currently mounted base screen widget.
    /// Only ONE base screen widget should be in the tree at a time.
    /// When the screen stack changes (push/pop/replace), this is updated.
    base_screen_node: Option<NodeId>,
    /// SPEC-HAL-018 Stage 12 / SPEC-HAL-127 Stage 4.2: Pending widget teardowns.
    ///
    /// When modals are closed, their mounted NodeIds are queued here for
    /// deferred removal from the widget tree.
    ///
    /// On close_modal/close_all_modals:
    ///   - Screen is removed from ModalStack immediately
    ///   - OverlayRoot entry is popped immediately (for correct top_id behavior)
    ///   - If mounted_node exists, it's pushed here for deferred widget removal
    ///
    /// On rebuild_active_view:
    ///   - For each pending teardown: remove widget from tree (unmount)
    pending_widget_teardowns: Vec<NodeId>,
    /// SPEC-HAL-137 INV-OVL-INPUT-1: when PointerDown outside a modal dismisses
    /// it, the rest of that physical pointer sequence is still owned by the
    /// dismissed modal boundary and must not click through to base content.
    modal_dismiss_pointer_sequence_active: bool,
    modal_dismiss_touch_sequences:
        std::collections::HashSet<(WindowId, crate::core::event::PointerId)>,
    /// SPEC-HAL-269: Mounted toast widget node (non-modal overlay).
    toast_widget_node: Option<NodeId>,
    /// SPEC-HAL-333 Rev 4 INV-G1: the menu host projection. None
    /// until App::run constructs it after window creation.
    menu_host: Option<Box<dyn crate::platform::menu_host::MenuHost>>,
    /// SPEC-HAL-333 Rev 4 INV-G2: the mounted in-window bar node.
    menu_bar_node: Option<NodeId>,
}

mod app_root;

#[cfg(test)]
#[path = "../tests/app/app_root_toast_tests.rs"]
mod app_root_toast_tests;

#[cfg(test)]
#[path = "../tests/app/app_menu_host_tests.rs"]
mod app_menu_host_tests;

#[cfg(test)]
#[path = "../tests/app/app_root_overlay_input_tests.rs"]
pub(crate) mod app_root_overlay_input_tests;

#[cfg(test)]
#[path = "../tests/app/app_signal_tests.rs"]
mod app_signal_tests;

// ============================================================================
// SPEC-HAL-018 Stage 16: App Facade for Simplified Example Setup
// ============================================================================

/// SPEC-HAL-018 Stage 16: Configuration for the App facade.
///
/// Allows customizing window title, windowed size, startup fullscreen, and theme.
pub struct AppConfig {
    #[cfg(target_arch = "wasm32")]
    web_navigation: Option<WebNavigation>,
    /// Application input policy; keyboard navigation and Capture remain the default.
    pub ui_interaction_policy: crate::core::event::UiInteractionPolicy,
    developer_shortcuts_enabled: bool,
    /// App-local logical UI scale, independent of the OS device pixel ratio.
    pub ui_scale: UiScale,
    /// Immutable application faces admitted before the first Screen is measured.
    pub font_assets: Vec<crate::platform::font_manager::FontAsset>,
    /// Window title
    pub title: String,
    /// Requested windowed width in logical pixels
    pub width: f32,
    /// Requested windowed height in logical pixels
    pub height: f32,
    /// Request native fullscreen at initial window creation; default is windowed.
    pub fullscreen: bool,
    /// Theme for widget rendering
    pub theme: Theme,
    #[cfg(target_os = "ios")]
    ios_window_binding: Option<fn(&winit::window::Window)>,
    frame_realization_observer: Option<AppFrameObserver>,
    flat_viewport3d_transfer_capacity: Option<std::num::NonZeroUsize>,
    #[cfg(all(target_os = "macos", debug_assertions))]
    relative_pointer_engine_conformance:
        Option<crate::platform::relative_pointer_macos_conformance::EngineRecorder>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Halcyon App".to_string(),
            #[cfg(target_arch = "wasm32")]
            web_navigation: None,
            ui_interaction_policy: crate::core::event::UiInteractionPolicy::default(),
            developer_shortcuts_enabled: true,
            ui_scale: UiScale::ONE,
            font_assets: Vec::new(),
            width: 800.0,
            height: 600.0,
            fullscreen: false,
            theme: Theme::default(),
            #[cfg(target_os = "ios")]
            ios_window_binding: None,
            frame_realization_observer: None,
            flat_viewport3d_transfer_capacity: None,
            #[cfg(all(target_os = "macos", debug_assertions))]
            relative_pointer_engine_conformance: None,
        }
    }
}

impl AppConfig {
    /// Bind public paths to the ordinary App's root. Its Screen renders from
    /// the committed projection and submits intents instead of stacking base
    /// screens. Applications without a route declaration retain their screen API.
    #[cfg(target_arch = "wasm32")]
    pub fn with_web_navigation(mut self, navigation: WebNavigation) -> Self {
        self.web_navigation = Some(navigation);
        self
    }

    pub(crate) fn interaction_domains(&self) -> InteractionDomains {
        InteractionDomains::with_ui_interaction_policy(self.ui_interaction_policy)
    }

    /// Enable the HAL-354 G1 flat viewport transfer with an explicit finite
    /// application-wide reservation bound. Default configuration admits no links.
    /// Allocation failure remains a typed admission error at AppContext.
    pub fn with_flat_viewport3d_transfer_capacity(
        mut self,
        capacity: std::num::NonZeroUsize,
    ) -> Self {
        self.flat_viewport3d_transfer_capacity = Some(capacity);
        self
    }

    pub fn with_ui_interaction_policy(
        mut self,
        policy: crate::core::event::UiInteractionPolicy,
    ) -> Self {
        self.ui_interaction_policy = policy;
        self
    }

    /// Set whether F9 exports a blueprint snapshot. Enabled by default.
    /// When disabled, F9 follows the app's ordinary input routing.
    pub fn with_developer_shortcuts_enabled(mut self, enabled: bool) -> Self {
        self.developer_shortcuts_enabled = enabled;
        self
    }

    pub fn with_ui_scale(mut self, scale: UiScale) -> Self {
        self.ui_scale = scale;
        self
    }

    pub fn with_font_assets(
        mut self,
        assets: Vec<crate::platform::font_manager::FontAsset>,
    ) -> Self {
        self.font_assets = assets;
        self
    }

    #[cfg(all(target_os = "macos", debug_assertions))]
    pub(crate) fn with_relative_pointer_engine_conformance(
        mut self,
        recorder: crate::platform::relative_pointer_macos_conformance::EngineRecorder,
    ) -> Self {
        self.relative_pointer_engine_conformance = Some(recorder);
        self
    }

    /// Create a new AppConfig with a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    /// Set the requested windowed size in logical pixels.
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Request native fullscreen once at startup, preserving the windowed size.
    pub fn with_fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    /// Set theme
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Bind the created primary window to the application's UIKit scene host.
    /// The callback configures native transport only, before UIApplicationMain.
    #[cfg(target_os = "ios")]
    pub fn with_ios_window_binding(mut self, binding: fn(&winit::window::Window)) -> Self {
        self.ios_window_binding = Some(binding);
        self
    }

    /// Observe immutable results of canonical App attempts, retaining only the
    /// latest snapshot. Does not create an event queue or request frames.
    pub fn with_frame_realization_observer(mut self) -> (Self, AppFrameObserver) {
        let observer = AppFrameObserver::new();
        self.frame_realization_observer = Some(observer.clone());
        (self, observer)
    }
}

fn startup_viewport_rect(physical_size: winit::dpi::PhysicalSize<u32>, scale_factor: f32) -> RectF {
    crate::utils::coords::checked_logical_viewport(
        physical_size.width,
        physical_size.height,
        scale_factor,
    )
    .expect("initial App viewport cannot represent its UI scale")
}

#[cfg(test)]
#[path = "../tests/app/startup_fullscreen_tests.rs"]
mod startup_fullscreen_tests;

/// Widget information recorded by the local F9 blueprint export.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
struct WidgetSnapshot {
    id: u64,
    type_name: String,
    label: Option<String>,
    hint: Option<String>,
    grow: Option<f32>,
    spacing: Option<f32>,
    placeholder: Option<String>,
    value: Option<f32>,
    style: Option<String>,
    parent_id: Option<u64>,
}

#[derive(serde::Serialize)]
struct TreeSnapshot {
    widgets: std::collections::HashMap<u64, WidgetSnapshot>,
}

fn capture_tree_snapshot(tree: &WidgetTree) -> TreeSnapshot {
    let mut widgets: std::collections::HashMap<u64, WidgetSnapshot> =
        std::collections::HashMap::new();

    for (node_id, node) in tree.iter_nodes() {
        let id = match tree.widget_id_for_node(node_id) {
            Some(id) => id.0,
            None => continue,
        };
        let type_name = node.widget.debug_type_name().to_string();
        let label = node.widget.text_info().map(|(t, _, _)| t.to_string());
        let hint = if node.diagnostic_hint.is_empty() {
            None
        } else {
            Some(node.diagnostic_hint.clone())
        };
        let grow = Some(node.flex.grow);

        let mut spacing: Option<f32> = None;
        let mut placeholder: Option<String> = None;
        let mut value: Option<f32> = None;
        let mut style: Option<String> = None;

        if let Some(flex) = node
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::flex::Flex>()
        {
            spacing = Some(flex.spacing());
        }
        if let Some(tf) = node
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::text_field::TextField>()
        {
            placeholder = tf.placeholder().map(|s| s.to_string());
        }
        if let Some(slider) = node
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::slider::Slider>()
        {
            value = Some(slider.value());
            style = Some(format!("{:?}", slider.display_style()));
        }
        if let Some(pb) = node
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::progress_bar::ProgressBar>()
        {
            value = Some(pb.value());
        }

        let parent_id = node
            .parent
            .and_then(|pn| tree.widget_id_for_node(pn).map(|id| id.0));

        widgets.insert(
            id,
            WidgetSnapshot {
                id,
                type_name,
                label,
                hint,
                grow,
                spacing,
                placeholder,
                value,
                style,
                parent_id,
            },
        );
    }

    TreeSnapshot { widgets }
}

fn write_blueprint_shortcut_snapshot(snapshot: &TreeSnapshot) {
    let json = serde_json::to_string_pretty(snapshot).unwrap_or_default();
    let filename = format!("blueprint_{}.json", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    match std::fs::write(&filename, &json) {
        Ok(_) => eprintln!("[F9] Blueprint exported to {}", filename),
        Err(e) => eprintln!("[F9] Export failed: {}", e),
    }
}

/// SPEC-HAL-018 Stage 16: Simplified App facade for examples.
///
/// Provides a high-level API for running Halcyon applications:
/// ```ignore
/// let app = App::new(AppConfig::new("My App").with_size(800.0, 600.0));
/// app.run(MyScreen::new());
/// ```
///
/// The App facade:
/// - Creates the window and renderer
/// - Sets up AppRoot with proper ScreenHost
/// - Runs the event loop with correct frame timing
/// - Ensures widgets live under AppRoot → ScreenHost → Screen hierarchy
///
/// This is the ONLY supported way to run examples. Direct WidgetTree::new()
/// and create_root() calls bypass the proper constraint propagation pipeline.
pub struct App {
    config: AppConfig,
}

mod run;

#[cfg(debug_assertions)]
pub(crate) mod hal_pass_probe {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static ACC: RefCell<HashMap<&'static str, (u32, u64)>> = RefCell::new(HashMap::new());
    }

    pub(crate) fn is_enabled() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var_os("HAL_PROFILE_LAYOUT_PASS").is_some())
    }

    pub(crate) fn reset() {
        ACC.with(|a| a.borrow_mut().clear());
    }

    pub(crate) fn record(type_name: &'static str, us: u64) {
        if !is_enabled() {
            return;
        }
        ACC.with(|a| {
            let mut m = a.borrow_mut();
            let e = m.entry(type_name).or_insert((0, 0));
            e.0 += 1;
            e.1 += us;
        });
    }

    pub(crate) fn dump(total_layout_us: u64) {
        if !is_enabled() {
            return;
        }
        ACC.with(|a| {
            let m = a.borrow();
            let mut rows: Vec<(&'static str, u32, u64)> =
                m.iter().map(|(k, v)| (*k, v.0, v.1)).collect();
            // Sort by call count descending.
            rows.sort_by(|x, y| y.1.cmp(&x.1));
            let total_calls: u32 = rows.iter().map(|r| r.1).sum();
            eprintln!(
                "[HAL-PASS] layout_phase={:.1}ms total_layout_child_calls={}",
                total_layout_us as f64 / 1000.0,
                total_calls
            );
            for (name, count, incl_us) in rows.iter().take(8) {
                eprintln!(
                    "[HAL-PASS]   {:<30} calls={:<6} inclusive={:.1}ms",
                    name,
                    count,
                    *incl_us as f64 / 1000.0
                );
            }
        });
    }
}

#[cfg(test)]
#[path = "../tests/app/cursor_application_tests.rs"]
mod cursor_application_tests;

#[cfg(test)]
#[path = "../tests/app/concrete_animation_lifetime_tests.rs"]
mod concrete_animation_lifetime_tests;
