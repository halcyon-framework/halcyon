// SPEC-HAL-038 Stage 3: Focus Scope Tests

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    FusedInputEvent, InteractionDomainState, KeyCode, Modifiers, PointerButton, PointerDevice,
    WindowId,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::utils::id::{TreeWidgetId, WidgetInternalId};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

// ============================================================================
// Focusable Test Widget
// ============================================================================

struct FocusableWidget {
    id: WidgetInternalId,
    events: Rc<RefCell<Vec<ScopeTestEvent>>>,
    label: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
enum ScopeTestEvent {
    OnFocus(&'static str),
    OnBlur(&'static str),
}

impl FocusableWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<ScopeTestEvent>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            events,
            label,
        }
    }
}

impl WidgetDowncast for FocusableWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FocusableWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 50.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(ScopeTestEvent::OnFocus(self.label));
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(ScopeTestEvent::OnBlur(self.label));
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

// ============================================================================
// Non-Focusable Container Widget
// ============================================================================

struct ContainerWidget {
    id: WidgetInternalId,
}

impl ContainerWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for ContainerWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ContainerWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(400.0, 200.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        false
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

// ============================================================================
// Focus Scope Container (like a modal)
// ============================================================================

struct ScopeWidget {
    id: WidgetInternalId,
}

impl ScopeWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for ScopeWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ScopeWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(200.0, 100.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        false
    }

    fn is_focus_scope(&self) -> bool {
        true
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

// ============================================================================
// Helper: Create Tab key event
// ============================================================================

fn tab_key_event(shift: bool) -> FusedInputEvent {
    FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Tab,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers {
            shift,
            ctrl: false,
            alt: false,
            meta: false,
        },
        repeat: false,
    }
}

// ============================================================================
// Helper: Setup tree with global widgets and a scope
// Tree structure:
//   root (container)
//   ├── global_a (focusable)
//   ├── global_b (focusable)
//   └── scope (ScopeWidget)
//       ├── scope_a (focusable)
//       └── scope_b (focusable)
// ============================================================================

struct ScopeTestSetup {
    tree: WidgetTree,
    root: NodeId,
    global_a: NodeId,
    global_b: NodeId,
    scope_node: NodeId,
    scope_a: NodeId,
    scope_b: NodeId,
    id_global_a: TreeWidgetId,
    id_global_b: TreeWidgetId,
    id_scope_a: TreeWidgetId,
    id_scope_b: TreeWidgetId,
    events: Rc<RefCell<Vec<ScopeTestEvent>>>,
}

fn setup_scope_test() -> ScopeTestSetup {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 300.0));

    // Global widget A
    let global_a = tree.add_child(
        root,
        Box::new(FocusableWidget::new("global_a", events.clone())),
        None,
        0,
    );
    tree.set_rect(global_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_global_a = tree.node(global_a).unwrap().id();

    // Global widget B
    let global_b = tree.add_child(
        root,
        Box::new(FocusableWidget::new("global_b", events.clone())),
        None,
        0,
    );
    tree.set_rect(global_b, RectF::new(110.0, 0.0, 100.0, 50.0));
    let id_global_b = tree.node(global_b).unwrap().id();

    // Scope container (like a modal)
    let scope_node = tree.add_child(root, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(scope_node, RectF::new(0.0, 100.0, 200.0, 100.0));

    // Scope widget A
    let scope_a = tree.add_child(
        scope_node,
        Box::new(FocusableWidget::new("scope_a", events.clone())),
        None,
        0,
    );
    tree.set_rect(scope_a, RectF::new(0.0, 0.0, 80.0, 40.0));
    let id_scope_a = tree.node(scope_a).unwrap().id();

    // Scope widget B
    let scope_b = tree.add_child(
        scope_node,
        Box::new(FocusableWidget::new("scope_b", events.clone())),
        None,
        0,
    );
    tree.set_rect(scope_b, RectF::new(90.0, 0.0, 80.0, 40.0));
    let id_scope_b = tree.node(scope_b).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    ScopeTestSetup {
        tree,
        root,
        global_a,
        global_b,
        scope_node,
        scope_a,
        scope_b,
        id_global_a,
        id_global_b,
        id_scope_a,
        id_scope_b,
        events,
    }
}

// ============================================================================
// Test 1: Scope Traps Tab Forward
// ============================================================================

#[test]
fn test_scope_traps_tab_forward() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push the scope (simulating modal open)
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    // Click on scope_a to focus it
    let click_scope_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(10.0, 110.0), // Inside scope_a (0,100) + (0,0) to (80,40)
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_scope_a,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));

    // Tab: scope_a → scope_b (within scope)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_b),
        "Tab should move to scope_b"
    );

    // Tab: scope_b → scope_a (wrap within scope, not to global)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_a),
        "Tab should wrap to scope_a, not escape to global"
    );
}

// ============================================================================
// Test 2: Scope Traps Tab Backward
// ============================================================================

#[test]
fn test_scope_traps_tab_backward() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push the scope
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    // Focus scope_a first
    let click_scope_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(10.0, 110.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_scope_a,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));

    // Shift+Tab: scope_a → scope_b (wrap backward within scope)
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_b),
        "Shift+Tab should wrap to scope_b"
    );

    // Shift+Tab: scope_b → scope_a (backward within scope)
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_a),
        "Shift+Tab should move to scope_a"
    );
}

// ============================================================================
// Test 3: Scope Release Restores Global Tab Order
// ============================================================================

#[test]
fn test_scope_release_restores_global_tab_order() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // First, verify global Tab works without scope
    let click_global_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_global_a,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_global_a)
    );

    // Tab should go to global_b, then scope_a, then scope_b, then wrap to global_a
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_global_b)
    );

    // Now push scope, do some Tab navigation, then pop
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    // Tab within scope (focus should move to first focusable in scope)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));

    // Pop scope (focus was inside scope, so it should be cleared)
    router.pop_focus_scope(&mut tree, &mut domain_state);
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should be cleared when scope is popped"
    );

    // Tab should now work globally again (first focusable)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_global_a),
        "After scope pop, Tab should work globally"
    );
}

// ============================================================================
// Test 4: Nested Scopes Select Innermost
// ============================================================================

#[test]
fn test_nested_scopes_select_innermost() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 400.0));

    // Outer scope
    let outer_scope = tree.add_child(root, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(outer_scope, RectF::new(0.0, 0.0, 300.0, 300.0));

    // Outer scope widget A
    let outer_a = tree.add_child(
        outer_scope,
        Box::new(FocusableWidget::new("outer_a", events.clone())),
        None,
        0,
    );
    tree.set_rect(outer_a, RectF::new(0.0, 0.0, 80.0, 40.0));
    let id_outer_a = tree.node(outer_a).unwrap().id();

    // Inner scope
    let inner_scope = tree.add_child(outer_scope, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(inner_scope, RectF::new(0.0, 50.0, 200.0, 150.0));

    // Inner scope widget A
    let inner_a = tree.add_child(
        inner_scope,
        Box::new(FocusableWidget::new("inner_a", events.clone())),
        None,
        0,
    );
    tree.set_rect(inner_a, RectF::new(0.0, 0.0, 80.0, 40.0));
    let id_inner_a = tree.node(inner_a).unwrap().id();

    // Inner scope widget B
    let inner_b = tree.add_child(
        inner_scope,
        Box::new(FocusableWidget::new("inner_b", events.clone())),
        None,
        0,
    );
    tree.set_rect(inner_b, RectF::new(90.0, 0.0, 80.0, 40.0));
    let id_inner_b = tree.node(inner_b).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push both scopes (outer first, then inner)
    router.push_focus_scope(outer_scope, &mut domain_state);
    router.push_focus_scope(inner_scope, &mut domain_state);

    // Focus inner_a
    let click_inner_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(10.0, 60.0), // Inside inner_a
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_inner_a,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_inner_a));

    // Tab should only cycle within inner scope (inner_a → inner_b → inner_a)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_inner_b));

    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_inner_a),
        "Tab should wrap within inner scope, not go to outer_a"
    );

    // Pop inner scope - Tab should now cycle in outer scope
    router.pop_focus_scope(&mut tree, &mut domain_state);

    // Focus should be cleared (was in inner scope)
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Tab should now focus outer_a (first in outer scope)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_outer_a),
        "After inner scope pop, Tab should work in outer scope"
    );
}

// ============================================================================
// Test 5: Focus Clears When Scope Popped
// ============================================================================

#[test]
fn test_focus_clears_when_scope_popped() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push scope and focus a widget inside it
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    let click_scope_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(10.0, 110.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_scope_a,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));

    // Pop scope - focus was inside, should be cleared
    router.pop_focus_scope(&mut tree, &mut domain_state);
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should be cleared when scope containing focused widget is popped"
    );
}

// ============================================================================
// Test 6: Click Moves Focus Into Scope
// ============================================================================

#[test]
fn test_click_moves_focus_into_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus global_a first
    let click_global = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_global,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_global_a)
    );

    // Push scope (simulating modal open)
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    // Click on scope_b
    let click_scope_b = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(140.0, 110.0), // Inside scope_b at (90,100)
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_scope_b,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should have moved to scope_b
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_b),
        "Click should move focus into scope widget"
    );

    setup.events.borrow_mut().clear();

    // Tab should cycle within scope
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));
}

// ============================================================================
// Test 7: Click Outside Scope Does Not Break Scope Navigation
// ============================================================================

#[test]
fn test_click_outside_scope_does_not_break_scope_navigation() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push scope and focus scope_a
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    let click_scope_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(10.0, 110.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_scope_a,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));

    // Click on global_a (outside scope)
    // Note: In a real modal scenario, this would be blocked by modal shielding.
    // But for focus scope testing, we verify Tab still works within scope.
    let click_global = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_global,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus moved to global_a
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_global_a)
    );

    // Tab should still cycle within scope (because scope is still active)
    // This tests that clicking outside doesn't break the scope stack
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Tab should find the first focusable in scope chain (scope_a)
    // because the scope is still pushed on the stack
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_a),
        "Tab should navigate within active scope even after click outside"
    );
}

// ============================================================================
// SPEC-HAL-038 Stage 4: Auto Focus Scope Tests
// ============================================================================

// Non-scope modal widget (is_focus_scope returns false)
struct NonScopeModalWidget {
    id: WidgetInternalId,
}

impl NonScopeModalWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for NonScopeModalWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for NonScopeModalWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(200.0, 100.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        false
    }

    fn is_focus_scope(&self) -> bool {
        false // Explicitly NOT a focus scope
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

// ============================================================================
// Test 8: Modal Scope Respects is_focus_scope
// ============================================================================

#[test]
fn test_modal_scope_respects_is_focus_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 400.0));

    // Non-scope modal (is_focus_scope = false)
    let non_scope_modal = tree.add_child(root, Box::new(NonScopeModalWidget::new()), None, 0);
    tree.set_rect(non_scope_modal, RectF::new(0.0, 0.0, 200.0, 100.0));

    // Non-scope modal child (focusable but should NOT be in scope)
    let non_scope_child = tree.add_child(
        non_scope_modal,
        Box::new(FocusableWidget::new("non_scope_child", events.clone())),
        None,
        0,
    );
    tree.set_rect(non_scope_child, RectF::new(0.0, 0.0, 80.0, 40.0));

    // Real scope modal (is_focus_scope = true)
    let scope_modal = tree.add_child(root, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(scope_modal, RectF::new(0.0, 150.0, 200.0, 100.0));

    // Scope modal child A
    let scope_child_a = tree.add_child(
        scope_modal,
        Box::new(FocusableWidget::new("scope_child_a", events.clone())),
        None,
        0,
    );
    tree.set_rect(scope_child_a, RectF::new(0.0, 0.0, 80.0, 40.0));
    let id_scope_child_a = tree.node(scope_child_a).unwrap().id();

    // Scope modal child B
    let scope_child_b = tree.add_child(
        scope_modal,
        Box::new(FocusableWidget::new("scope_child_b", events.clone())),
        None,
        0,
    );
    tree.set_rect(scope_child_b, RectF::new(90.0, 0.0, 80.0, 40.0));
    let id_scope_child_b = tree.node(scope_child_b).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Sync with BOTH modal nodes - only scope_modal should become a scope
    router.sync_focus_scopes(
        &mut tree,
        &[non_scope_modal, scope_modal],
        &mut domain_state,
    );

    // Verify only scope_modal is active scope
    assert_eq!(
        domain_state.focus_manager.active_scope(),
        Some(scope_modal),
        "Only is_focus_scope=true node should become active scope"
    );

    // Focus scope_child_a
    let click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(10.0, 160.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_scope_child_a));

    // Tab should cycle within scope_modal only
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_scope_child_b));

    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_scope_child_a),
        "Tab should wrap within scope, not leak to non_scope_modal"
    );
}

// ============================================================================
// Test 9: Action Palette Modal is Focus Scope
// ============================================================================

#[test]
fn test_action_palette_modal_is_focus_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    use halcyon::widgets::ActionPalette;

    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 800.0, 600.0));

    // Base widget (outside palette)
    let base_widget = tree.add_child(
        root,
        Box::new(FocusableWidget::new("base", events.clone())),
        None,
        0,
    );
    tree.set_rect(base_widget, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_base = tree.node(base_widget).unwrap().id();

    // ActionPalette - this is the real widget that should be a focus scope
    let palette_node = tree.add_child(root, Box::new(ActionPalette::new()), None, 0);
    tree.set_rect(palette_node, RectF::new(100.0, 100.0, 600.0, 400.0));

    // Verify ActionPalette returns is_focus_scope = true
    {
        let node = tree.node(palette_node).unwrap();
        assert!(
            node.widget().is_focus_scope(),
            "ActionPalette should return is_focus_scope() = true"
        );
    }

    // Add focusable children inside palette for testing
    let palette_child_a = tree.add_child(
        palette_node,
        Box::new(FocusableWidget::new("palette_a", events.clone())),
        None,
        0,
    );
    tree.set_rect(palette_child_a, RectF::new(10.0, 10.0, 80.0, 40.0));
    let id_palette_a = tree.node(palette_child_a).unwrap().id();

    let palette_child_b = tree.add_child(
        palette_node,
        Box::new(FocusableWidget::new("palette_b", events.clone())),
        None,
        0,
    );
    tree.set_rect(palette_child_b, RectF::new(100.0, 10.0, 80.0, 40.0));
    let id_palette_b = tree.node(palette_child_b).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Simulate palette modal open - sync with palette node
    router.sync_focus_scopes(&mut tree, &[palette_node], &mut domain_state);
    assert_eq!(
        domain_state.focus_manager.active_scope(),
        Some(palette_node),
        "ActionPalette should become active scope"
    );

    // Focus palette_child_a
    let click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(120.0, 120.0), // Inside palette_child_a at (110,110)
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_palette_a));

    // Tab should stay inside palette
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_palette_b));

    // Shift+Tab should wrap within palette
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_palette_a));

    // Simulate palette closing - empty modal list
    router.sync_focus_scopes(&mut tree, &[], &mut domain_state);
    assert_eq!(
        domain_state.focus_manager.active_scope(),
        None,
        "No scope after palette closes"
    );

    // Tab should now work globally
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_base),
        "After palette close, Tab should work globally"
    );
}

// ============================================================================
// Test 10: Sync Focus Scopes Filters Non-Scope Nodes
// ============================================================================

#[test]
fn test_sync_focus_scopes_filters_non_scope_nodes() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 300.0));

    // Node A: is_focus_scope = false
    let node_a = tree.add_child(root, Box::new(NonScopeModalWidget::new()), None, 0);
    tree.set_rect(node_a, RectF::new(0.0, 0.0, 100.0, 50.0));

    // Node B: is_focus_scope = true
    let node_b = tree.add_child(root, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(node_b, RectF::new(0.0, 100.0, 100.0, 50.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();

    // Pass both nodes to sync_focus_scopes
    router.sync_focus_scopes(&mut tree, &[node_a, node_b], &mut domain_state);

    // Only node_b should be in the scope stack
    assert_eq!(
        domain_state.focus_manager.active_scope(),
        Some(node_b),
        "Only is_focus_scope=true node should be active scope"
    );

    // Verify scope depth is 1 (only one scope)
    assert_eq!(
        domain_state.focus_manager.scope_depth(),
        1,
        "Only one scope should be pushed"
    );

    // Test filter_focus_scopes helper directly
    let filtered = tree.filter_focus_scopes(&[node_a, node_b]);
    assert_eq!(
        filtered.len(),
        1,
        "filter_focus_scopes should return only scope nodes"
    );
    assert_eq!(filtered[0], node_b, "Filtered result should contain node_b");
}

// ============================================================================
// SPEC-HAL-038 Stage 5: Directional Navigation Tests
// ============================================================================

/// Helper: Create arrow key event
fn arrow_key_event(key_code: KeyCode) -> FusedInputEvent {
    FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    }
}

/// Helper: Create ESC key event
fn esc_key_event() -> FusedInputEvent {
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

/// Helper: Send a complete click (PointerDown + PointerUp) to properly end gesture session.
/// SPEC-HAL-128 requires PointerUp to end gesture sessions before ESC can clear focus.
fn send_click(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    pos: Point,
    root: Option<NodeId>,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
) {
    let down = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        tree,
        down,
        root,
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    let up = FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    };
    router.route_input_event(
        tree,
        up,
        root,
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

/// Setup for directional navigation tests:
///   [A] [B] [C]   (y=0)
///   [D] [E] [F]   (y=100)
///   [G] [H] [I]   (y=200)
struct DirectionalTestSetup {
    tree: WidgetTree,
    root: NodeId,
    ids: Vec<TreeWidgetId>, // A=0, B=1, C=2, D=3, E=4, F=5, G=6, H=7, I=8
    nodes: Vec<NodeId>,
}

fn setup_directional_test() -> DirectionalTestSetup {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 400.0));

    let labels = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];
    let mut ids = Vec::new();
    let mut nodes = Vec::new();

    for (i, label) in labels.iter().enumerate() {
        let col = (i % 3) as f32;
        let row = (i / 3) as f32;
        let x = col * 110.0; // 100 width + 10 gap
        let y = row * 110.0; // 100 height + 10 gap

        let node = tree.add_child(
            root,
            Box::new(FocusableWidget::new(label, events.clone())),
            None,
            0,
        );
        tree.set_rect(node, RectF::new(x, y, 100.0, 100.0));
        ids.push(tree.node(node).unwrap().id());
        nodes.push(node);
    }

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    DirectionalTestSetup {
        tree,
        root,
        ids,
        nodes,
    }
}

// Test 11: Arrow keys move focus correctly
#[test]
fn test_arrow_keys_move_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_directional_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Click on E (center) to start
    let click_e = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(160.0, 160.0), // Center of E at (110,110)
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_e,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[4]),
        "Should focus E"
    );

    // Arrow Up: E → B
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Up),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[1]),
        "Up should move to B"
    );

    // Arrow Down: B → E
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Down),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[4]),
        "Down should move to E"
    );

    // Arrow Left: E → D
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Left),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[3]),
        "Left should move to D"
    );

    // Arrow Right: D → E
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Right),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[4]),
        "Right should move to E"
    );
}

// Test 12: No movement when no widget in direction
#[test]
fn test_arrow_no_movement_at_edge() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_directional_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Click on A (top-left corner)
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 50.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_a,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[0]),
        "Should focus A"
    );

    // Arrow Up: A has nothing above
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Up),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[0]),
        "Up from A should stay on A"
    );

    // Arrow Left: A has nothing to the left
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Left),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[0]),
        "Left from A should stay on A"
    );

    // Now test bottom-right corner (I)
    let click_i = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(270.0, 270.0), // Center of I at (220,220)
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_i,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[8]),
        "Should focus I"
    );

    // Arrow Down: I has nothing below
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Down),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[8]),
        "Down from I should stay on I"
    );

    // Arrow Right: I has nothing to the right
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Right),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.ids[8]),
        "Right from I should stay on I"
    );
}

// Test 13: Directional keys stay within scope
#[test]
fn test_directional_keys_stay_within_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push scope
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    // Click on scope_a
    let click_scope_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(10.0, 110.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_scope_a,
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));

    // Arrow Up: Should not move to global widgets
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Up),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_a),
        "Up should not escape scope to global widgets"
    );

    // Arrow Right: Should move to scope_b (within scope)
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Right),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_b),
        "Right should move to scope_b within scope"
    );

    // Arrow Left: Should move back to scope_a
    router.route_input_event(
        &mut tree,
        arrow_key_event(KeyCode::Left),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_scope_a),
        "Left should move back to scope_a"
    );
}

// ============================================================================
// SPEC-HAL-038 Stage 5: ESC Focus Reset Tests
// ============================================================================

// Test 14: ESC pops focus scope
#[test]
fn test_esc_pops_focus_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push scope and focus scope_a
    router.push_focus_scope(setup.scope_node, &mut domain_state);

    // SPEC-HAL-128: Use complete click (Down+Up) to end gesture session before ESC
    send_click(
        &mut router,
        &mut tree,
        Point::new(10.0, 110.0),
        Some(setup.root),
        &mut text_engine,
        &mut domain_state,
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(setup.id_scope_a));
    assert!(domain_state.focus_manager.active_scope().is_some());

    // ESC should pop the scope
    router.route_input_event(
        &mut tree,
        esc_key_event(),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    assert_eq!(
        domain_state.focus_manager.active_scope(),
        None,
        "ESC should pop the focus scope"
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should be cleared when scope is popped"
    );
}

// Test 15: ESC pops only top scope in nested scopes
#[test]
fn test_esc_pops_only_top_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 400.0));

    // Outer scope
    let outer_scope = tree.add_child(root, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(outer_scope, RectF::new(0.0, 100.0, 200.0, 200.0));

    // Inner scope (nested inside outer)
    let inner_scope = tree.add_child(outer_scope, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(inner_scope, RectF::new(10.0, 10.0, 150.0, 150.0));

    // Focusable inside inner scope
    let inner_child = tree.add_child(
        inner_scope,
        Box::new(FocusableWidget::new("inner", events.clone())),
        None,
        0,
    );
    tree.set_rect(inner_child, RectF::new(10.0, 10.0, 50.0, 50.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push both scopes
    router.push_focus_scope(outer_scope, &mut domain_state);
    router.push_focus_scope(inner_scope, &mut domain_state);
    assert_eq!(domain_state.focus_manager.scope_depth(), 2);
    assert_eq!(domain_state.focus_manager.active_scope(), Some(inner_scope));

    // First ESC pops inner scope
    router.route_input_event(
        &mut tree,
        esc_key_event(),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.scope_depth(), 1);
    assert_eq!(
        domain_state.focus_manager.active_scope(),
        Some(outer_scope),
        "First ESC should leave outer scope active"
    );

    // Second ESC pops outer scope
    router.route_input_event(
        &mut tree,
        esc_key_event(),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.scope_depth(), 0);
    assert_eq!(
        domain_state.focus_manager.active_scope(),
        None,
        "Second ESC should clear all scopes"
    );
}

// Test 16: ESC clears focus when no scope is active
#[test]
fn test_esc_clears_focus_when_no_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus global_a (no scope active)
    // SPEC-HAL-128: Use complete click (Down+Up) to end gesture session before ESC
    send_click(
        &mut router,
        &mut tree,
        Point::new(50.0, 25.0),
        Some(setup.root),
        &mut text_engine,
        &mut domain_state,
    );
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_global_a)
    );
    assert!(domain_state.focus_manager.active_scope().is_none());

    // ESC at root scope is a no-op for focus.
    router.route_input_event(
        &mut tree,
        esc_key_event(),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(setup.id_global_a),
        "ESC should preserve focus when no scope is active"
    );
}

// Test 17: ESC does nothing when no focus and no scope
#[test]
fn test_esc_does_nothing_when_no_focus_or_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let setup = setup_scope_test();
    let mut tree = setup.tree;
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // No focus, no scope
    assert!(domain_state.focus_manager.focused().is_none());
    assert!(domain_state.focus_manager.active_scope().is_none());

    // ESC should do nothing (not panic, return false internally)
    router.route_input_event(
        &mut tree,
        esc_key_event(),
        Some(setup.root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Still no focus, no scope
    assert!(domain_state.focus_manager.focused().is_none());
    assert!(domain_state.focus_manager.active_scope().is_none());
}

// ============================================================================
// SPEC-HAL-038 Stage 6: Keyboard Activation Tests
// ============================================================================

/// Activatable widget that tracks keyboard activations
struct ActivatableWidget {
    id: WidgetInternalId,
    activations: Rc<RefCell<Vec<&'static str>>>,
    label: &'static str,
}

impl ActivatableWidget {
    fn new(label: &'static str, activations: Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            activations,
            label,
        }
    }
}

impl WidgetDowncast for ActivatableWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ActivatableWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 50.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accepts_keyboard_activation(&self) -> bool {
        true
    }

    fn on_keyboard_activate(&mut self, _cx: &mut EventContext) {
        self.activations.borrow_mut().push(self.label);
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

/// Non-activatable focusable widget (accepts_keyboard_activation = false)
struct NonActivatableWidget {
    id: WidgetInternalId,
    activations: Rc<RefCell<Vec<&'static str>>>,
    label: &'static str,
}

impl NonActivatableWidget {
    fn new(label: &'static str, activations: Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            activations,
            label,
        }
    }
}

impl WidgetDowncast for NonActivatableWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for NonActivatableWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 50.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accepts_keyboard_activation(&self) -> bool {
        false // Does NOT accept keyboard activation
    }

    fn on_keyboard_activate(&mut self, _cx: &mut EventContext) {
        // Should never be called, but track it if it is
        self.activations.borrow_mut().push(self.label);
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

/// Helper: Create Enter key event
fn enter_key_event(repeat: bool) -> FusedInputEvent {
    FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Enter,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat,
    }
}

/// Helper: Create Space key event
fn space_key_event(repeat: bool) -> FusedInputEvent {
    FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Space,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat,
    }
}

// Test 18: Space activates focused widget
#[test]
fn test_space_activates_focused_widget() {
    let _guard = LayoutAuthorityGuard::enter();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    let widget_a = tree.add_child(
        root,
        Box::new(ActivatableWidget::new("A", activations.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Click to focus widget A
    let click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Press Space
    router.route_input_event(
        &mut tree,
        space_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Widget A should have been activated
    let activations = activations.borrow();
    assert_eq!(activations.len(), 1, "Should have one activation");
    assert_eq!(activations[0], "A", "Widget A should be activated");
}

// Test 19: Enter activates focused widget
#[test]
fn test_enter_activates_focused_widget() {
    let _guard = LayoutAuthorityGuard::enter();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    let widget_a = tree.add_child(
        root,
        Box::new(ActivatableWidget::new("A", activations.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Click to focus widget A
    let click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Press Enter
    router.route_input_event(
        &mut tree,
        enter_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Widget A should have been activated
    let activations = activations.borrow();
    assert_eq!(activations.len(), 1, "Should have one activation");
    assert_eq!(activations[0], "A", "Widget A should be activated");
}

// Test 20: No activation when widget doesn't accept keyboard activation
#[test]
fn test_no_activation_when_not_accepting() {
    let _guard = LayoutAuthorityGuard::enter();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // NonActivatableWidget: focusable but doesn't accept keyboard activation
    let widget_a = tree.add_child(
        root,
        Box::new(NonActivatableWidget::new("A", activations.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Click to focus widget A
    let click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Press Enter and Space
    router.route_input_event(
        &mut tree,
        enter_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    router.route_input_event(
        &mut tree,
        space_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // No activations should have occurred
    let activations = activations.borrow();
    assert_eq!(
        activations.len(),
        0,
        "Non-activatable widget should not be activated"
    );
}

// Test 21: Keyboard activation respects scope
#[test]
fn test_keyboard_activation_respects_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 300.0));

    // Global widget A
    let global_a = tree.add_child(
        root,
        Box::new(ActivatableWidget::new("global_A", activations.clone())),
        None,
        0,
    );
    tree.set_rect(global_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_global_a = tree.node(global_a).unwrap().id();

    // Scope container
    let scope_node = tree.add_child(root, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(scope_node, RectF::new(0.0, 100.0, 200.0, 100.0));

    // Widget B inside scope
    let scope_b = tree.add_child(
        scope_node,
        Box::new(ActivatableWidget::new("scope_B", activations.clone())),
        None,
        0,
    );
    tree.set_rect(scope_b, RectF::new(0.0, 0.0, 80.0, 40.0));
    let id_scope_b = tree.node(scope_b).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push scope and focus B
    router.push_focus_scope(scope_node, &mut domain_state);
    let click_b = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(40.0, 120.0), // Inside scope_b
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_b,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_scope_b));

    // Press Enter - should activate B
    router.route_input_event(
        &mut tree,
        enter_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    {
        let acts = activations.borrow();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0], "scope_B", "B inside scope should be activated");
    }

    // Pop scope (focus cleared)
    router.pop_focus_scope(&mut tree, &mut domain_state);
    assert!(domain_state.focus_manager.focused().is_none());

    // Focus global_a
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_a,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_global_a));

    // Press Space - should activate A
    router.route_input_event(
        &mut tree,
        space_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    {
        let acts = activations.borrow();
        assert_eq!(acts.len(), 2);
        assert_eq!(
            acts[1], "global_A",
            "A should be activated after scope closed"
        );
    }
}

// Test 22: No activation on key repeat
#[test]
fn test_no_repeat_activation() {
    let _guard = LayoutAuthorityGuard::enter();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    let widget_a = tree.add_child(
        root,
        Box::new(ActivatableWidget::new("A", activations.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Click to focus widget A
    let click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Initial press (repeat=false) - should activate
    router.route_input_event(
        &mut tree,
        enter_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Repeat presses (repeat=true) - should NOT activate
    router.route_input_event(
        &mut tree,
        enter_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    router.route_input_event(
        &mut tree,
        enter_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    router.route_input_event(
        &mut tree,
        enter_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Only one activation should have occurred
    let activations = activations.borrow();
    assert_eq!(
        activations.len(),
        1,
        "Repeat key events should not trigger activation"
    );
}

// Test 23: ESC and activation separation
#[test]
fn test_esc_and_activation_separation() {
    let _guard = LayoutAuthorityGuard::enter();
    let activations = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 300.0));

    // Widget inside scope
    let scope_node = tree.add_child(root, Box::new(ScopeWidget::new()), None, 0);
    tree.set_rect(scope_node, RectF::new(0.0, 100.0, 200.0, 100.0));

    let scope_a = tree.add_child(
        scope_node,
        Box::new(ActivatableWidget::new("scope_A", activations.clone())),
        None,
        0,
    );
    tree.set_rect(scope_a, RectF::new(0.0, 0.0, 80.0, 40.0));
    let id_scope_a = tree.node(scope_a).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Push scope and focus widget
    router.push_focus_scope(scope_node, &mut domain_state);
    // SPEC-HAL-128: Use complete click (Down+Up) to end gesture session before ESC
    send_click(
        &mut router,
        &mut tree,
        Point::new(40.0, 120.0),
        Some(root),
        &mut text_engine,
        &mut domain_state,
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_scope_a));

    // ESC pops scope and clears focus
    router.route_input_event(
        &mut tree,
        esc_key_event(),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert!(domain_state.focus_manager.focused().is_none());
    assert!(domain_state.focus_manager.active_scope().is_none());

    // Now Enter/Space should not activate anything (no focus)
    router.route_input_event(
        &mut tree,
        enter_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    router.route_input_event(
        &mut tree,
        space_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // No activations should have occurred
    let activations = activations.borrow();
    assert_eq!(activations.len(), 0, "No activation after ESC clears focus");
}
