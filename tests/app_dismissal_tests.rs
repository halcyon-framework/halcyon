// ============================================================================
// SPEC-HAL-127 Stage 4.1: AppRoot-Level Dismissal Semantics Tests
// ============================================================================
//
// These tests verify that dismissal semantics work correctly at the AppRoot
// level (handle_fused_input_modal_first), not just at OverlayRoot level.
//
// Stage 4.1 Invariants tested:

// - S4.1.INV1: One Event → At Most One Modal Close
// - S4.1.INV2: ESC Dismissal Is Centralized
// - S4.1.INV3: Outside Click Dismissal Is Centralized
// - S4.1.INV4: No Double Dismissal Paths
// ============================================================================

use halcyon::core::app::{AppContext, AppRoot, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{
    FusedInputEvent, KeyCode, Modifiers, PointerButton, PointerDevice, WindowId,
};
use halcyon::core::math::{Point, RectF};
use halcyon::core::overlay_root::DismissPolicy;
use halcyon::core::theme::Theme;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::widgets::screen_host::ScreenHost;
use std::time::Instant;

// ============================================================================
// Test Modal Implementation
// ============================================================================

/// Configurable test modal for dismissal tests.
///
/// Allows control over:
/// - dismiss_policy (modal_default or popup_default)
/// - covers_point behavior (returns true only inside modal_rect)
struct TestModal {
    /// Policy for dismiss behavior
    policy: DismissPolicy,
    /// Rect that this modal "covers" (for hit-testing)
    modal_rect: RectF,
    /// Track if handle_input was called with ESC (for double-dismiss detection)
    saw_esc: bool,
}

impl TestModal {
    fn new_modal_default() -> Self {
        Self {
            policy: DismissPolicy::modal_default(),
            // Modal covers center 200x200 area
            modal_rect: RectF::new(300.0, 200.0, 200.0, 200.0),
            saw_esc: false,
        }
    }

    fn new_popup_default() -> Self {
        Self {
            policy: DismissPolicy::popup_default(),
            modal_rect: RectF::new(300.0, 200.0, 200.0, 200.0),
            saw_esc: false,
        }
    }

    fn did_see_esc(&self) -> bool {
        self.saw_esc
    }
}

impl Screen for TestModal {
    fn build<'a>(&'a mut self, _cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        panic!("TestModal::build should not be called in these tests")
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // Track if we see ESC (for testing that modal doesn't self-dismiss)
        if let FusedInputEvent::KeyDown { key_code, .. } = event {
            if *key_code == KeyCode::Escape {
                self.saw_esc = true;
                // Return false to let ESC propagate to centralized handling
                // S4.INV2: Modal code must not decide dismissal
                return false;
            }
        }
        false
    }

    fn covers_point(&self, logical_pos: Point) -> bool {
        self.modal_rect.contains(logical_pos)
    }

    fn dismiss_policy(&self) -> DismissPolicy {
        self.policy
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

fn make_esc_event() -> FusedInputEvent {
    FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Escape,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    }
}

fn make_pointer_down(pos: Point) -> FusedInputEvent {
    FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    }
}

// ============================================================================
// S4.1.INV2: ESC Dismissal Is Centralized
// ============================================================================

/// S4.1.INV2: Modal with on_escape=true closes on ESC
#[test]
fn s41_esc_closes_modal_when_policy_allows() {
    let (mut app_root, mut tree) = create_app_root();

    // Open a modal with modal_default (on_escape: true)
    app_root.open_modal(Box::new(TestModal::new_modal_default()));
    assert_eq!(app_root.modals.len(), 1);
    assert_eq!(app_root.overlay_root.len(), 1);

    // Send ESC event
    let event = make_esc_event();
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // S4.1.INV2: ESC closes exactly one modal and is consumed
    assert!(
        handled,
        "ESC should be consumed when policy.on_escape is true"
    );
    assert_eq!(app_root.modals.len(), 0, "Modal should be closed");
    assert_eq!(
        app_root.overlay_root.len(),
        0,
        "OverlayRoot should be empty"
    );
}

/// S4.1.INV2: Modal with on_escape=false does NOT close on ESC
#[test]
fn s41_esc_does_not_close_when_policy_disallows() {
    let (mut app_root, mut tree) = create_app_root();

    // Create a modal with on_escape disabled
    let modal = TestModal {
        policy: DismissPolicy {
            on_outside_click: false,
            on_escape: false,
        },
        modal_rect: RectF::new(300.0, 200.0, 200.0, 200.0),
        saw_esc: false,
    };
    app_root.open_modal(Box::new(modal));
    assert_eq!(app_root.modals.len(), 1);

    // Send ESC event
    let event = make_esc_event();
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // S4.1.INV2: ESC does NOT close modal when policy forbids
    // Note: ESC is not consumed at app level when policy.on_escape is false
    assert!(
        !handled,
        "ESC should not be consumed when policy.on_escape is false"
    );
    assert_eq!(app_root.modals.len(), 1, "Modal should NOT be closed");
    assert_eq!(
        app_root.overlay_root.len(),
        1,
        "OverlayRoot should still have entry"
    );
}

/// S4.1.INV2: popup_default also closes on ESC
#[test]
fn s41_popup_closes_on_esc() {
    let (mut app_root, mut tree) = create_app_root();

    // Open a popup (on_escape: true, on_outside_click: true)
    app_root.open_modal(Box::new(TestModal::new_popup_default()));
    assert_eq!(app_root.modals.len(), 1);

    // Send ESC event
    let event = make_esc_event();
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // S4.1.INV2: ESC closes popup
    assert!(handled, "ESC should be consumed for popup");
    assert_eq!(app_root.modals.len(), 0, "Popup should be closed");
}

// ============================================================================
// S4.1.INV3: Outside Click Dismissal Is Centralized
// ============================================================================

/// S4.1.INV3: popup_default closes on outside click
#[test]
fn s41_outside_click_closes_popup() {
    let (mut app_root, mut tree) = create_app_root();

    // Open a popup (on_outside_click: true)
    app_root.open_modal(Box::new(TestModal::new_popup_default()));
    assert_eq!(app_root.modals.len(), 1);

    // Click outside the modal rect (300,200,200,200) means outside is e.g. (100,100)
    let event = make_pointer_down(Point::new(100.0, 100.0));
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // S4.1.INV3: Outside click closes popup and event is consumed (shielding)
    assert!(handled, "Outside click should be consumed (shielding)");
    assert_eq!(
        app_root.modals.len(),
        0,
        "Popup should be closed on outside click"
    );
    assert_eq!(
        app_root.overlay_root.len(),
        0,
        "OverlayRoot should be empty"
    );
}

/// S4.1.INV3: modal_default does NOT close on outside click
#[test]
fn s41_outside_click_does_not_close_standard_modal() {
    let (mut app_root, mut tree) = create_app_root();

    // Open a modal (on_outside_click: false)
    app_root.open_modal(Box::new(TestModal::new_modal_default()));
    assert_eq!(app_root.modals.len(), 1);

    // Click outside
    let event = make_pointer_down(Point::new(100.0, 100.0));
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // S4.1.INV3: Event is consumed (shielding) but modal is NOT closed
    assert!(handled, "Outside click should be consumed (shielding)");
    assert_eq!(app_root.modals.len(), 1, "Modal should NOT be closed");
    assert_eq!(
        app_root.overlay_root.len(),
        1,
        "OverlayRoot should still have entry"
    );
}

/// S4.1.INV3: Click INSIDE modal does not trigger dismissal
#[test]
fn s41_inside_click_does_not_dismiss() {
    let (mut app_root, mut tree) = create_app_root();

    // Open a popup
    app_root.open_modal(Box::new(TestModal::new_popup_default()));
    assert_eq!(app_root.modals.len(), 1);

    // Click inside the modal rect (300,200,200,200) - center is (400,300)
    let event = make_pointer_down(Point::new(400.0, 300.0));
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // Inside click should NOT dismiss (even for popup)
    // Note: handled is false because event is not consumed by shielding when inside
    assert!(!handled, "Inside click should not be consumed by shielding");
    assert_eq!(
        app_root.modals.len(),
        1,
        "Modal should NOT be closed on inside click"
    );
}

// ============================================================================
// S4.1.INV1: One Event → At Most One Modal Close
// ============================================================================

/// S4.1.INV1: ESC closes only the top modal
#[test]
fn s41_esc_closes_only_top_modal() {
    let (mut app_root, mut tree) = create_app_root();

    // Open two modals
    app_root.open_modal(Box::new(TestModal::new_modal_default()));
    app_root.open_modal(Box::new(TestModal::new_modal_default()));
    assert_eq!(app_root.modals.len(), 2);

    // Send ESC event
    let event = make_esc_event();
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // S4.1.INV1: Only one modal closed per event
    assert!(handled);
    assert_eq!(app_root.modals.len(), 1, "Only top modal should be closed");
    assert_eq!(
        app_root.overlay_root.len(),
        1,
        "OverlayRoot should have one entry"
    );
}

/// S4.1.INV1: Outside click closes only the top modal
#[test]
fn s41_outside_click_closes_only_top_modal() {
    let (mut app_root, mut tree) = create_app_root();

    // Open two popups
    app_root.open_modal(Box::new(TestModal::new_popup_default()));
    app_root.open_modal(Box::new(TestModal::new_popup_default()));
    assert_eq!(app_root.modals.len(), 2);

    // Click outside
    let event = make_pointer_down(Point::new(100.0, 100.0));
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // S4.1.INV1: Only one modal closed per event
    assert!(handled);
    assert_eq!(app_root.modals.len(), 1, "Only top modal should be closed");
    assert_eq!(
        app_root.overlay_root.len(),
        1,
        "OverlayRoot should have one entry"
    );
}

// ============================================================================
// S4.1.INV4: No Double Dismissal Paths
// ============================================================================

/// S4.1.INV4: ESC is handled via centralized path only, not modal handle_input
#[test]
fn s41_esc_goes_through_centralized_path_only() {
    let (mut app_root, mut tree) = create_app_root();

    // Open a modal
    app_root.open_modal(Box::new(TestModal::new_modal_default()));
    assert_eq!(app_root.modals.len(), 1);

    // Send ESC - the modal's handle_input will see it first, but should return false
    // (not handle it) to let centralized ESC handling work
    let event = make_esc_event();
    let _handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // Modal should be closed by centralized path
    assert_eq!(app_root.modals.len(), 0, "Modal should be closed");

    // The test passes if modal closes exactly once - if there were double dismissal,
    // the second close would fail or cause issues. The fact that modal is at 0 and
    // overlay_root is at 0 proves exactly one close happened.
    assert_eq!(app_root.overlay_root.len(), 0);
}

/// S4.1.INV4: Outside click on PointerDown (not PointerUp) triggers dismissal once
#[test]
fn s41_outside_click_only_on_pointer_down() {
    let (mut app_root, mut tree) = create_app_root();

    // Open a popup
    app_root.open_modal(Box::new(TestModal::new_popup_default()));
    assert_eq!(app_root.modals.len(), 1);

    // Send PointerMove outside - should NOT dismiss
    let move_event = FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(100.0, 100.0),
        delta: (0.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: halcyon::core::event::PointerButtonsState::default(),
    };
    let handled = app_root.handle_fused_input_modal_first(&move_event, &mut tree);

    assert!(
        handled,
        "PointerMove outside should be consumed (shielding)"
    );
    assert_eq!(app_root.modals.len(), 1, "PointerMove should NOT dismiss");

    // Send PointerUp outside - should NOT dismiss
    let up_event = FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(100.0, 100.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    };
    let handled = app_root.handle_fused_input_modal_first(&up_event, &mut tree);

    assert!(handled, "PointerUp outside should be consumed (shielding)");
    assert_eq!(app_root.modals.len(), 1, "PointerUp should NOT dismiss");

    // Send PointerDown outside - SHOULD dismiss
    let down_event = make_pointer_down(Point::new(100.0, 100.0));
    let handled = app_root.handle_fused_input_modal_first(&down_event, &mut tree);

    assert!(handled, "PointerDown outside should be consumed");
    assert_eq!(app_root.modals.len(), 0, "PointerDown should dismiss popup");
}

// ============================================================================
// Edge Cases
// ============================================================================

/// No modal means ESC is not consumed at app level
#[test]
fn s41_esc_with_no_modal_not_consumed() {
    let (mut app_root, mut tree) = create_app_root();

    // No modal open
    assert_eq!(app_root.modals.len(), 0);

    // Send ESC
    let event = make_esc_event();
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // ESC should not be consumed when no modal is open
    assert!(!handled, "ESC should not be consumed with no modal");
}

/// No modal means outside click is not consumed
#[test]
fn s41_click_with_no_modal_not_consumed() {
    let (mut app_root, mut tree) = create_app_root();

    // No modal open
    assert_eq!(app_root.modals.len(), 0);

    // Click anywhere
    let event = make_pointer_down(Point::new(100.0, 100.0));
    let handled = app_root.handle_fused_input_modal_first(&event, &mut tree);

    // Click should not be consumed when no modal
    assert!(!handled, "Click should not be consumed with no modal");
}
