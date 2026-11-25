// ============================================================================
// SPEC-HAL-127 Stage 4.2: Overlay ↔ Widget Lifecycle Coupling Tests
// ============================================================================
//
// These tests verify that modal teardown maintains correct parity between
// ModalStack and OverlayRoot at the AppRoot level.
//
// Key behaviors tested:
// - S2.INV1: ModalStack and OverlayRoot maintain parity
// - close_modal pops from both stacks correctly
// - close_all_modals clears both stacks correctly
// - Widget teardown is deferred to rebuild_active_view
// ============================================================================

use halcyon::core::app::{AppContext, AppRoot, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::overlay_root::DismissPolicy;
use halcyon::core::theme::Theme;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::widgets::screen_host::ScreenHost;

mod native_wake_ticker_tests {
    use halcyon::core::app::{EngineTicker, FrameReason};
    use std::time::{Duration, Instant};

    fn recently_rendered_idle_ticker() -> (EngineTicker, Instant) {
        let start = Instant::now();
        let mut ticker = EngineTicker::new(start);
        for elapsed in [0, 34, 68] {
            let at = start + Duration::from_millis(elapsed);
            let frame = ticker.begin_frame_at(at);
            assert!(!ticker.should_throttle(frame.now));
            ticker.mark_rendered(frame.now);
        }
        assert!(ticker.is_idle());
        (ticker, start + Duration::from_millis(68))
    }

    #[test]
    fn native_ui_wake_old_unclassified_redraw_consumes_the_only_request() {
        let (mut ticker, last) = recently_rendered_idle_ticker();
        // The old native UserEvent arm changed only pending_render. Its
        // AboutToWait consumes that one request, before the idle throttle.
        let mut pending_render = true;
        let redraw = std::mem::take(&mut pending_render);
        assert!(redraw && !pending_render);
        let frame = ticker.begin_frame_at(last + Duration::from_millis(1));
        assert_eq!(frame.reason, FrameReason::Tick);
        assert!(ticker.should_throttle(frame.now));
        assert!(
            !pending_render,
            "no second redraw exists under ControlFlow::Wait"
        );
        assert!(ticker.should_throttle(last + Duration::from_millis(32)));
        assert!(!ticker.should_throttle(last + Duration::from_millis(33)));
    }

    #[test]
    fn native_ui_wake_forced_reason_admits_once_and_preserves_idle_interval() {
        let (mut ticker, last) = recently_rendered_idle_ticker();
        let before = 3; // The helper begins exactly three real frames.
        // Coalesced notifications retain one pending frame/reason, not a queue
        // of synthetic frames. A hidden window does not consume either value.
        let mut pending_render = false;
        for _ in 0..4 {
            pending_render = true;
            ticker.notify_reason(FrameReason::Forced);
        }
        assert!(!ticker.is_idle());
        let visible = false;
        if visible {
            pending_render = false;
        }
        assert!(pending_render);
        assert!(std::mem::take(&mut pending_render));
        let wake = ticker.begin_frame_at(last + Duration::from_millis(1));
        assert_eq!(wake.reason, FrameReason::Forced);
        assert_eq!(wake.frame_index, before);
        assert!(!ticker.should_throttle(wake.now));
        ticker.mark_rendered(wake.now);
        assert!(!pending_render);
        // Existing unprompted Tick behavior remains. No timer/redraw is added.
        let next = ticker.begin_frame_at(last + Duration::from_millis(2));
        assert_eq!(next.frame_index, before + 1);
        assert_eq!(next.reason, FrameReason::Tick);
        let quiet = ticker.begin_frame_at(last + Duration::from_millis(3));
        assert_eq!(quiet.reason, FrameReason::Tick);
        assert!(ticker.is_idle());
        assert!(ticker.should_throttle(quiet.now));
        assert!(ticker.should_throttle(wake.now + Duration::from_millis(32)));
        assert!(!ticker.should_throttle(wake.now + Duration::from_millis(33)));
        for reason in [FrameReason::Input, FrameReason::Animation, FrameReason::Forced] {
            ticker.notify_reason(reason);
            let f = ticker.begin_frame_at(wake.now + Duration::from_millis(34));
            assert_eq!(f.reason, reason);
            assert!(!ticker.should_throttle(f.now));
        }
    }
}

mod control_subtree_phase_tests {
    use super::*;
    use halcyon::core::rect_authority::{
        AllowPaintMutationGuard, LayoutAuthorityGuard, PaintAuthorityGuard,
    };
    use halcyon::core::widget_tree::{ControlSubtreePlacement, NodeId};
    use halcyon::widgets::{flex::Flex, label::Label};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    fn tree_and_host() -> (WidgetTree, NodeId) {
        let mut tree = WidgetTree::new();
        let host = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        (tree, host)
    }

    fn assert_mutation_is_legal() {
        let (mut tree, host) = tree_and_host();
        tree.set_control_subtree_order(host, &[], ControlSubtreePlacement::Main { z_index: 200 })
            .unwrap();
        tree.clear_control_subtree_order(host).unwrap();
    }

    #[test]
    fn hal359_control_subtree_phase_app_build_and_deferred_mutation() {
        struct OrderedScreen {
            placeholder: ScreenHost,
            nodes: Rc<Cell<Option<(NodeId, [NodeId; 2])>>>,
            flushed: Rc<Cell<bool>>,
        }

        impl Screen for OrderedScreen {
            fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
                let host = cx.mount_child(Box::new(Flex::column()));
                let a = cx.tree.add_child(host, Box::new(Label::new("A")), None, 0);
                let b = cx.tree.add_child(host, Box::new(Label::new("B")), None, 0);
                cx.tree
                    .set_control_subtree_order(
                        host,
                        &[a, b],
                        ControlSubtreePlacement::Main { z_index: 200 },
                    )
                    .unwrap();
                self.nodes.set(Some((host, [a, b])));
                &mut self.placeholder
            }

            fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
                let (host, [a, b]) = self.nodes.get().unwrap();
                let tree = cx.tree.as_deref_mut().unwrap();
                tree.clear_control_subtree_order(host).unwrap();
                tree.set_control_subtree_order(
                    host,
                    &[b, a],
                    ControlSubtreePlacement::Main { z_index: 200 },
                )
                .unwrap();
                self.flushed.set(true);
            }
        }

        let (mut tree, root) = tree_and_host();
        let mut app = AppRoot::new(AppContext::new(Theme::default()), root);
        let nodes = Rc::new(Cell::new(None));
        let flushed = Rc::new(Cell::new(false));
        app.push_screen(Box::new(OrderedScreen {
            placeholder: ScreenHost::new(),
            nodes: nodes.clone(),
            flushed: flushed.clone(),
        }));
        app.rebuild_active_view(&mut tree);
        let (host, [a, b]) = nodes.get().unwrap();
        assert_eq!(tree.children(host), [a, b]);
        app.flush_pending_actions_all(&mut tree);
        assert!(flushed.get());
        tree.clear_control_subtree_order(host).unwrap();
    }

    #[test]
    #[should_panic(expected = "INV-359-CONTROL-MUTATION-PHASE")]
    fn hal359_control_subtree_phase_set_during_layout_panics() {
        let (mut tree, host) = tree_and_host();
        let _layout = LayoutAuthorityGuard::enter();
        tree.set_control_subtree_order(host, &[], ControlSubtreePlacement::Main { z_index: 200 })
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "INV-359-CONTROL-MUTATION-PHASE")]
    fn hal359_control_subtree_phase_clear_during_layout_panics() {
        let (mut tree, host) = tree_and_host();
        let _layout = LayoutAuthorityGuard::enter();
        tree.clear_control_subtree_order(host).unwrap();
    }

    #[test]
    #[should_panic(expected = "INV-359-CONTROL-MUTATION-PHASE")]
    fn hal359_control_subtree_phase_set_during_paint_panics() {
        let (mut tree, host) = tree_and_host();
        let _paint = PaintAuthorityGuard::enter();
        tree.set_control_subtree_order(host, &[], ControlSubtreePlacement::Main { z_index: 200 })
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "INV-359-CONTROL-MUTATION-PHASE")]
    fn hal359_control_subtree_phase_clear_during_allowed_paint_panics() {
        let (mut tree, host) = tree_and_host();
        let _paint = PaintAuthorityGuard::enter();
        let _allow = AllowPaintMutationGuard::enter();
        tree.clear_control_subtree_order(host).unwrap();
    }

    #[test]
    fn hal359_control_subtree_phase_return_and_thread_isolation() {
        fn paint_then_return() {
            let _paint = PaintAuthorityGuard::enter();
            std::thread::spawn(assert_mutation_is_legal).join().unwrap();
        }
        paint_then_return();
        assert_mutation_is_legal();
    }

    #[test]
    fn hal359_control_subtree_phase_unwind_cleanup() {
        struct ObserveCleanup(Arc<AtomicBool>);
        impl Drop for ObserveCleanup {
            fn drop(&mut self) {
                assert_mutation_is_legal();
                self.0.store(true, Ordering::Release);
            }
        }

        for paint in [false, true] {
            let cleaned = Arc::new(AtomicBool::new(false));
            let observation = cleaned.clone();
            let result = std::thread::spawn(move || {
                let _observe = ObserveCleanup(observation);
                if paint {
                    let _paint = PaintAuthorityGuard::enter();
                    panic!("exercise Paint guard unwind");
                } else {
                    let _layout = LayoutAuthorityGuard::enter();
                    panic!("exercise Layout guard unwind");
                }
            })
            .join();
            assert!(result.is_err());
            assert!(cleaned.load(Ordering::Acquire));
        }
        assert_mutation_is_legal();
    }

    #[test]
    #[cfg(not(debug_assertions))]
    #[should_panic(expected = "INV-359-CONTROL-MUTATION-PHASE")]
    fn hal359_control_subtree_phase_release_nested_paint_preserves_outer_scope() {
        let (mut tree, host) = tree_and_host();
        let _outer = PaintAuthorityGuard::enter();
        {
            let _inner = PaintAuthorityGuard::enter();
        }
        tree.clear_control_subtree_order(host).unwrap();
    }
}

// ============================================================================
// Test Modal Implementation
// ============================================================================

/// Minimal test modal for lifecycle tests.
struct TestModal {
    _id: usize,
}

impl TestModal {
    fn new(id: usize) -> Self {
        Self { _id: id }
    }
}

impl Screen for TestModal {
    fn build<'a>(&'a mut self, _cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        panic!("TestModal::build should not be called in lifecycle tests")
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn covers_point(&self, _logical_pos: halcyon::core::math::Point) -> bool {
        true
    }

    fn dismiss_policy(&self) -> DismissPolicy {
        DismissPolicy::modal_default()
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn create_app_root() -> (AppRoot, WidgetTree) {
    let mut tree = WidgetTree::new();
    let root_node = tree.create_root(Box::new(ScreenHost::new()), None, 0);
    let context = AppContext::new(Theme::dark());
    let app_root = AppRoot::new(context, root_node);
    (app_root, tree)
}

// ============================================================================
// Basic Close Tests
// ============================================================================

/// close_modal removes from both ModalStack and OverlayRoot
#[test]
fn close_modal_maintains_parity() {
    let (mut app_root, mut _tree) = create_app_root();

    // Open a modal
    app_root.open_modal(Box::new(TestModal::new(1)));
    assert_eq!(app_root.overlay_root.len(), 1);
    assert_eq!(app_root.modals.len(), 1);

    // Close the modal
    let closed = app_root.close_modal();
    assert!(closed);

    // Both stacks should be empty
    assert_eq!(app_root.modals.len(), 0, "ModalStack should be empty");
    assert_eq!(
        app_root.overlay_root.len(),
        0,
        "OverlayRoot should be empty"
    );
}

/// Multiple close_modal calls maintain parity
#[test]
fn multiple_closes_maintain_parity() {
    let (mut app_root, mut _tree) = create_app_root();

    // Open three modals
    app_root.open_modal(Box::new(TestModal::new(1)));
    app_root.open_modal(Box::new(TestModal::new(2)));
    app_root.open_modal(Box::new(TestModal::new(3)));
    assert_eq!(app_root.overlay_root.len(), 3);
    assert_eq!(app_root.modals.len(), 3);

    // Close two modals
    app_root.close_modal();
    assert_eq!(app_root.modals.len(), 2);
    assert_eq!(app_root.overlay_root.len(), 2, "Parity after first close");

    app_root.close_modal();
    assert_eq!(app_root.modals.len(), 1);
    assert_eq!(app_root.overlay_root.len(), 1, "Parity after second close");
}

/// close_all_modals clears both stacks
#[test]
fn close_all_modals_maintains_parity() {
    let (mut app_root, mut _tree) = create_app_root();

    // Open three modals
    app_root.open_modal(Box::new(TestModal::new(1)));
    app_root.open_modal(Box::new(TestModal::new(2)));
    app_root.open_modal(Box::new(TestModal::new(3)));
    assert_eq!(app_root.overlay_root.len(), 3);
    assert_eq!(app_root.modals.len(), 3);

    // Close all modals
    app_root.close_all_modals();

    // Both stacks should be empty
    assert_eq!(app_root.modals.len(), 0, "ModalStack should be empty");
    assert_eq!(
        app_root.overlay_root.len(),
        0,
        "OverlayRoot should be empty"
    );
}

// ============================================================================
// OverlayId Correctness Tests
// ============================================================================

/// top_id returns correct overlay after close
#[test]
fn top_id_correct_after_close() {
    let (mut app_root, mut _tree) = create_app_root();

    // Open two modals
    app_root.open_modal(Box::new(TestModal::new(1)));
    let id1 = app_root.overlay_root.top_id().unwrap();

    app_root.open_modal(Box::new(TestModal::new(2)));
    let id2 = app_root.overlay_root.top_id().unwrap();
    assert_ne!(id1, id2);

    // Close top modal - id1 should now be top
    app_root.close_modal();
    assert_eq!(app_root.overlay_root.top_id(), Some(id1));
}

/// Interleaved open/close maintains correct overlay ordering
#[test]
fn interleaved_open_close_correct() {
    let (mut app_root, mut _tree) = create_app_root();

    // Open modal 1
    app_root.open_modal(Box::new(TestModal::new(1)));
    let id1 = app_root.overlay_root.top_id().unwrap();
    assert_eq!(app_root.overlay_root.len(), 1);

    // Close modal 1
    app_root.close_modal();
    assert_eq!(app_root.overlay_root.len(), 0);

    // Open modal 2
    app_root.open_modal(Box::new(TestModal::new(2)));
    let id2 = app_root.overlay_root.top_id().unwrap();
    assert_ne!(id1, id2);
    assert_eq!(app_root.overlay_root.len(), 1);

    // Parity maintained
    assert_eq!(app_root.modals.len(), 1);
    assert_eq!(app_root.overlay_root.top_id(), Some(id2));
}

// ============================================================================
// Edge Cases
// ============================================================================

/// Close on empty stack is safe
#[test]
fn close_empty_is_noop() {
    let (mut app_root, mut _tree) = create_app_root();

    // No modals
    assert!(app_root.modals.is_empty());
    assert!(app_root.overlay_root.is_empty());

    // Close should be a no-op
    let closed = app_root.close_modal();
    assert!(!closed);

    assert!(app_root.modals.is_empty());
    assert!(app_root.overlay_root.is_empty());
}

/// close_all_modals on empty stack is safe
#[test]
fn close_all_empty_is_noop() {
    let (mut app_root, mut _tree) = create_app_root();

    // No modals
    assert!(app_root.modals.is_empty());
    assert!(app_root.overlay_root.is_empty());

    // Close all should be a no-op
    app_root.close_all_modals();

    assert!(app_root.modals.is_empty());
    assert!(app_root.overlay_root.is_empty());
}

/// Opening without closing maintains parity
#[test]
fn open_without_close_maintains_parity() {
    let (mut app_root, mut _tree) = create_app_root();

    // Open a modal but don't close it
    app_root.open_modal(Box::new(TestModal::new(1)));
    assert_eq!(app_root.overlay_root.len(), 1);
    assert_eq!(app_root.modals.len(), 1);

    // Parity maintained
    assert_eq!(
        app_root.overlay_root.len(),
        app_root.modals.len(),
        "Parity maintained without close"
    );
}

// ============================================================================
// Stale NodeId Prevention Tests
// ============================================================================

/// After close, overlay entry is removed (no stale references)
#[test]
fn no_stale_overlay_entries_after_close() {
    let (mut app_root, mut _tree) = create_app_root();

    // Open a modal
    app_root.open_modal(Box::new(TestModal::new(1)));
    let id1 = app_root.overlay_root.top_id().unwrap();

    // Verify overlay has the entry
    assert!(app_root.overlay_root.top().is_some());

    // Close the modal
    app_root.close_modal();

    // OverlayRoot is empty - no stale entries
    assert!(app_root.overlay_root.is_empty());
    assert!(app_root.overlay_root.top().is_none());

    // The ID should no longer be findable
    // (We can't directly check remove_by_id result, but emptiness proves it)
    let _ = id1; // Acknowledge we captured this for verification purposes
}
