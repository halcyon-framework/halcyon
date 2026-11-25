// SPEC-HAL-009: Event Routing Implementation
// SPEC-HAL-024 Stage 6: Uses absolute_paint_rect for all coordinate conversions
// SPEC-HAL-033: Input Fusion integration
//
// ============================================================================
// SPEC-HAL-079 input phases; SPEC-HAL-128 gesture arbitration
// ============================================================================
//
// Canonical fused pointer routes enforce the four input phases, with
// per-domain pointer sessions, soft/hard capture and lifecycle teardown.
// InteractionDomainState owns interaction state; GestureArbiter is stateless.
//
// HAL-128 commit requests are resolved during Routing: one winner captures and
// every other candidate is cancelled. PostProcess handles terminal cleanup.
// HAL-396 uses this same registry and capture lifecycle for a native touch pan.
// ============================================================================

use crate::core::drag_drop::DragSession;
use crate::core::event::{
    CancellationReason, CaptureMode, CursorIcon, Event, EventPhase, FusedInputEvent,
    GestureArbiter, GestureAxis, GestureCandidateId, GestureKind, InputPipelinePhase,
    InteractionDomainId, InteractionDomainState, KeyCode, KeyEvent, KeyboardAuthorityMode,
    Modifiers, PhysicalPressState, PointerButton, PointerEvent, PointerEventKind, PointerId,
    PointerRouteOrigin, PointerType, QualifiedTouchMotion, TouchPressIdentity, TouchScrollMotion,
    PendingTouchTap, TouchDragMotion, TouchHit, TouchScrollSource, TouchTap, WindowId,
};
use crate::core::layout::Axis;
use crate::core::math::{Point, RectF};
use crate::core::relative_pointer::{
    CursorRestoreOutcome, CursorRestorePolicy, PointerMotionPolicy, RelativePointerAdmissionError,
    RelativePointerCancellationReason, RelativePointerMotionEvent, RelativePointerTerminalOutcome,
};
use crate::core::relative_pointer_authority::{
    RelativePointerAdmissionFacts, RelativePointerHardCaptureFacts,
};
use crate::core::widget_tree::{NodeId, WidgetMountToken, WidgetTree};
use crate::utils::id::TreeWidgetId;
use crate::widgets::scroll_area::ScrollArea;
use std::cell::Cell;

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod browser_paste;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod browser_text;
#[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
pub(crate) mod host_choice;

/// SPEC-HAL-268: Double-tap Ctrl detection for proximity scroll
#[derive(Debug, Clone, Copy, PartialEq)]
enum CtrlDoubleTapState {
    /// No recent Ctrl activity
    Idle,
    /// First Ctrl released, waiting for second press
    WaitingSecondPress(web_time::Instant),
    /// Double-tap confirmed, Ctrl held
    Active,
}

// ============================================================================
// SPEC-HAL-038 Stage 5: Directional Navigation
// ============================================================================

/// Direction for arrow-key focus navigation and directional focus entry.
///
/// Used internally by the navigation algorithm and surfaced to widgets via
/// `EventContext::focus_entry_direction()` so composite widgets with
/// internal focus targets (RadioGroup, TabBar, etc.) can choose the
/// correct internal entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RelativePointerWindowFacts {
    pub window_id: WindowId,
    pub interaction_domain_id: InteractionDomainId,
    pub focused_window_id: Option<WindowId>,
    pub client_bounds: RectF,
}

struct TouchScrollRequest {
    candidate: GestureCandidateId,
    source: TouchScrollSource,
    delta: Point,
    sample: QualifiedTouchMotion,
}

/// HAL328 §20: a stack-local result of the original captured target dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapturedAppKeyOutcome {
    NotEligible,
    WidgetHandled,
    AppFallback,
}

/// Exact, invocation-local membership. No widget reference survives routing.
pub(crate) struct CapturedAppKeyTarget {
    owner: crate::core::widget_tree::WidgetMountToken,
    input_root: crate::core::widget_tree::WidgetMountToken,
    root: NodeId,
    domain: InteractionDomainId,
    window: WindowId,
}

impl CapturedAppKeyTarget {
    fn current_owner(
        tree: &WidgetTree,
        state: &InteractionDomainState,
        event: &FusedInputEvent,
        root: NodeId,
    ) -> Option<TreeWidgetId> {
        let FusedInputEvent::KeyDown {
            key_code,
            modifiers,
            reservation: None,
            ..
        } = event
        else {
            return None;
        };
        if !(modifiers.ctrl || modifiers.alt || modifiers.meta)
            || matches!(
                key_code,
                KeyCode::Escape | KeyCode::Control | KeyCode::Shift | KeyCode::Alt | KeyCode::Meta
            )
            || state.keyboard_mode != KeyboardAuthorityMode::Capture
        {
            return None;
        }
        let owner = state.focus_manager.focused()?;
        let node = tree.find_node_by_widget_id(owner)?;
        let widget = &tree.node(node)?.widget;
        (widget.capture_eligible()
            && !widget.physical_keyboard_capture()
            && tree.is_descendant_of_node(owner, root))
        .then_some(owner)
    }

    pub(crate) fn qualify(
        tree: &WidgetTree,
        state: &InteractionDomainState,
        input: &crate::core::event::SessionQualifiedInput,
        root: NodeId,
    ) -> Option<Self> {
        if input.registered_global_shortcut().is_some() {
            return None;
        }
        let owner = Self::current_owner(tree, state, input.event(), root)?;
        Some(Self {
            owner: tree.concrete_mount(owner)?,
            input_root: tree.concrete_mount(tree.node(root)?.id)?,
            root,
            domain: state.focus_manager.interaction_domain_id(),
            window: input.event().window_id(),
        })
    }

    pub(crate) fn is_current(
        &self,
        tree: &WidgetTree,
        state: &InteractionDomainState,
        event: &FusedInputEvent,
        root: NodeId,
    ) -> bool {
        self.root == root
            && self.domain == state.focus_manager.interaction_domain_id()
            && self.window == event.window_id()
            && self.owner.is_live()
            && self.input_root.is_live()
            && tree.concrete_mount(self.owner.owner()).as_ref() == Some(&self.owner)
            && tree
                .node(root)
                .and_then(|node| tree.concrete_mount(node.id))
                .as_ref()
                == Some(&self.input_root)
            && Self::current_owner(tree, state, event, root) == Some(self.owner.owner())
    }
}

#[derive(Debug, Clone, Copy)]
enum RelativePointerExit {
    Completed,
    Cancelled(RelativePointerCancellationReason),
}

/// Event router - coordinates event dispatch through the widget tree
/// SPEC-HAL-033 Phase 2: Click tracking removed - handled by InputFusion
/// SPEC-HAL-128 Stage 3: Added GestureArbiter for commit arbitration
///
/// SPEC-HAL-151: EventRouter is now STATELESS with respect to interaction state.
/// All interaction state (focus, capture, gestures, hover, scroll-capture) is
/// owned by InteractionDomainState and passed explicitly to routing methods.
///
/// INV-151-DOMAIN-ISOLATION: Interaction state MUST NOT leak between domains.
/// INV-151-SINGLE-DOMAIN-PER-EVENT: Each event routes to exactly one domain.
#[derive(Clone, Copy)]
enum CompositeLifecycle {
    Focus,
    Blur,
    CaptureEnter,
    CaptureLeave,
}

enum WidgetInput {
    Ordinary,
    TouchTap(TouchTap),
    #[cfg(any(target_arch = "wasm32", test))]
    BrowserText(browser_text::BrowserTextAction),
    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    HostChoice(crate::model::selection_resolver::OptionId),
}

pub struct EventRouter {
    composite_transaction_active: bool,
    #[cfg(any(target_arch = "wasm32", test))]
    touch_keyboard_activation: Option<(QualifiedTouchMotion, TreeWidgetId)>,
    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    host_choice: host_choice::HostChoice,
    /// SPEC-HAL-128 Stage 3: Gesture arbiter for commit decisions (stateless)
    gesture_arbiter: GestureArbiter,
    /// SPEC-HAL-101 Stage-4: Current frame time in ms for caret blinking
    frame_time_ms: u64,
    /// SPEC-HAL-056: Platform clipboard for copy/paste
    clipboard: Option<crate::platform::clipboard::Clipboard>,
    /// SPEC-HAL-131E: Last projected hit for keyboard routing continuity
    last_projected_hit: Option<crate::core::input_projection::ProjectedHitResult>,
    /// Current modifier state (updated from KeyDown/KeyUp events)
    current_modifiers: Modifiers,
    /// Focus reveal: Widget that gained focus this frame, needs scroll reveal post-layout.
    /// Set by request_focus_for_widget(), consumed by post_layout_reveal_focused_widget().
    pending_focus_reveal: Option<TreeWidgetId>,
    /// Stage 42G: Continuous frame pump requested by a widget.
    ///
    /// INV-42G-3 (Active Resize Frame Pump): While true, the app MUST schedule
    /// continuous frames independent of further input.
    ///
    /// Set by dispatch_to_widget when EventContext::continuous_frames_requested() is true.
    /// Checked by app after event dispatch to schedule continuous redraws.
    continuous_frames_requested: bool,
    /// Cursor icon requested by a widget during this frame.
    ///
    /// INV-CURSOR-EPHEMERAL: Reset at start of each frame, last request wins.
    /// Checked by app after event dispatch to update window cursor.
    requested_cursor: Option<CursorIcon>,
    /// SPEC-HAL-356: Widget whose active pointer capture owns sticky hidden cursor state.
    cursor_hidden_owner: Option<TreeWidgetId>,
    /// SPEC-HAL-356: Visibility restoration that must survive a frame-boundary reset.
    cursor_restore_pending: bool,
    /// SPEC-HAL-079: Current input pipeline phase.
    ///
    /// Tracks which phase the router is currently in for phase guards.
    /// None when not processing an event.
    ///
    /// INV-079-PHASE-ORDER: Phases MUST execute in order.
    /// INV-079-NO-DISPATCH-PREHITTEST: Widget dispatch forbidden during PreHitTest/HitTest.
    /// INV-079-NO-HITTEST-ROUTING: Hit-test forbidden during Routing/PostProcess.
    current_phase: Cell<Option<InputPipelinePhase>>,
    /// SPEC-HAL-268: Double-tap Ctrl state machine for proximity scroll
    ctrl_double_tap: CtrlDoubleTapState,
    /// SPEC-HAL-268: Sticky cursor-hidden state for proximity scroll.
    /// When true, cursor stays hidden across events that don't explicitly request a cursor.
    pub(crate) cursor_hidden_sticky: bool,
    /// SPEC-HAL-326 §12.x INV-F9: One-shot side channel for directional
    /// focus entry. Set by `request_focus_for_widget_directional` before
    /// the focus mutation; consumed (taken) by `request_focus_for_widget`
    /// when constructing the `on_focus` EventContext. Cleared after every
    /// focus dispatch so non-directional callers always see None.
    pending_focus_entry_direction: Option<Direction>,
    relative_window_facts: Cell<Option<RelativePointerWindowFacts>>,
    geometry_capture_changes: Vec<(crate::core::widget_tree::GeometryCausalCutoff, TreeWidgetId)>,
}

impl EventRouter {
    pub fn new() -> Self {
        Self {
            composite_transaction_active: false,
            #[cfg(any(target_arch = "wasm32", test))]
            touch_keyboard_activation: None,
            #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
            host_choice: host_choice::HostChoice::default(),
            gesture_arbiter: GestureArbiter::new(),
            frame_time_ms: 0,
            clipboard: crate::platform::clipboard::Clipboard::new(),
            last_projected_hit: None,
            current_modifiers: Modifiers::default(),
            pending_focus_reveal: None,
            // Stage 42G: No continuous frames requested initially
            continuous_frames_requested: false,
            // INV-CURSOR-EPHEMERAL: No cursor requested initially
            requested_cursor: None,
            // SPEC-HAL-356: No captured drag owns hidden cursor state initially
            cursor_hidden_owner: None,
            cursor_restore_pending: false,
            // SPEC-HAL-079: No phase active initially
            current_phase: Cell::new(None),
            // SPEC-HAL-268: No double-tap activity
            ctrl_double_tap: CtrlDoubleTapState::Idle,
            // SPEC-HAL-268: Cursor not hidden initially
            cursor_hidden_sticky: false,
            // SPEC-HAL-326 §12.x INV-F9: No pending directional entry tag
            pending_focus_entry_direction: None,
            relative_window_facts: Cell::new(None),
            geometry_capture_changes: Vec::new(),
        }
    }

    pub(crate) fn relative_ingress_gate(
        domain_state: &InteractionDomainState,
    ) -> Option<crate::core::relative_pointer::RelativePointerIngressGate> {
        domain_state
            .relative_pointer_authority
            .as_ref()?
            .active()
            .map(|activation| activation.ingress_gate())
    }

    pub(crate) fn claims_relative_event(
        event: &FusedInputEvent,
        domain_state: &InteractionDomainState,
    ) -> bool {
        let Some(binding) = domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
            .map(|activation| activation.binding())
        else {
            return false;
        };
        match event {
            FusedInputEvent::RelativePointerMotion {
                lease_id,
                window_id,
                pointer_id,
                ..
            } => {
                *lease_id == binding.lease_id()
                    && *window_id == binding.window_id()
                    && *pointer_id == binding.pointer_id()
            }
            FusedInputEvent::DragEnd {
                window_id, button, ..
            }
            | FusedInputEvent::PointerUp {
                window_id, button, ..
            } => *window_id == binding.window_id() && *button == binding.pointer_button(),
            _ => false,
        }
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn route_input_event_with_relative_facts(
        &mut self,
        tree: &mut WidgetTree,
        event: FusedInputEvent,
        root: Option<NodeId>,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
        binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
        facts: RelativePointerWindowFacts,
    ) {
        self.relative_window_facts.set(Some(facts));
        self.route_input_event(
            tree,
            event,
            root,
            text_engine,
            animation,
            domain_state,
            binding_registry,
        );
        self.relative_window_facts.set(None);
    }

    fn terminate_relative_pointer(
        &mut self,
        mut tree: Option<&mut WidgetTree>,
        domain_state: &mut InteractionDomainState,
        exit: RelativePointerExit,
        facts: Option<RelativePointerWindowFacts>,
    ) -> bool {
        let Some(activation) = domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
        else {
            return false;
        };
        let binding = activation.binding();
        let pointer_id = binding.pointer_id();
        let kind = activation.gesture_kind();
        let may_restore = matches!(
            exit,
            RelativePointerExit::Completed
                | RelativePointerExit::Cancelled(
                    RelativePointerCancellationReason::ExplicitSemanticCancel
                )
        );
        let safe_to_restore = may_restore
            && binding.restore_policy() == CursorRestorePolicy::DownAnchor
            && facts.is_some_and(|current| {
                current.window_id == binding.window_id()
                    && current.interaction_domain_id == binding.interaction_domain_id()
                    && current.focused_window_id == Some(binding.window_id())
                    && binding.down_anchor().is_some_and(|anchor| {
                        anchor.x.is_finite()
                            && anchor.y.is_finite()
                            && current.client_bounds.contains(anchor)
                    })
            })
            && domain_state
                .relative_pointer_authority
                .as_ref()
                .is_some_and(|authority| authority.restoration_supported(binding));
        let Some((_, cleanup)) = domain_state
            .relative_pointer_authority
            .as_mut()
            .and_then(|authority| authority.terminate(binding.lease_id(), safe_to_restore))
        else {
            return false;
        };
        if !cleanup.is_clean() || cleanup.restore() == CursorRestoreOutcome::FailedPlatformOperation
        {
            tracing::error!(?cleanup, "relative pointer lease cleanup failed");
        }
        let outcome = match exit {
            RelativePointerExit::Completed => RelativePointerTerminalOutcome::Completed {
                restore: cleanup.restore(),
            },
            RelativePointerExit::Cancelled(reason) => RelativePointerTerminalOutcome::Cancelled {
                reason,
                restore: cleanup.restore(),
            },
        };
        if let Some(tree) = tree.as_deref_mut() {
            if let Some(node_id) = tree.find_node_by_widget_id(binding.capture_owner()) {
                let tree_ptr = tree as *mut WidgetTree;
                let clipboard_ptr = self.clipboard_ptr();
                if let Some(node) = tree.node_mut(node_id) {
                    let mut cx = crate::core::widget::EventContext::new(
                        binding.capture_owner(),
                        tree_ptr,
                        self.frame_time_ms,
                        clipboard_ptr,
                        None,
                        None,
                        false,
                        false,
                        domain_state.keyboard_mode,
                    );
                    node.widget
                        .on_relative_pointer_terminal(&mut cx, kind, outcome);
                }
            }
        }
        self.restore_hidden_cursor_for_widget(binding.capture_owner());
        domain_state
            .pointer_capture_manager
            .cancel_capture(pointer_id);
        domain_state.pointer_capture.release();
        domain_state.gesture_registry.clear_candidates(pointer_id);
        true
    }

    fn deny_relative_candidate(
        &mut self,
        tree: &mut WidgetTree,
        pointer_id: PointerId,
        winner_widget: TreeWidgetId,
        winner_kind: crate::core::event::GestureKind,
        error: RelativePointerAdmissionError,
        domain_state: &mut InteractionDomainState,
    ) {
        if let Some(node_id) = tree.find_node_by_widget_id(winner_widget) {
            let tree_ptr = tree as *mut WidgetTree;
            let clipboard_ptr = self.clipboard_ptr();
            if let Some(node) = tree.node_mut(node_id) {
                let mut cx = crate::core::widget::EventContext::new(
                    winner_widget,
                    tree_ptr,
                    self.frame_time_ms,
                    clipboard_ptr,
                    None,
                    None,
                    false,
                    false,
                    domain_state.keyboard_mode,
                );
                node.widget.on_relative_pointer_terminal(
                    &mut cx,
                    winner_kind,
                    RelativePointerTerminalOutcome::AdmissionDenied(error),
                );
            }
        }
        domain_state.gesture_registry.clear_candidates(pointer_id);
        self.cancel_pointer_session_with_tree(
            tree,
            pointer_id,
            CancellationReason::SystemCancel,
            domain_state,
        );
    }

    fn notify_gesture_losers(
        &mut self,
        tree: &mut WidgetTree,
        losers: &[(TreeWidgetId, crate::core::event::GestureKind)],
        domain_state: &InteractionDomainState,
        touch_press: Option<TouchPressIdentity>,
    ) {
        let tree_ptr = tree as *mut WidgetTree;
        let clipboard_ptr = self.clipboard_ptr();
        for (loser_widget, loser_kind) in losers {
            tree.geometry_protected_owners.remove(loser_widget);
            if let Some(node_id) = tree.find_node_by_widget_id(*loser_widget) {
                if let Some(node) = tree.node_mut(node_id) {
                    let mut cx = crate::core::widget::EventContext::new(
                        *loser_widget,
                        tree_ptr,
                        self.frame_time_ms,
                        clipboard_ptr,
                        None,
                        None,
                        false,
                        false,
                        domain_state.keyboard_mode,
                    );
                    cx.set_touch_gesture_press(touch_press);
                    node.widget.on_gesture_cancel(&mut cx, *loser_kind);
                }
            }
        }
    }

    pub(crate) fn cancel_active_relative_with_tree(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
        reason: RelativePointerCancellationReason,
        facts: Option<RelativePointerWindowFacts>,
    ) -> bool {
        self.terminate_relative_pointer(
            Some(tree),
            domain_state,
            RelativePointerExit::Cancelled(reason),
            facts,
        )
    }

    /// Consume Screen::update semantic intents after destroyed owners have been
    /// processed. A request only affects the currently bound owner and pointer.
    pub(crate) fn drain_relative_pointer_semantic_cancels(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
        facts: Option<RelativePointerWindowFacts>,
    ) -> bool {
        let mut cancelled = false;
        for (owner, pointer_id) in tree.drain_relative_pointer_semantic_cancels() {
            let matches_active_binding = domain_state
                .relative_pointer_authority
                .as_ref()
                .and_then(|authority| authority.active())
                .is_some_and(|activation| {
                    let binding = activation.binding();
                    binding.capture_owner() == owner && binding.pointer_id() == pointer_id
                });
            if matches_active_binding && tree.find_node_by_widget_id(owner).is_some() {
                cancelled |= self.cancel_active_relative_with_tree(
                    tree,
                    domain_state,
                    RelativePointerCancellationReason::ExplicitSemanticCancel,
                    facts,
                );
            }
        }
        cancelled
    }

    // =========================================================================
    // SPEC-HAL-079 Phase-2: Pipeline Enforcement & Observable Routing
    // =========================================================================

    /// Enter a pipeline phase with observability.
    ///
    /// INV-079-PHASE-ORDER: Phases MUST execute in order: PreHitTest → HitTest → Routing → PostProcess.
    /// INV-079-SINGLE-OWNER: Only EventRouter may change phases.
    #[inline]
    fn enter_phase(&self, phase: InputPipelinePhase, _pointer_id: PointerId, _event_kind: &str) {
        #[cfg(debug_assertions)]
        {
            let current = self.current_phase.get();
            debug_assert!(
                current.is_none(),
                "SPEC-079 VIOLATION: Entering {:?} while already in {:?}",
                phase,
                current
            );
            tracing::trace!(
                target: "input.phase",
                pointer_id = ?_pointer_id,
                event_kind = _event_kind,
                "enter = {:?}",
                phase
            );
        }
        self.current_phase.set(Some(phase));
    }

    /// Exit the current pipeline phase with observability.
    ///
    /// INV-079-PHASE-ORDER: Must exit the expected phase.
    #[inline]
    fn exit_phase(&self, _expected: InputPipelinePhase, _pointer_id: PointerId, _event_kind: &str) {
        #[cfg(debug_assertions)]
        {
            let current = self.current_phase.get();
            debug_assert!(
                current == Some(_expected),
                "SPEC-079 VIOLATION: Exiting {:?} but current phase is {:?}",
                _expected,
                current
            );
            tracing::trace!(
                target: "input.phase",
                pointer_id = ?_pointer_id,
                event_kind = _event_kind,
                "exit = {:?}",
                _expected
            );
        }
        self.current_phase.set(None);
    }

    /// Assert we're in the expected phase (debug-only).
    ///
    /// INV-079-PHASE-GUARD: Operations must occur in correct phase.
    #[inline]
    #[allow(
        dead_code,
        reason = "Retained input phase assertion at the router authority boundary"
    )]
    fn assert_phase(&self, _expected: InputPipelinePhase) {
        #[cfg(debug_assertions)]
        {
            let current = self.current_phase.get();
            debug_assert!(
                current == Some(_expected),
                "SPEC-079 VIOLATION: Expected phase {:?} but current is {:?}",
                _expected,
                current
            );
        }
    }

    /// Assert we're in Routing phase (for widget dispatch and capture mutations).
    ///
    /// INV-079-NO-DISPATCH-OUTSIDE-ROUTING: Widget dispatch forbidden outside Routing.
    /// INV-079-NO-CAPTURE-OUTSIDE-ROUTING: Capture mutations forbidden outside Routing.
    #[inline]
    fn assert_routing_phase(&self) {
        #[cfg(debug_assertions)]
        {
            let current = self.current_phase.get();
            debug_assert!(
                current == Some(InputPipelinePhase::Routing),
                "SPEC-079 VIOLATION: Operation requires Routing phase but current is {:?}",
                current
            );
        }
    }

    /// Assert we're in HitTest phase (for hit-test operations).
    ///
    /// INV-079-NO-HITTEST-OUTSIDE-HITTEST: Hit-test forbidden outside HitTest phase.
    #[inline]
    #[allow(
        dead_code,
        reason = "Retained input phase assertion at the router authority boundary"
    )]
    fn assert_hittest_phase(&self) {
        #[cfg(debug_assertions)]
        {
            let current = self.current_phase.get();
            debug_assert!(
                current == Some(InputPipelinePhase::HitTest),
                "SPEC-079 VIOLATION: Hit-test requires HitTest phase but current is {:?}",
                current
            );
        }
    }

    /// Complete remaining phases on early return.
    ///
    /// INV-079-NO-SKIP-PHASES: No early returns that skip exit_phase.
    /// When returning early, we must still complete all 4 phases.
    #[inline]
    fn complete_remaining_phases(
        &self,
        current: InputPipelinePhase,
        pointer_id: PointerId,
        event_kind: &str,
    ) {
        // Exit current phase
        self.exit_phase(current, pointer_id, event_kind);

        // Complete any remaining phases
        match current {
            InputPipelinePhase::PreHitTest => {
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }
            InputPipelinePhase::HitTest => {
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }
            InputPipelinePhase::Routing => {
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }
            InputPipelinePhase::PostProcess => {
                // Already at last phase, nothing more to complete
            }
        }
    }

    /// SPEC-HAL-101 Stage-4: Set the current frame time before routing events.
    pub fn set_frame_time_ms(&mut self, frame_time_ms: u64) {
        self.frame_time_ms = frame_time_ms;
    }

    pub(crate) fn frame_time_ms(&self) -> u64 {
        self.frame_time_ms
    }

    /// Take the pending focus reveal widget ID (if any).
    ///
    /// Returns the widget that gained focus this frame and needs scroll reveal.
    /// The value is consumed (set to None) after taking.
    /// Called by post_layout_reveal_focused_widget() in the frame pipeline.
    pub fn take_pending_focus_reveal(&mut self) -> Option<TreeWidgetId> {
        self.pending_focus_reveal.take()
    }

    /// Stage 42G: Check if any widget requested continuous frames.
    ///
    /// INV-42G-3 (Active Resize Frame Pump): When true, the app MUST schedule
    /// continuous frames independent of further input.
    ///
    /// Called by app after event dispatch to determine frame scheduling.
    pub fn continuous_frames_requested(&self) -> bool {
        self.continuous_frames_requested
    }

    /// Stage 42G: Reset the continuous frames flag.
    ///
    /// INV-42G-3: Called at start of each frame to reset the flag.
    /// Widgets must re-request continuous frames each frame if needed.
    pub fn reset_continuous_frames_request(&mut self) {
        self.continuous_frames_requested = false;
    }

    /// SPEC-HAL-328 §15: framework guarantee — after a mode transition
    /// (Capture↔Navigation), focus change, or scope push/pop, request
    /// a continuous frame so the visual update lands immediately even
    /// if the widget's lifecycle callbacks forgot to request one.
    pub fn request_continuous_frames_after_mode_transition(&mut self) {
        self.continuous_frames_requested = true;
    }

    /// Take the requested cursor icon, if any.
    ///
    /// INV-CURSOR-EPHEMERAL: Cursor requests are per-frame. The value is
    /// consumed (set to None) after taking.
    ///
    /// Called by app after event dispatch to update window cursor.
    pub fn take_requested_cursor(&mut self) -> Option<CursorIcon> {
        if self.cursor_restore_pending {
            self.cursor_restore_pending = false;
            self.requested_cursor = None;
            Some(CursorIcon::Default)
        } else {
            self.requested_cursor.take()
        }
    }

    /// Reset cursor request at start of frame.
    ///
    /// INV-CURSOR-EPHEMERAL: Cursor must be re-requested each frame.
    /// Called at start of each frame alongside reset_continuous_frames_request.
    pub fn reset_cursor_request(&mut self) {
        self.requested_cursor = None;
    }

    fn record_cursor_request(
        &mut self,
        widget_id: TreeWidgetId,
        cursor: CursorIcon,
        domain_state: &InteractionDomainState,
    ) {
        if cursor == CursorIcon::Hidden {
            if domain_state
                .pointer_capture_manager
                .is_captured_by(PointerId::MOUSE, widget_id)
            {
                self.cursor_hidden_owner = Some(widget_id);
                self.cursor_restore_pending = false;
                self.requested_cursor = Some(CursorIcon::Hidden);
            }
            return;
        }

        if self
            .cursor_hidden_owner
            .is_some_and(|owner| owner != widget_id)
        {
            return;
        }

        self.cursor_hidden_owner = None;
        self.cursor_restore_pending = false;
        self.requested_cursor = Some(cursor);
    }

    fn restore_hidden_cursor_for_widget(&mut self, widget_id: TreeWidgetId) {
        if self.cursor_hidden_owner == Some(widget_id) {
            self.cursor_hidden_owner = None;
            self.cursor_restore_pending = true;
            self.requested_cursor = Some(CursorIcon::Default);
        }
    }

    fn restore_hidden_cursor_for_capture(
        &mut self,
        pointer_id: PointerId,
        domain_state: &InteractionDomainState,
    ) {
        if pointer_id != PointerId::MOUSE {
            return;
        }
        if let Some(widget_id) = domain_state
            .pointer_capture_manager
            .current_capture(pointer_id)
        {
            self.restore_hidden_cursor_for_widget(widget_id);
        }
    }

    fn process_lifecycle_requests(
        &mut self,
        widget_id: TreeWidgetId,
        cx: &crate::core::widget::EventContext<'_>,
        domain_state: &mut InteractionDomainState,
    ) {
        if cx.is_release_requested() {
            domain_state
                .pointer_capture_manager
                .release_capture(PointerId::MOUSE, widget_id);
            if domain_state.pointer_capture.captured_widget == Some(widget_id) {
                domain_state.pointer_capture.release();
            }
            self.restore_hidden_cursor_for_widget(widget_id);
        }
        if cx.continuous_frames_requested() {
            self.continuous_frames_requested = true;
        }
        if let Some(cursor) = cx.cursor_requested() {
            self.record_cursor_request(widget_id, cursor, domain_state);
        }
    }

    /// SPEC-HAL-268: Whether Ctrl was double-tapped and is held (proximity scroll trigger)
    fn is_ctrl_double_tap_active(&self) -> bool {
        matches!(self.ctrl_double_tap, CtrlDoubleTapState::Active)
    }

    /// SPEC-HAL-056: Get clipboard pointer for EventContext
    fn clipboard_ptr(&mut self) -> *mut crate::platform::clipboard::Clipboard {
        self.clipboard
            .as_mut()
            .map(|c| c as *mut _)
            .unwrap_or(std::ptr::null_mut())
    }

    // =========================================================================
    // DEPRECATED PATH — Lifecycle-correct, do not extend
    // =========================================================================
    //
    // route_pointer_event is the legacy entry point before InputFusion.
    // It still has correct capture lifecycle (teardown on PointerUp), but:
    //   - Does NOT use four-phase pipeline
    //   - Does NOT have phase guards
    //   - Should NOT be extended with new features
    //
    // All new work uses route_input_event with FusedInputEvent.
    // =========================================================================

    /// Route a pointer event through the widget tree (DEPRECATED)
    ///
    /// SPEC-HAL-009 Section 6.1: Pointer Event Routing
    /// SPEC-HAL-024 Stage 6: Uses absolute_paint_rect for local_pos conversion
    ///
    /// Pointer capture semantics:
    /// - On PointerDown: hit-test to find target, set capture to that widget
    /// - On Move/Up while captured: route ONLY to captured widget (no hit-test)
    /// - On PointerUp: auto-release capture after delivery
    /// - Scroll/Wheel: never capture, always hit-test
    ///
    /// SPEC-HAL-128 Stage 3.H: Single resolution point - gesture commit resolution happens
    /// exactly once at the end of this function, after all dispatch phases complete.
    ///
    /// SPEC-HAL-151: Accepts domain state explicitly for interaction isolation.
    #[deprecated(
        since = "0.1.3",
        note = "Use route_input_event with FusedInputEvent instead"
    )]
    pub fn route_pointer_event(
        &mut self,
        tree: &mut WidgetTree,
        event: PointerEvent,
        domain_state: &mut InteractionDomainState,
    ) {
        self.validate_capture_target(tree, PointerId::MOUSE, domain_state);

        // SPEC-HAL-128 Stage 3.H: Track if this is an Up event for session teardown
        let is_up = matches!(event.kind, PointerEventKind::Up);

        match event.kind {
            // Scroll events: never capture, always hit-test
            // Scroll doesn't participate in gesture system - return early
            PointerEventKind::Scroll { .. } => {
                self.route_scroll_event(tree, event, domain_state);
                return;
            }

            // Down event: hit-test to find target, capture will be set by widget
            // SPEC-HAL-038 Stage 8: If capture is active, route Down to captured widget
            // SPEC-HAL-128: Use PointerCaptureManager for capture state
            // SPEC-HAL-128 Stage 2.1: Begin gesture session before widget handles Down
            // SPEC-HAL-128 Stage 7: Pass pointer_type for semantic binding
            // Stage 45B: DragStart routes to captured widget for resize activation
            PointerEventKind::Down
            | PointerEventKind::DoubleClick
            | PointerEventKind::TripleClick
            | PointerEventKind::DragStart => {
                // SPEC-HAL-128 Stage 2.1: Begin session ONLY on physical PointerDown
                // DoubleClick/TripleClick/DragStart are semantic derivatives, not lifecycle boundaries
                // SPEC-HAL-128 Stage 7 INV-S7-P1: pointer_type is explicit and required
                if matches!(event.kind, PointerEventKind::Down) {
                    domain_state
                        .gesture_registry
                        .begin_session(PointerId::MOUSE, event.pointer_type);
                }

                if let Some(captured_id) = domain_state
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE)
                {
                    // SPEC-HAL-038 Stage 8: During capture, route Down to captured widget
                    // This prevents other widgets from stealing capture or receiving input
                    // Compute local_pos relative to captured widget
                    // SPEC-HAL-141: Use getter for private geometry field
                    let mut local_event = event.clone();
                    if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            local_event.local_pos = (
                                event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                                event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                            );
                        }
                    }

                    // Deprecated method: pass None for text_engine_ptr
                    self.route_to_widget(
                        tree,
                        captured_id,
                        &Event::Pointer(local_event),
                        None,
                        domain_state,
                        None,
                    );
                } else {
                    // No capture: normal hit-test routing
                    let (global_x, global_y) = event.global_pos;
                    let hit = tree.hit_test_for_routing(global_x, global_y);
                    if matches!(event.kind, PointerEventKind::Down)
                        && event.pointer_type != PointerType::Touch
                        && event.button == Some(PointerButton::Primary)
                    {
                        tree.dismiss_nonmodal_popup_for_press(
                            Point::new(global_x, global_y),
                            hit.as_ref().map(|(hit, _)| hit.widget_id),
                        );
                    }
                    if let Some((hit_result, route_origin)) = hit {
                        let target_id = hit_result.widget_id;

                        // SPEC-HAL-087 Stage 2: Verify hit-test returned a resolvable ID
                        #[cfg(debug_assertions)]
                        debug_assert!(
                            tree.verify_tree_widget_id(target_id),
                            "SPEC-HAL-087: hit_test returned invalid TreeWidgetId({})",
                            target_id.0
                        );

                        // SPEC-HAL-038 Stage 7: PointerDown implicitly sets hover to clicked widget
                        // SPEC-HAL-021 Stage 4: No animation for click-driven hover (click != hover)
                        self.update_hover(tree, Some(target_id), None, domain_state);

                        // Build event path and dispatch through phases
                        let event_path = self.build_event_path(tree, target_id);

                        // SPEC-HAL-128 Stage 8: Register candidates from widget declarations
                        // INV-GS2: Widgets declare, system decides
                        // INV-GS3: Registration is the only widget action
                        if matches!(event.kind, PointerEventKind::Down) {
                            self.register_candidates_from_declarations(
                                tree,
                                &event_path,
                                PointerId::MOUSE,
                                domain_state,
                                false,
                            );
                        }

                        // Deprecated method: pass None for text_engine_ptr
                        self.dispatch_pointer_event(
                            tree,
                            &event_path,
                            event.clone(),
                            route_origin,
                            None,
                            domain_state,
                            None,
                        );
                    }
                }
            }

            // Move/Up events: if captured, route ONLY to captured widget
            // SPEC-HAL-128: Use PointerCaptureManager for capture state
            // SPEC-HAL-128 Stage 5 INV-PC-UP: PointerUp is delivered BEFORE teardown
            // SPEC-HAL-128 Stage 5 INV-PC6: Guaranteed delivery to captured widget
            PointerEventKind::Move | PointerEventKind::Up => {
                if let Some(captured_id) = domain_state
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE)
                {
                    // SPEC-HAL-128 Stage 5 INV-PC6: Verify captured widget still exists
                    // If destroyed, cancel session and skip delivery
                    if tree.find_node_by_widget_id(captured_id).is_none() {
                        tracing::warn!(
                            "SPEC-HAL-128 Stage 5: Captured widget {:?} destroyed, cancelling session",
                            captured_id.0
                        );
                        self.cancel_pointer_session(
                            PointerId::MOUSE,
                            CancellationReason::WidgetDestroyed,
                            domain_state,
                        );
                        // Fall through to non-captured handling
                    } else {
                        // Exclusive capture: route directly to captured widget, no hit-testing
                        // Compute local_pos relative to captured widget
                        // SPEC-HAL-086 Stage 7: Account for scroll offset when computing local_pos
                        // SPEC-HAL-141: Use getter for private geometry field
                        let mut local_event = event.clone();
                        if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                            // Get cumulative scroll offset from all ancestor scroll containers
                            let scroll_offset = tree.cumulative_scroll_offset(node_id);

                            if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                                // Transform global_pos to content coordinates, then compute local
                                // global_pos is screen coords, abs_rect is unscrolled content coords
                                // content_pos = global_pos + scroll_offset
                                // local_pos = content_pos - abs_rect
                                local_event.local_pos = (
                                    event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                                    event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                                );
                            }
                        }

                        // Dispatch to captured widget
                        // INV-PC6: Guaranteed delivery to captured widget (even outside window)
                        // INV-PC-UP: PointerUp delivered here, BEFORE teardown at end of function
                        // Deprecated method: pass None for text_engine_ptr
                        self.route_to_widget(
                            tree,
                            captured_id,
                            &Event::Pointer(local_event),
                            None,
                            domain_state,
                            None,
                        );
                    }
                } else {
                    // No capture: hit-test for Move (hover tracking), drop Up events
                    // SPEC-HAL-021 Stage 4: This deprecated path passes None for animation.
                    // Proper animation injection happens in route_input_event for PointerMove.
                    if matches!(event.kind, PointerEventKind::Move) {
                        let (global_x, global_y) = event.global_pos;
                        if let Some((hit_result, route_origin)) =
                            tree.hit_test_for_routing(global_x, global_y)
                        {
                            let target_id = hit_result.widget_id;

                            // SPEC-HAL-038 Stage 7: Update hover state with lifecycle calls
                            // SPEC-HAL-021 Stage 4: No animation in deprecated path
                            self.update_hover(tree, Some(target_id), None, domain_state);

                            // Dispatch hover event
                            let event_path = self.build_event_path(tree, target_id);
                            // Deprecated method: pass None for text_engine_ptr
                            self.dispatch_pointer_event(
                                tree,
                                &event_path,
                                event.clone(),
                                route_origin,
                                None,
                                domain_state,
                                None,
                            );
                        } else {
                            // SPEC-HAL-038 Stage 7: Pointer moved outside all widgets, clear hover
                            // SPEC-HAL-021 Stage 4: No animation in deprecated path
                            self.update_hover(tree, None, None, domain_state);
                        }
                    }
                    // PointerUp without capture: drop (no widget to receive it)
                }
            }
        }

        // SPEC-HAL-128 Stage 3.H: Single resolution point - resolve all pending gesture commits
        // after full dispatch (all phases, all widgets). This is the ONLY place resolution occurs.
        // SPEC-HAL-128 Stage 8: Pass tree for commit/cancel callbacks
        self.resolve_gesture_commits_for_pointer(tree, PointerId::MOUSE, domain_state);

        // SPEC-HAL-128 Stage 3.H: Session teardown on PointerUp - AFTER resolution
        // SPEC-HAL-128 Stage 5 INV-LIFE-ORDER: Teardown order is:
        //   1. Deliver PointerUp (if captured) - done above in dispatch
        //   2. Resolve commit (if applicable) - done above in resolve_gesture_commits
        //   3. Release capture - done below
        //   4. Clear gesture session - done below
        //   5. Destroy widget (if applicable) - not handled here, caller responsibility
        // INV-PC3: Capture ends on pointer up
        // INV-PC-UP: PointerUp was delivered BEFORE we reach this point
        // INV-GA-TEARDOWN: pending_commits cleared with session (no stale state survives)
        if is_up {
            tracing::info!(
                "SPEC-HAL-128: Session teardown after PointerUp (INV-LIFE-ORDER steps 3-4)"
            );
            self.restore_hidden_cursor_for_capture(PointerId::MOUSE, domain_state);
            domain_state
                .pointer_capture_manager
                .cancel_capture(PointerId::MOUSE);
            domain_state.pointer_capture.release();
            domain_state
                .gesture_registry
                .clear_candidates(PointerId::MOUSE);
        }
    }

    /// Route scroll events (never captured, always hit-test)
    fn route_scroll_event(
        &mut self,
        tree: &mut WidgetTree,
        event: PointerEvent,
        domain_state: &mut InteractionDomainState,
    ) {
        if domain_state.touch_scroll_source().is_some() {
            return;
        }
        let (global_x, global_y) = event.global_pos;
        if let Some((hit_result, route_origin)) = tree.hit_test_for_routing(global_x, global_y) {
            let target_id = hit_result.widget_id;

            // SPEC-HAL-087 Stage 2: Verify hit-test returned a resolvable ID
            #[cfg(debug_assertions)]
            debug_assert!(
                tree.verify_tree_widget_id(target_id),
                "SPEC-HAL-087: scroll hit_test returned invalid TreeWidgetId({})",
                target_id.0
            );

            let event_path = self.build_event_path(tree, target_id);
            // Deprecated method: pass None for text_engine_ptr
            self.dispatch_pointer_event(
                tree,
                &event_path,
                event,
                route_origin,
                None,
                domain_state,
                None,
            );
        }
    }

    /// Route a key event to the focused widget
    /// SPEC-HAL-009 Section 6.2: Keyboard Event Routing
    #[deprecated(
        since = "0.1.3",
        note = "Use route_input_event with FusedInputEvent instead"
    )]
    pub fn route_key_event(
        &mut self,
        tree: &mut WidgetTree,
        event: KeyEvent,
        domain_state: &mut InteractionDomainState,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) {
        self.route_focused_key_event(tree, event, domain_state, animation);
    }

    fn route_focused_key_event(
        &mut self,
        tree: &mut WidgetTree,
        event: KeyEvent,
        domain_state: &mut InteractionDomainState,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) {
        if let Some(focused_id) = domain_state.focus_manager.focused() {
            self.route_to_widget(
                tree,
                focused_id,
                &Event::Key(event),
                None,
                domain_state,
                animation,
            );
        }
        // If no widget is focused, key events are dropped
        // TODO: Could add global key handler here
    }

    fn clear_focused_read_only_selection_on_outside_down(
        &mut self,
        tree: &mut WidgetTree,
        hit_target: Option<TreeWidgetId>,
        domain_state: &InteractionDomainState,
    ) {
        let Some(focused_id) = domain_state.focus_manager.focused() else {
            return;
        };
        let Some(focused_node_id) = tree.find_node_by_widget_id(focused_id) else {
            return;
        };

        let inside_focused = hit_target
            .map(|target| tree.is_descendant_of_node(target, focused_node_id))
            .unwrap_or(false);
        if inside_focused {
            return;
        }

        if let Some(node) = tree.node_mut(focused_node_id) {
            if node.widget.clear_read_only_text_selection_on_outside_down() {
                self.continuous_frames_requested = true;
            }
        }
    }

    /// Build path from root to target widget
    fn build_event_path(&self, tree: &WidgetTree, target_id: TreeWidgetId) -> Vec<TreeWidgetId> {
        let mut path = Vec::new();

        // Find the NodeId for this TreeWidgetId
        if let Some(target_node_id) = tree.find_node_by_widget_id(target_id) {
            let mut current = Some(target_node_id);

            // Walk up to root, collecting widget IDs
            while let Some(node_id) = current {
                if let Some(node) = tree.node(node_id) {
                    path.push(node.id);
                    current = node.parent;
                } else {
                    break;
                }
            }
        }

        // Reverse to get root-to-target order
        path.reverse();
        path
    }

    /// Dispatch pointer event through capture/target/bubble phases
    /// SPEC-HAL-024 Stage 6: Uses absolute_paint_rect for local_pos computation
    ///
    /// INV-INPUT-PROJ-1: Input projection uses the same scroll offset as paint.
    /// INV-INPUT-PROJ-2: Widgets inside scroll containers receive correct local coords.
    /// INV-INPUT-PROJ-3: No double-application - scroll offset applied exactly once per widget.
    fn dispatch_pointer_event<'te>(
        &mut self,
        tree: &mut WidgetTree,
        event_path: &[TreeWidgetId],
        event: PointerEvent,
        route_origin: PointerRouteOrigin,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
        mut animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) {
        if event_path.is_empty() {
            return;
        }

        // Special handling for scroll events: bubble to first widget that wants_scroll()
        if matches!(event.kind, PointerEventKind::Scroll { .. }) {
            // Search from target (end) to root (start) for a widget that wants scroll
            for i in (0..event_path.len()).rev() {
                let widget_id = event_path[i];
                if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
                    let wants_scroll = if let Some(node) = tree.node(node_id) {
                        node.widget.wants_scroll()
                    } else {
                        false
                    };

                    if wants_scroll {
                        // SPEC-HAL-024 Stage 6: Use absolute_paint_rect for local_pos
                        // INV-INPUT-PROJ-1: Apply cumulative scroll offset for correct coords
                        // SPEC-HAL-141: Use getter for private geometry field
                        let mut local_event = event.clone();
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            let scroll_offset = tree.cumulative_scroll_offset(node_id);
                            local_event.local_pos = (
                                event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                                event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                            );
                        }
                        self.dispatch_to_widget_with_pointer_origin(
                            tree,
                            widget_id,
                            &Event::Pointer(local_event),
                            EventPhase::Target,
                            route_origin,
                            text_engine_ptr,
                            domain_state,
                            animation.take(),
                        );
                        return;
                    }
                }
            }
            // No widget wants scroll, drop event
            return;
        }

        let target_idx = event_path.len() - 1;

        // ===== CAPTURE PHASE =====
        // Root → parent of target
        //
        // INV-42G-1 (Capture Path Correctness): For any pointer event dispatched
        // to a descendant widget, EventPhase::Capture MUST invoke handle_event()
        // on every ancestor from root → target.
        //
        // INV-42I-CAPTURE-COORDS-1 (Capture Phase Visual-Local): In capture phase,
        // local_pos MUST be computed as visual-local (global - abs_rect.origin).
        // Scroll offset is NOT applied because capture-phase widgets receive events
        // in their own coordinate space, not scrolled content space.
        for i in 0..target_idx {
            let widget_id = event_path[i];
            // Stage 42I: Capture phase uses visual-local coords (NO scroll offset).
            // Scroll offset is only for target/bubble into scrolled content.
            // SPEC-HAL-141: Use getter for private geometry field
            let mut local_event = event.clone();
            if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
                if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                    // INV-42I-CAPTURE-COORDS-1: NO scroll offset for capture phase
                    local_event.local_pos = (
                        event.global_pos.0 - abs_rect.x,
                        event.global_pos.1 - abs_rect.y,
                    );
                }
            }
            if self.dispatch_to_widget_with_pointer_origin(
                tree,
                widget_id,
                &Event::Pointer(local_event),
                EventPhase::Capture,
                route_origin,
                text_engine_ptr,
                domain_state,
                None,
            ) {
                return; // Event consumed
            }
        }

        // ===== TARGET PHASE =====
        let target_id = event_path[target_idx];
        // SPEC-HAL-024 Stage 6: Use absolute_paint_rect for local_pos
        // INV-INPUT-PROJ-1: Apply cumulative scroll offset for correct coords
        // SPEC-HAL-141: Use getter for private geometry field
        let mut local_event = event.clone();
        if let Some(node_id) = tree.find_node_by_widget_id(target_id) {
            if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                let scroll_offset = tree.cumulative_scroll_offset(node_id);
                local_event.local_pos = (
                    event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                    event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                );
            }
        }
        if self.dispatch_to_widget_with_pointer_origin(
            tree,
            target_id,
            &Event::Pointer(local_event),
            EventPhase::Target,
            route_origin,
            text_engine_ptr,
            domain_state,
            animation.take(),
        ) {
            return; // Event consumed
        }

        // ===== BUBBLE PHASE =====
        // Parent of target → root
        if target_idx > 0 {
            for i in (0..target_idx).rev() {
                let widget_id = event_path[i];
                // SPEC-HAL-024 Stage 6: Use absolute_paint_rect for local_pos
                // INV-INPUT-PROJ-1: Apply cumulative scroll offset for correct coords
                // SPEC-HAL-141: Use getter for private geometry field
                let mut local_event = event.clone();
                if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
                    if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        local_event.local_pos = (
                            event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                            event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                        );
                    }
                }
                if self.dispatch_to_widget_with_pointer_origin(
                    tree,
                    widget_id,
                    &Event::Pointer(local_event),
                    EventPhase::Bubble,
                    route_origin,
                    text_engine_ptr,
                    domain_state,
                    None,
                ) {
                    return; // Event consumed
                }
            }
        }
    }

    // =========================================================================
    // SPEC-HAL-157: Capture-First Routing Authority
    //
    // INV-157-1: Capture phase executes before projected routing on PointerDown.
    // INV-157-2: If capture consumes, projected routing MUST NOT run.
    // INV-157-3: Capture is not re-run later in the same event.
    // INV-157-4: Behavior is generic and widget-agnostic.
    // INV-157-5: No silent fallbacks or partial routing.
    //
    // These helper functions split capture from target/bubble so that capture
    // phase can run BEFORE projected routing on PointerDown.
    // =========================================================================

    /// Run capture phase only (root → parent of target).
    /// Returns true if any widget consumed the event.
    ///
    /// SPEC-HAL-157: Called BEFORE projected routing so that ancestors can
    /// claim interaction authority before virtualized item routing occurs.
    fn dispatch_capture_phase_only<'te>(
        &mut self,
        tree: &mut WidgetTree,
        event_path: &[TreeWidgetId],
        event: PointerEvent,
        route_origin: PointerRouteOrigin,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
    ) -> bool {
        if event_path.is_empty() {
            return false;
        }

        let target_idx = event_path.len() - 1;

        // Capture phase: root → parent of target
        // INV-42I-CAPTURE-COORDS-1: NO scroll offset for capture phase
        // SPEC-HAL-141: Use getter for private geometry field
        for i in 0..target_idx {
            let widget_id = event_path[i];
            let mut local_event = event.clone();
            if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
                if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                    // Visual-local coords (no scroll offset) for capture phase
                    local_event.local_pos = (
                        event.global_pos.0 - abs_rect.x,
                        event.global_pos.1 - abs_rect.y,
                    );
                }
            }
            if self.dispatch_to_widget_with_pointer_origin(
                tree,
                widget_id,
                &Event::Pointer(local_event),
                EventPhase::Capture,
                route_origin,
                text_engine_ptr,
                domain_state,
                None,
            ) {
                return true; // Event consumed
            }
        }

        false
    }

    /// Run target and bubble phases only (skipping capture).
    /// Used after capture phase has already run separately.
    ///
    /// SPEC-HAL-157 (INV-157-3): Called AFTER projected routing when capture
    /// did not consume. Capture phase already ran before projected routing.
    fn dispatch_target_and_bubble<'te>(
        &mut self,
        tree: &mut WidgetTree,
        event_path: &[TreeWidgetId],
        event: PointerEvent,
        route_origin: PointerRouteOrigin,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
        mut animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) {
        if event_path.is_empty() {
            return;
        }

        // Special handling for scroll events (unchanged from dispatch_pointer_event)
        if matches!(event.kind, PointerEventKind::Scroll { .. }) {
            for i in (0..event_path.len()).rev() {
                let widget_id = event_path[i];
                if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
                    let wants_scroll = if let Some(node) = tree.node(node_id) {
                        node.widget.wants_scroll()
                    } else {
                        false
                    };

                    if wants_scroll {
                        // SPEC-HAL-141: Use getter for private geometry field
                        let mut local_event = event.clone();
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            let scroll_offset = tree.cumulative_scroll_offset(node_id);
                            local_event.local_pos = (
                                event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                                event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                            );
                        }
                        self.dispatch_to_widget_with_pointer_origin(
                            tree,
                            widget_id,
                            &Event::Pointer(local_event),
                            EventPhase::Target,
                            route_origin,
                            text_engine_ptr,
                            domain_state,
                            animation.take(),
                        );
                        return;
                    }
                }
            }
            return;
        }

        let target_idx = event_path.len() - 1;

        // ===== TARGET PHASE =====
        // SPEC-HAL-141: Use getter for private geometry field
        let target_id = event_path[target_idx];
        let mut local_event = event.clone();
        if let Some(node_id) = tree.find_node_by_widget_id(target_id) {
            if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                let scroll_offset = tree.cumulative_scroll_offset(node_id);
                local_event.local_pos = (
                    event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                    event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                );
            }
        }
        if self.dispatch_to_widget_with_pointer_origin(
            tree,
            target_id,
            &Event::Pointer(local_event),
            EventPhase::Target,
            route_origin,
            text_engine_ptr,
            domain_state,
            animation.take(),
        ) {
            return;
        }

        // ===== BUBBLE PHASE =====
        // SPEC-HAL-141: Use getter for private geometry field
        if target_idx > 0 {
            for i in (0..target_idx).rev() {
                let widget_id = event_path[i];
                let mut local_event = event.clone();
                if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
                    if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        local_event.local_pos = (
                            event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                            event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                        );
                    }
                }
                if self.dispatch_to_widget_with_pointer_origin(
                    tree,
                    widget_id,
                    &Event::Pointer(local_event),
                    EventPhase::Bubble,
                    route_origin,
                    text_engine_ptr,
                    domain_state,
                    None,
                ) {
                    return;
                }
            }
        }
    }

    /// Dispatch event to a single widget
    /// Returns true if event was handled (stops propagation)
    ///
    /// INV-079-NO-DISPATCH-OUTSIDE-ROUTING: Widget dispatch forbidden outside Routing phase.
    fn dispatch_to_widget<'te>(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        event: &Event,
        phase: EventPhase,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) -> bool {
        self.dispatch_to_widget_with_pointer_origin(
            tree,
            widget_id,
            event,
            phase,
            PointerRouteOrigin::Regular,
            text_engine_ptr,
            domain_state,
            animation,
        )
    }

    fn dispatch_to_widget_with_pointer_origin<'te>(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        event: &Event,
        phase: EventPhase,
        route_origin: PointerRouteOrigin,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) -> bool {
        self.dispatch_widget_input(
            tree, widget_id, Some(event), phase, route_origin, text_engine_ptr,
            domain_state, animation, WidgetInput::Ordinary,
        )
    }

    fn dispatch_widget_input<'te>(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        event: Option<&Event>,
        phase: EventPhase,
        route_origin: PointerRouteOrigin,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
        mut animation: Option<crate::core::animation::AnimationContext<'_>>,
        input: WidgetInput,
    ) -> bool {
        use crate::core::composite_text_edit::{CompositeEditOrigin, CompositeEditOutcome};
        use crate::core::widget::EventContext;
        let origin = match &input {
            #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
            WidgetInput::HostChoice(_) => CompositeEditOrigin::HostChoice,
            _ => CompositeEditOrigin::CurrentInput,
        };
        self.assert_routing_phase();
        let Some(node_id) = tree.find_node_by_widget_id(widget_id) else {
            return false;
        };
        if !matches!(event, Some(Event::Focus(_) | Event::Window(_) | Event::Ime(crate::core::event::ImeEvent::Cancel)))
            && tree.node(node_id).is_some_and(|node| !tree.composite_choice_input_ready(node))
        {
            return false;
        }
        let pointer_id = match event {
            Some(Event::Pointer(p)) => p.id,
            _ => PointerId::MOUSE,
        };
        let is_scroll =
            matches!(event, Some(Event::Pointer(p)) if matches!(p.kind, PointerEventKind::Scroll { .. }));
        let has_capture = domain_state
            .pointer_capture_manager
            .current_capture(pointer_id)
            == Some(widget_id)
            || (is_scroll && domain_state.scroll_capture_owner == Some(widget_id));
        let combo_popup_hit = if phase == EventPhase::Target
            && matches!(event, Some(Event::Pointer(_) | Event::Key(_)))
        {
            tree.combo_popup_input_geometry(widget_id)
        } else {
            None
        };
        let scrollbar_rail_point = if let Some(Event::Pointer(pointer)) = event
            && matches!(pointer.kind, PointerEventKind::Move | PointerEventKind::Up)
            && pointer.pointer_type != PointerType::Touch
            && (phase == EventPhase::Target
                || (phase == EventPhase::Capture && route_origin == PointerRouteOrigin::Regular))
            && tree
                .node(node_id)
                .and_then(|node| node.widget.as_any().downcast_ref::<ScrollArea>())
                .is_some_and(ScrollArea::auto_hides_scrollbars)
        {
            tree.hit_test_candidate(node_id, Point::new(pointer.global_pos.0, pointer.global_pos.1))
                .map(|hit| Point::new(hit.local_pos.0, hit.local_pos.1))
        } else {
            None
        };
        let tree_ptr = tree as *mut WidgetTree;
        let clipboard = self.clipboard_ptr();
        // End the complete host context and Widget borrow before child lookup.
        let (mut handled, mut request, mut effects) = {
            let Some(node) = tree.node_mut(node_id) else {
                return false;
            };
            if matches!(&input, WidgetInput::Ordinary)
                && matches!(event, Some(Event::Pointer(p)) if p.pointer_type == PointerType::Touch)
                && !node.widget.accepts_touch_pointer_input()
            {
                return false;
            }
            crate::core::deferred_wiring::assert_sealed_for_phase(node.widget.as_ref(), "event");
            let mut cx = EventContext::new(
                widget_id,
                tree_ptr,
                self.frame_time_ms,
                clipboard,
                text_engine_ptr.map(|p| unsafe { &mut *p }),
                animation.as_mut().map(|a| a.reborrow()),
                has_capture,
                self.is_ctrl_double_tap_active(),
                domain_state.keyboard_mode,
            );
            if let (WidgetInput::Ordinary, Some(Event::Pointer(pointer))) = (&input, event) {
                cx.set_routed_click_count(pointer.kind);
            }
            cx.set_pointer_route_origin(route_origin);
            cx.set_combo_popup_hit(combo_popup_hit);
            cx.set_scrollbar_rail_point(scrollbar_rail_point);
            let handled = match input {
                WidgetInput::Ordinary => node.widget.handle_event(
                    &mut cx, event.expect("ordinary widget dispatch requires an event"), phase,
                ),
                WidgetInput::TouchTap(tap) => node.widget.on_touch_tap(&mut cx, tap),
                #[cfg(any(target_arch = "wasm32", test))]
                WidgetInput::BrowserText(action) => {
                    browser_text::dispatch_text(node.widget.as_mut(), &mut cx, action)
                }
                #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
                WidgetInput::HostChoice(option) => node.widget.as_any_mut()
                    .downcast_mut::<crate::widgets::combo_box::ComboBox>()
                    .is_some_and(|combo| combo.select_host_option(&mut cx, option)),
            };
            (
                handled,
                cx.take_composite_edit(),
                cx.take_dispatch_effects(),
            )
        };
        if request.is_some() {
            // Only ordinary prerequisite host activation precedes child work.
            // Explicit exit and redirect retain precedence.
            if effects.focus_redirect.is_none() && !effects.keyboard_capture_release_requested {
                if effects.focus_requested {
                    self.request_focus_with_text_engine(
                        tree,
                        widget_id,
                        domain_state,
                        text_engine_ptr.map(|p| unsafe { &mut *p }),
                    );
                    effects.focus_requested = false;
                }
                let pointer_entry = phase == EventPhase::Target
                    && route_origin != PointerRouteOrigin::Overlay
                    && !has_capture
                    && matches!(event, Some(Event::Pointer(p)) if p.kind == PointerEventKind::Down && p.button == Some(PointerButton::Primary));
                if domain_state.focus_manager.focused() == Some(widget_id)
                    && domain_state.keyboard_mode != KeyboardAuthorityMode::Capture
                    && (effects.keyboard_capture_requested || pointer_entry)
                    && tree
                        .node(node_id)
                        .is_some_and(|n| n.widget.capture_eligible())
                {
                    domain_state.keyboard_mode = KeyboardAuthorityMode::Capture;
                    let entry = self.invoke_composite_lifecycle(
                        tree,
                        widget_id,
                        CompositeLifecycle::CaptureEnter,
                        domain_state,
                        text_engine_ptr,
                        false,
                        true,
                    );
                    effects.keyboard_capture_requested = false;
                    self.continuous_frames_requested = true;
                    if let Some(entry) = entry {
                        request = match crate::widgets::composite_edit_host::combine_entry_request(
                            entry,
                            request.take().expect("current composite request"),
                        ) {
                            Ok(combined) => Some(combined),
                            Err((outcome, current)) => {
                                tracing::error!(?outcome, "invalid composite entry combination");
                                let reply = self.resume_composite_failure(
                                    tree,
                                    widget_id,
                                    current,
                                    outcome,
                                    domain_state,
                                    text_engine_ptr,
                                    &mut animation,
                                    &mut effects,
                                );
                                handled = reply.handled;
                                None
                            }
                        };
                    }
                }
            }
            if let Some(request) = request {
                match tree.composite_editor_id(widget_id) {
                    Ok(id) => {
                        let reply = self.run_composite_request(
                            tree,
                            &id,
                            request,
                            origin,
                            event,
                            route_origin,
                            domain_state,
                            text_engine_ptr,
                            &mut animation,
                            &mut effects,
                        );
                        // Request admission was only provisional. The completed
                        // transaction decides consumption, including whether a
                        // paired native TextInput must be suppressed.
                        handled = reply.handled;
                    }
                    Err(outcome) => {
                        let reply = self.resume_composite_failure(
                            tree,
                            widget_id,
                            request,
                            outcome,
                            domain_state,
                            text_engine_ptr,
                            &mut animation,
                            &mut effects,
                        );
                        handled = reply.handled || outcome == CompositeEditOutcome::DeniedConsume;
                    }
                }
            }
        }
        self.finish_widget_dispatch(
            tree,
            node_id,
            widget_id,
            event,
            phase,
            route_origin,
            text_engine_ptr,
            domain_state,
            has_capture,
            handled,
            effects,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_widget_dispatch<'te>(
        &mut self,
        tree: &mut WidgetTree,
        node_id: NodeId,
        widget_id: TreeWidgetId,
        event: Option<&Event>,
        phase: EventPhase,
        route_origin: PointerRouteOrigin,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
        has_capture: bool,
        handled: bool,
        effects: crate::core::widget::EventContextEffects,
    ) -> bool {
        let pointer_id = match event {
            Some(Event::Pointer(p)) => p.id,
            _ => PointerId::MOUSE,
        };
        let mut relative_release = None;
        // Process capture/focus requests from widget
        // SPEC-HAL-128: Use new PointerCaptureManager alongside legacy PointerCapture
        // SPEC-HAL-079: Support CaptureMode (Soft vs Hard)
        if let Some(button) = effects.capture_requested {
            // INV-079-NO-CAPTURE-OUTSIDE-ROUTING: Capture request MUST be in Routing phase
            #[cfg(debug_assertions)]
            debug_assert!(
                self.current_phase.get() == Some(InputPipelinePhase::Routing),
                "INV-079-NO-CAPTURE-OUTSIDE-ROUTING: capture request outside Routing phase"
            );
            // INV-PC1: Explicit capture request from widget
            let mode = effects.capture_mode;
            if let Err(e) = domain_state
                .pointer_capture_manager
                .request_capture_with_mode(pointer_id, widget_id, mode)
            {
                tracing::warn!(
                    "SPEC-HAL-128: Capture request denied for {:?}: {:?}",
                    widget_id.0,
                    e
                );
            } else if pointer_id == PointerId::MOUSE {
                // Keep legacy pointer_capture in sync
                domain_state.pointer_capture.capture(widget_id, button);
            } else if matches!(event, Some(Event::Pointer(p)) if p.pointer_type == PointerType::Touch && p.kind == PointerEventKind::Down)
                && let Some(press) = domain_state.gesture_registry.physical_press(pointer_id)
                && let Some(mount) = tree.concrete_mount(widget_id)
            {
                domain_state
                    .pointer_capture_manager
                    .qualify_touch_capture(press.touch_identity(), mount);
            }
        }
        if effects.release_capture {
            // INV-079-WIDGET-RELEASE-IN-ROUTING: Widget-explicit release in Routing phase
            #[cfg(debug_assertions)]
            debug_assert!(
                self.current_phase.get() == Some(InputPipelinePhase::Routing),
                "INV-079-WIDGET-RELEASE-IN-ROUTING: widget release outside Routing phase"
            );
            let active_relative_binding = domain_state
                .relative_pointer_authority
                .as_ref()
                .and_then(|authority| authority.active())
                .map(|activation| activation.binding())
                .filter(|binding| {
                    binding.pointer_id() == pointer_id && binding.capture_owner() == widget_id
                });
            if let Some(binding) = active_relative_binding {
                relative_release = Some(
                    if matches!(
                        event,
                        Some(Event::Pointer(PointerEvent {
                            kind: PointerEventKind::Up,
                            button: Some(button),
                            ..
                        })) if *button == binding.pointer_button()
                    ) {
                        RelativePointerExit::Completed
                    } else {
                        RelativePointerExit::Cancelled(
                            RelativePointerCancellationReason::ExplicitSemanticCancel,
                        )
                    },
                );
            } else {
                // INV-PC3: Explicit release from widget
                domain_state
                    .pointer_capture_manager
                    .release_capture(pointer_id, widget_id);
                if pointer_id == PointerId::MOUSE {
                    domain_state.pointer_capture.release();
                }
                self.restore_hidden_cursor_for_widget(widget_id);
            }
        }
        if let Some(redirect_id) = effects.focus_redirect {
            // Cross-widget focus transfer with proper lifecycle.
            self.request_focus_with_text_engine(
                tree,
                redirect_id,
                domain_state,
                text_engine_ptr.map(|p| unsafe { &mut *p }),
            );

            // Adjust keyboard mode for the target widget.
            if let Some(target_node) = tree.find_node_by_widget_id(redirect_id) {
                let eligible = tree
                    .node(target_node)
                    .is_some_and(|node| {
                        node.widget.capture_eligible() && tree.composite_choice_input_ready(node)
                    });
                if eligible {
                    // Target supports Capture — enter it.
                    domain_state.keyboard_mode = crate::core::event::KeyboardAuthorityMode::Capture;
                    self.invoke_composite_lifecycle(
                        tree,
                        redirect_id,
                        CompositeLifecycle::CaptureEnter,
                        domain_state,
                        text_engine_ptr,
                        false,
                        false,
                    );
                } else {
                    // Target doesn't support Capture — release to Navigation.
                    if domain_state.keyboard_mode
                        == crate::core::event::KeyboardAuthorityMode::Capture
                    {
                        domain_state.keyboard_mode =
                            crate::core::event::KeyboardAuthorityMode::Navigation;
                        // on_capture_leave for the OLD widget is handled by
                        // request_focus_for_widget's on_blur callback — the
                        // old widget clears its internal capture state there.
                    }
                }
            }
        } else if effects.focus_requested {
            self.request_focus_with_text_engine(
                tree,
                widget_id,
                domain_state,
                text_engine_ptr.map(|p| unsafe { &mut *p }),
            );
        }
        if effects.keyboard_capture_requested {
            // SPEC-HAL-176 INV-176-MODE-2: A request_keyboard_capture
            // when the domain is ALREADY in Capture is not a
            // transition. Skip on_capture_enter to avoid spurious
            // re-entries that break widgets with embedded sub-widgets
            // (e.g., ComboBox + TextField: TextField's Pointer Down
            // requests capture, and without this guard the parent's
            // on_capture_enter re-fires and may resurrect transient
            // state like a closing popup).
            if domain_state.keyboard_mode != crate::core::event::KeyboardAuthorityMode::Capture {
                // Widget requested direct Capture entry on click.
                // Only honour if the widget is capture_eligible.
                let eligible = tree
                    .node(node_id)
                    .map_or(false, |n| n.widget.capture_eligible());
                if eligible {
                    domain_state.keyboard_mode = crate::core::event::KeyboardAuthorityMode::Capture;
                    // Notify widget it entered Capture
                    self.invoke_composite_lifecycle(
                        tree,
                        widget_id,
                        CompositeLifecycle::CaptureEnter,
                        domain_state,
                        text_engine_ptr,
                        false,
                        false,
                    );
                    tracing::debug!(
                        "SPEC-HAL-176: Capture entered via request_keyboard_capture for widget {:?}",
                        widget_id
                    );
                }
            }
        }

        if effects.keyboard_capture_release_requested {
            if domain_state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture {
                domain_state.keyboard_mode = crate::core::event::KeyboardAuthorityMode::Navigation;
                self.invoke_composite_lifecycle(
                    tree,
                    widget_id,
                    CompositeLifecycle::CaptureLeave,
                    domain_state,
                    text_engine_ptr,
                    false,
                    false,
                );
                // SPEC-HAL-328 §15: framework guarantees a continuous
                // frame on mode transition.
                self.continuous_frames_requested = true;
            }
        }

        // Pointer edit intent follows target directives so entry cannot
        // preempt a control's toggle or undo an explicit capture release.
        if phase == EventPhase::Target
            && route_origin != PointerRouteOrigin::Overlay
            && !has_capture
            && matches!(event, Some(Event::Pointer(pe)) if pe.kind == PointerEventKind::Down && pe.button == Some(PointerButton::Primary))
            && domain_state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Navigation
            && domain_state.focus_manager.focused() == Some(widget_id)
            && effects.focus_redirect.is_none()
            && !effects.keyboard_capture_release_requested
            && tree
                .node(node_id)
                .is_some_and(|n| n.widget.capture_eligible())
        {
            self.handle_engine_action(
                tree,
                crate::runtime::actions::ActionId::ENTER_WIDGET,
                None,
                domain_state,
                text_engine_ptr
                    .map(|p| unsafe { &mut *p })
                    .expect("routed pointer entry requires text engine"),
                None,
            );
        }

        // SPEC-HAL-128 Stage 3 Fix: Queue commit requests for later resolution
        // Commit requests are collected during dispatch and resolved once per pointer
        // after all widgets have processed the event (priority-based arbitration).
        if effects.gesture_commit {
            if let Some(candidate) = domain_state
                .gesture_registry
                .get_candidate_for_widget(pointer_id, widget_id)
            {
                let candidate_id = candidate.candidate_id;
                domain_state
                    .gesture_registry
                    .push_commit_request(pointer_id, candidate_id);
            } else {
                tracing::warn!(
                    "SPEC-HAL-128 Stage 3: Commit requested by widget {:?} but no candidate found",
                    widget_id.0
                );
            }
        }

        // Stage 42G: Check if continuous frames were requested.
        // INV-42G-3: Propagate to event router for app frame scheduling.
        if effects.continuous_frames {
            self.continuous_frames_requested = true;
        }

        // INV-CURSOR-EPHEMERAL: Propagate cursor request to event router.
        // Last widget to request wins.
        if let Some(cursor) = effects.cursor {
            self.record_cursor_request(widget_id, cursor, domain_state);
        }

        if handled {
            tree.mark_paint_dirty(node_id);
        }

        if let Some(exit) = relative_release {
            self.terminate_relative_pointer(
                Some(tree),
                domain_state,
                exit,
                self.relative_window_facts.get(),
            );
        }
        handled
    }

    #[allow(clippy::too_many_arguments)]
    fn resume_composite_failure<'te>(
        &mut self,
        tree: &mut WidgetTree,
        owner: TreeWidgetId,
        request: crate::core::composite_text_edit::CompositeEditRequest,
        outcome: crate::core::composite_text_edit::CompositeEditOutcome,
        state: &mut InteractionDomainState,
        text_engine: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        animation: &mut Option<crate::core::animation::AnimationContext<'_>>,
        effects: &mut crate::core::widget::EventContextEffects,
    ) -> crate::core::composite_text_edit::CompositeEditReply {
        let mut reply = crate::core::composite_text_edit::CompositeEditReply::failure(outcome);
        let tree_ptr = tree as *mut WidgetTree;
        let clipboard = self.clipboard_ptr();
        if let Some(node) = tree
            .find_node_by_widget_id(owner)
            .and_then(|node| tree.node_mut(node))
        {
            let mut cx = crate::core::widget::EventContext::new(
                owner,
                tree_ptr,
                self.frame_time_ms,
                clipboard,
                text_engine.map(|p| unsafe { &mut *p }),
                animation.as_mut().map(|a| a.reborrow()),
                false,
                false,
                state.keyboard_mode,
            );
            if let crate::widgets::composite_edit_host::CompositeHostStep::Done { handled } =
                crate::widgets::composite_edit_host::resume(
                    node.widget.as_mut(),
                    &mut cx,
                    request.continuation,
                    reply.clone(),
                )
            {
                reply.handled = handled;
            }
            effects.merge(cx.take_dispatch_effects());
        }
        reply
    }

    #[allow(clippy::too_many_arguments)]
    fn run_composite_child_scope<'te>(
        &mut self,
        tree: &mut WidgetTree,
        id: &crate::core::composite_text_edit::CompositeEditorId,
        operation: crate::core::composite_text_edit::CompositeEditOperation,
        origin: crate::core::composite_text_edit::CompositeEditOrigin,
        event: Option<&Event>,
        route_origin: PointerRouteOrigin,
        state: &mut InteractionDomainState,
        text_engine: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        animation: &mut Option<crate::core::animation::AnimationContext<'_>>,
        effects: &mut crate::core::widget::EventContextEffects,
    ) -> crate::core::composite_text_edit::CompositeEditReply {
        use crate::core::composite_text_edit::{
            CompositeEditOrigin, CompositeEditOutcome, CompositeEditReply,
        };
        if let Err(error) = tree.validate_composite_editor(id) {
            return CompositeEditReply::failure_for(id, error);
        }
        let host_capture = state.focus_manager.focused() == Some(id.host())
            && state.keyboard_mode == KeyboardAuthorityMode::Capture;
        if origin == CompositeEditOrigin::CurrentInput
            && (!host_capture
                || effects.keyboard_capture_release_requested
                || effects.focus_redirect.is_some())
        {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::DeniedBubble);
        }
        let Some(child) = tree.composite_child(id) else {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::Retired);
        };
        let position = match event {
            Some(Event::Pointer(pointer)) => tree.get_absolute_paint_rect(child).map(|rect| {
                let scroll = tree.cumulative_scroll_offset(child);
                (
                    pointer.global_pos.0 - rect.x + scroll.0,
                    pointer.global_pos.1 - rect.y + scroll.1,
                )
            }),
            _ => None,
        };
        let tree_ptr = tree as *mut WidgetTree;
        let pointer = event.and_then(|event| {
            if let Event::Pointer(p) = event {
                Some(p.id)
            } else {
                None
            }
        });
        let has_pointer_capture = pointer
            .is_some_and(|p| state.pointer_capture_manager.current_capture(p) == Some(id.host()));
        let mut cx = crate::core::widget::EventContext::new(
            id.child(),
            tree_ptr,
            self.frame_time_ms,
            self.clipboard_ptr(),
            text_engine.map(|p| unsafe { &mut *p }),
            animation.as_mut().map(|a| a.reborrow()),
            has_pointer_capture,
            self.is_ctrl_double_tap_active(),
            state.keyboard_mode,
        );
        cx.set_pointer_route_origin(route_origin);
        cx.set_composite_edit_authority(id, origin, host_capture);
        cx.set_composite_local_position(position);
        let reply = tree.run_composite_child(id, operation, &mut cx, event);
        effects.merge(cx.take_dispatch_effects());
        debug_assert!(
            cx.take_composite_edit().is_none(),
            "a subordinate TextField cannot stage another composite"
        );
        reply
    }

    #[allow(clippy::too_many_arguments)]
    fn run_composite_request<'te>(
        &mut self,
        tree: &mut WidgetTree,
        id: &crate::core::composite_text_edit::CompositeEditorId,
        mut request: crate::core::composite_text_edit::CompositeEditRequest,
        origin: crate::core::composite_text_edit::CompositeEditOrigin,
        event: Option<&Event>,
        route_origin: PointerRouteOrigin,
        state: &mut InteractionDomainState,
        text_engine: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        animation: &mut Option<crate::core::animation::AnimationContext<'_>>,
        effects: &mut crate::core::widget::EventContextEffects,
    ) -> crate::core::composite_text_edit::CompositeEditReply {
        use crate::core::composite_text_edit::{
            CompositeEditOutcome, CompositeEditReply, CompositeEditRequest,
        };
        use crate::widgets::composite_edit_host::{self, CompositeHostStep};
        if self.composite_transaction_active {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::Reentrant);
        }
        self.composite_transaction_active = true;
        let result = (|| {
            let mut action_emitted = false;
            let mut last_session = None;
            for crossing in 1..=2 {
                let mut reply = self.run_composite_child_scope(
                    tree,
                    id,
                    request.operation,
                    origin,
                    event,
                    route_origin,
                    state,
                    text_engine,
                    animation,
                    effects,
                );
                reply.id = Some(id.clone());
                if reply.session.is_some() {
                    last_session = reply.session;
                } else {
                    reply.session = last_session;
                }
                reply.stage = crossing;
                reply.host_action_already_emitted = action_emitted;
                if let Err(error) = tree.validate_composite_editor(id) {
                    reply.outcome = error;
                    reply.snapshot = None;
                    return reply;
                }
                let Some(host) = tree.find_node_by_widget_id(id.host()) else {
                    reply.outcome = CompositeEditOutcome::Retired;
                    return reply;
                };
                let tree_ptr = tree as *mut WidgetTree;
                let clipboard = self.clipboard_ptr();
                // No child borrow or child context survives this boundary.
                let (step, host_effects, extra_request) = {
                    let Some(node) = tree.node_mut(host) else {
                        let mut failure =
                            CompositeEditReply::failure_for(id, CompositeEditOutcome::Retired);
                        failure.session = last_session;
                        failure.stage = crossing;
                        failure.host_action_already_emitted = action_emitted;
                        return failure;
                    };
                    let mut cx = crate::core::widget::EventContext::new(
                        id.host(),
                        tree_ptr,
                        self.frame_time_ms,
                        clipboard,
                        text_engine.map(|p| unsafe { &mut *p }),
                        animation.as_mut().map(|a| a.reborrow()),
                        false,
                        self.is_ctrl_double_tap_active(),
                        state.keyboard_mode,
                    );
                    cx.set_pointer_route_origin(route_origin);
                    let step = composite_edit_host::resume(
                        node.widget.as_mut(),
                        &mut cx,
                        request.continuation,
                        reply.clone(),
                    );
                    (step, cx.take_dispatch_effects(), cx.take_composite_edit())
                };
                effects.merge(host_effects);
                if extra_request.is_some() {
                    reply.outcome = CompositeEditOutcome::Reentrant;
                    return reply;
                }
                match step {
                    CompositeHostStep::Done { handled } => {
                        reply.handled |= handled;
                        return reply;
                    }
                    CompositeHostStep::Final {
                        operation,
                        continuation,
                    } if crossing == 1 => {
                        action_emitted =
                            composite_edit_host::continuation_has_emitted_action(&continuation);
                        request = CompositeEditRequest {
                            operation,
                            continuation,
                        };
                    }
                    CompositeHostStep::Final { .. } => {
                        reply.outcome = CompositeEditOutcome::Reentrant;
                        return reply;
                    }
                }
            }
            unreachable!("the final composite continuation terminates")
        })();
        self.composite_transaction_active = false;
        result
    }

    pub(crate) fn run_composite_owner_operation(
        &mut self,
        tree: &mut WidgetTree,
        id: &crate::core::composite_text_edit::CompositeEditorId,
        operation: crate::core::composite_text_edit::CompositeEditOperation,
        state: &mut InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        frame_time_ms: u64,
    ) -> crate::core::composite_text_edit::CompositeEditReply {
        use crate::core::composite_text_edit::{
            CompositeEditOrigin, CompositeEditOutcome, CompositeEditReply,
        };
        if self.composite_transaction_active {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::Reentrant);
        }
        self.frame_time_ms = frame_time_ms;
        self.composite_transaction_active = true;
        let mut effects = crate::core::widget::EventContextEffects::empty();
        let reply = self.run_composite_child_scope(
            tree,
            id,
            operation,
            CompositeEditOrigin::OwnerProperty,
            None,
            PointerRouteOrigin::Regular,
            state,
            Some(text_engine as *mut _),
            &mut None,
            &mut effects,
        );
        if tree.validate_composite_editor(id).is_ok() {
            if let Some(node) = tree
                .find_node_by_widget_id(id.host())
                .and_then(|host| tree.node_mut(host))
            {
                crate::widgets::composite_edit_host::observe(node.widget.as_mut(), &reply);
            }
        }
        self.composite_transaction_active = false;
        self.settle_composite_lifecycle_effects(
            tree,
            id.host(),
            effects,
            state,
            Some(text_engine as *mut _),
        );
        reply
    }

    pub(crate) fn run_composite_owner_request(
        &mut self,
        tree: &mut WidgetTree,
        id: &crate::core::composite_text_edit::CompositeEditorId,
        request: crate::core::composite_text_edit::CompositeEditRequest,
        state: &mut InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        frame_time_ms: u64,
    ) -> crate::core::composite_text_edit::CompositeEditReply {
        self.frame_time_ms = frame_time_ms;
        let mut effects = crate::core::widget::EventContextEffects::empty();
        let reply = self.run_composite_request(
            tree,
            id,
            request,
            crate::core::composite_text_edit::CompositeEditOrigin::OwnerProperty,
            None,
            PointerRouteOrigin::Regular,
            state,
            Some(text_engine as *mut _),
            &mut None,
            &mut effects,
        );
        self.settle_composite_lifecycle_effects(
            tree,
            id.host(),
            effects,
            state,
            Some(text_engine as *mut _),
        );
        reply
    }

    fn settle_composite_lifecycle_effects<'te>(
        &mut self,
        tree: &mut WidgetTree,
        owner: TreeWidgetId,
        effects: crate::core::widget::EventContextEffects,
        state: &mut InteractionDomainState,
        text_engine: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
    ) {
        if let Some((pointer, capture_owner)) = effects.retire_descendant_pointer_capture {
            let authenticated = owner != capture_owner
                && tree
                    .find_node_by_widget_id(owner)
                    .zip(tree.find_node_by_widget_id(capture_owner))
                    .is_some_and(|(parent, child)| tree.is_ancestor_of(parent, child))
                && state.pointer_capture_manager.current_capture(pointer) == Some(capture_owner);
            if authenticated {
                self.cancel_pointer_session_with_tree(
                    tree,
                    pointer,
                    CancellationReason::SystemCancel,
                    state,
                );
            }
        }
        if effects.release_capture {
            state
                .pointer_capture_manager
                .release_capture(PointerId::MOUSE, owner);
            if state.pointer_capture.captured_widget == Some(owner) {
                state.pointer_capture.release();
            }
            self.restore_hidden_cursor_for_widget(owner);
        }
        if effects.continuous_frames {
            self.continuous_frames_requested = true;
        }
        if let Some(cursor) = effects.cursor {
            self.record_cursor_request(owner, cursor, state);
        }
        if effects.keyboard_capture_release_requested
            && state.focus_manager.focused() == Some(owner)
            && state.keyboard_mode == KeyboardAuthorityMode::Capture
        {
            state.keyboard_mode = KeyboardAuthorityMode::Navigation;
            self.invoke_composite_lifecycle(
                tree,
                owner,
                CompositeLifecycle::CaptureLeave,
                state,
                text_engine,
                false,
                false,
            );
            self.continuous_frames_requested = true;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn invoke_composite_lifecycle<'te>(
        &mut self,
        tree: &mut WidgetTree,
        owner: TreeWidgetId,
        kind: CompositeLifecycle,
        state: &mut InteractionDomainState,
        text_engine: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        availability: bool,
        defer_entry: bool,
    ) -> Option<crate::core::composite_text_edit::CompositeEditRequest> {
        let node_id = tree.find_node_by_widget_id(owner)?;
        let tree_ptr = tree as *mut WidgetTree;
        let clipboard = self.clipboard_ptr();
        let (request, mut effects) = {
            let node = tree.node_mut(node_id)?;
            let mut cx = crate::core::widget::EventContext::new(
                owner,
                tree_ptr,
                self.frame_time_ms,
                clipboard,
                text_engine.map(|p| unsafe { &mut *p }),
                None,
                false,
                false,
                state.keyboard_mode,
            );
            if availability {
                cx.set_availability_transition();
            }
            match kind {
                CompositeLifecycle::Focus => {
                    cx.set_focus_entry_direction(self.pending_focus_entry_direction.take());
                    node.widget.on_focus(&mut cx);
                }
                CompositeLifecycle::Blur => node.widget.on_blur(&mut cx),
                CompositeLifecycle::CaptureEnter => node.widget.on_capture_enter(&mut cx),
                CompositeLifecycle::CaptureLeave => node.widget.on_capture_leave(&mut cx),
            }
            (cx.take_composite_edit(), cx.take_dispatch_effects())
        };
        tree.mark_paint_dirty(node_id);
        if defer_entry {
            self.settle_composite_lifecycle_effects(tree, owner, effects, state, text_engine);
            return request;
        }
        if let Some(request) = request {
            if let Ok(id) = tree.composite_editor_id(owner) {
                self.run_composite_request(
                    tree,
                    &id,
                    request,
                    crate::core::composite_text_edit::CompositeEditOrigin::Lifecycle,
                    None,
                    PointerRouteOrigin::Regular,
                    state,
                    text_engine,
                    &mut None,
                    &mut effects,
                );
            }
        }
        self.settle_composite_lifecycle_effects(tree, owner, effects, state, text_engine);
        None
    }

    /// Route event directly to a specific widget (for captured pointer or focused keys)
    fn route_to_widget<'te>(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        event: &Event,
        text_engine_ptr: Option<*mut (dyn crate::core::text_engine::TextEngine + 'te)>,
        domain_state: &mut InteractionDomainState,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) {
        self.dispatch_to_widget(
            tree,
            widget_id,
            event,
            EventPhase::Target,
            text_engine_ptr,
            domain_state,
            animation,
        );
    }

    fn gesture_owner_available(tree: &WidgetTree, owner: TreeWidgetId) -> bool {
        tree.find_node_by_widget_id(owner)
            .and_then(|id| tree.node(id))
            .is_some_and(|node| {
                crate::core::widget_availability::effective(node.widget.as_ref())
                    .is_none_or(|state| state.enabled)
            })
    }

    fn touch_tap_at(
        tree: &WidgetTree,
        hit: &crate::core::event::HitTestResult,
        origin: PointerRouteOrigin,
        position: Point,
        root: Option<NodeId>,
    ) -> Option<PendingTouchTap> {
        let owner = hit.widget_id;
        let node_id = tree.find_node_by_widget_id(owner)?;
        let node = tree.node(node_id)?;
        if root.is_some_and(|root| !tree.is_descendant_of_node(owner, root))
            || !Self::gesture_owner_available(tree, owner)
            || !tree.composite_choice_input_ready(node)
        {
            return None;
        }
        let popup = (origin == PointerRouteOrigin::Overlay)
            .then(|| tree.combo_popup_input_geometry(owner))
            .flatten();
        let target = node.widget.touch_target(TouchHit {
            local_position: Point::new(hit.local_pos.0, hit.local_pos.1),
            size: crate::core::math::Size::new(hit.absolute_bounds.2, hit.absolute_bounds.3),
            position,
            origin,
            popup,
        })?;
        Some(PendingTouchTap {
            mount: tree.concrete_mount(owner)?,
            target,
            origin,
            rect: tree.get_absolute_paint_rect(node_id)?,
            viewport: tree.get_viewport(node_id),
            scroll: tree.cumulative_scroll_offset(node_id),
            popup_revision: node.widget.touch_target_revision(),
            popup_rows: popup.map(|geometry| geometry.rows),
        })
    }

    fn combo_popup_inert_press_at(
        tree: &WidgetTree,
        hit: &crate::core::event::HitTestResult,
        position: Point,
    ) -> bool {
        tree.find_node_by_widget_id(hit.widget_id)
            .and_then(|id| tree.node(id))
            .and_then(|node| {
                node.widget
                    .as_any()
                    .downcast_ref::<crate::widgets::ComboBox>()
            })
            .is_some_and(|combo| {
                combo.popup_inert_press_at(
                    tree.combo_popup_input_geometry(hit.widget_id),
                    hit.local_pos,
                    (position.x, position.y),
                )
            })
    }

    fn touch_dismissal_at(
        tree: &WidgetTree, position: Point, target: Option<TreeWidgetId>, root: Option<NodeId>,
    ) -> Option<PendingTouchTap> {
        let node_id = tree.touch_dismissible_popup(position, target)?;
        let node = tree.node(node_id)?;
        if root.is_some_and(|root| !tree.is_descendant_of_node(node.id, root)) {
            return None;
        }
        Some(PendingTouchTap {
            mount: tree.concrete_mount(node.id)?,
            target: crate::core::event::TouchTarget::DismissPopup,
            origin: PointerRouteOrigin::Overlay,
            rect: tree.get_absolute_paint_rect(node_id)?,
            viewport: tree.get_viewport(node_id),
            scroll: tree.cumulative_scroll_offset(node_id),
            popup_revision: node.widget.touch_target_revision(),
            popup_rows: tree.combo_popup_input_geometry(node.id).map(|geometry| geometry.rows),
        })
    }

    fn domain_accepts_touch_scroll(tree: &WidgetTree, state: &InteractionDomainState) -> bool {
        state.touch_scroll_source().is_none()
            && !state
                .pointer_capture_manager
                .active_pointers()
                .into_iter()
                .any(|pointer| {
                    let Some(owner) = state.pointer_capture_manager.current_capture(pointer) else {
                        return false;
                    };
                    state.scroll_capture_owner == Some(owner)
                        || state
                            .gesture_registry
                            .committed_candidate(pointer)
                            .is_some_and(|candidate| candidate.kind == GestureKind::Scroll)
                        || tree
                            .find_node_by_widget_id(owner)
                            .and_then(|id| tree.node(id))
                            .and_then(|node| node.widget.scroll_container())
                            .is_some_and(|scroll| {
                                scroll.input_mode()
                                    == crate::core::scroll_container::ScrollInputMode::Drag
                            })
                })
    }

    fn request_touch_scroll(
        tree: &WidgetTree,
        motion: QualifiedTouchMotion,
        root: Option<NodeId>,
        state: &mut InteractionDomainState,
    ) -> Option<TouchScrollRequest> {
        let pointer = motion.press.pointer;
        if !motion.first_contact
            || !(motion.position.distance(motion.origin)
                >= crate::core::input_fusion::DRAG_DISTANCE_THRESHOLD)
            || state.gesture_registry.is_committed(pointer)
            || !Self::domain_accepts_touch_scroll(tree, state)
        {
            return None;
        }
        let delta = Point::new(
            motion.position.x - motion.origin.x,
            motion.position.y - motion.origin.y,
        );
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return None;
        }
        let axis = if delta.y.abs() >= delta.x.abs() {
            GestureAxis::Vertical
        } else {
            GestureAxis::Horizontal
        };
        let (candidate, _) = state
            .gesture_registry
            .touch_scroll_mounts(pointer)
            .iter()
            .rev()
            .find(|(_, mount)| {
                mount.is_live()
                    && root.is_none_or(|root| tree.is_descendant_of_node(mount.owner(), root))
                    && Self::gesture_owner_available(tree, mount.owner())
                    && tree
                        .find_node_by_widget_id(mount.owner())
                        .and_then(|id| tree.node(id))
                        .is_some_and(|node| tree.composite_choice_input_ready(node)
                            && node.widget.touch_scroll_intent(motion, axis))
            })?;
        let candidate = *candidate;
        state
            .gesture_registry
            .push_commit_request(pointer, candidate);
        Some(TouchScrollRequest {
            candidate,
            source: TouchScrollSource {
                press: motion.press,
                axis,
            },
            delta,
            sample: motion,
        })
    }

    fn request_touch_drag(
        tree: &WidgetTree, motion: QualifiedTouchMotion, state: &mut InteractionDomainState,
    ) {
        if !(motion.position.distance(motion.origin) >= crate::core::input_fusion::DRAG_DISTANCE_THRESHOLD)
            || state.gesture_registry.is_committed(motion.press.pointer)
        {
            return;
        }
        let Some((candidate, mount, axis)) = state.gesture_registry.touch_drag(motion.press) else { return; };
        let (dx, dy) = (motion.position.x - motion.origin.x, motion.position.y - motion.origin.y);
        let dominant = if dy.abs() >= dx.abs() { GestureAxis::Vertical } else { GestureAxis::Horizontal };
        if (*axis == GestureAxis::Any || *axis == dominant)
            && mount.is_live()
            && Self::gesture_owner_available(tree, mount.owner())
        {
            state.gesture_registry.push_commit_request(motion.press.pointer, *candidate);
        }
    }

    fn route_active_touch_drag(
        &mut self, tree: &mut WidgetTree, motion: QualifiedTouchMotion,
        is_release: bool, state: &mut InteractionDomainState,
    ) -> bool {
        let pointer = motion.press.pointer;
        let Some((candidate, mount, _)) = state.gesture_registry.touch_drag(motion.press) else { return false; };
        let owner = mount.owner();
        if state.gesture_registry.committed_gesture(pointer) != Some(*candidate)
            || state.pointer_capture_manager.current_capture(pointer) != Some(owner)
        {
            return false;
        }
        if !mount.is_live() || !Self::gesture_owner_available(tree, owner) {
            self.cancel_pointer_session_with_tree(tree, pointer, CancellationReason::SystemCancel, state);
            return true;
        }
        let Some(id) = tree.find_by_id(owner) else { return true; };
        let Some(rect) = tree.get_absolute_paint_rect(id) else { return true; };
        let (sx, sy) = tree.cumulative_scroll_offset(id);
        let local = Point::new(motion.position.x + sx - rect.x, motion.position.y + sy - rect.y);
        let tree_ptr = tree as *mut WidgetTree;
        let mut cx = crate::core::widget::EventContext::new(
            owner, tree_ptr, self.frame_time_ms, self.clipboard_ptr(), None, None,
            true, false, state.keyboard_mode,
        );
        if local.x.is_finite() && local.y.is_finite()
            && let Some(node) = tree.node_mut(id)
        {
            node.widget.on_touch_drag_motion(&mut cx, TouchDragMotion {
                press: motion.press,
                local_position: local,
                is_release,
            });
        }
        true
    }

    fn route_active_touch_scroll(
        &mut self,
        tree: &mut WidgetTree,
        motion: QualifiedTouchMotion,
        root: Option<NodeId>,
        is_release: bool,
        state: &mut InteractionDomainState,
    ) -> bool {
        let Some(source) = state
            .touch_scroll_source()
            .filter(|source| source.press == motion.press)
        else {
            return false;
        };
        let pointer = source.press.pointer;
        let capture = Self::current_touch_scroll_owner(tree, state, source)
            .filter(|&owner| root.is_none_or(|root| tree.is_descendant_of_node(owner, root)));
        let Some(owner) = capture else {
            self.cancel_pointer_session_with_tree(
                tree,
                pointer,
                CancellationReason::SystemCancel,
                state,
            );
            return true;
        };
        let tree_ptr = tree as *mut WidgetTree;
        let mut kinetic_requested = false;
        let combo_popup_hit = tree.combo_popup_input_geometry(owner);
        if let Some(node) = tree
            .find_node_by_widget_id(owner)
            .and_then(|id| tree.node_mut(id))
        {
            let mut cx = crate::core::widget::EventContext::new(
                owner,
                tree_ptr,
                self.frame_time_ms,
                self.clipboard_ptr(),
                None,
                None,
                true,
                false,
                state.keyboard_mode,
            );
            cx.set_combo_popup_hit(combo_popup_hit);
            cx.set_touch_scroll_motion(TouchScrollMotion {
                press: source.press,
                axis: source.axis,
                delta: Point::new(
                    motion.position.x - motion.previous.x,
                    motion.position.y - motion.previous.y,
                ),
                is_release,
                sample: motion,
            });
            node.widget.on_touch_scroll_motion(&mut cx);
            kinetic_requested = cx.kinetic_scroll_requested();
        }
        if is_release {
            tree.discard_pending_scroll_intents();
            if kinetic_requested && let Some(mount) = tree.concrete_mount(owner) {
                state.admit_kinetic_scroll(source, mount);
            }
        }
        true
    }

    fn current_touch_scroll_owner(
        tree: &WidgetTree,
        state: &InteractionDomainState,
        source: TouchScrollSource,
    ) -> Option<TreeWidgetId> {
        let owner = state.scroll_capture_owner?;
        let pointer = source.press.pointer;
        let winner = state.gesture_registry.committed_candidate(pointer)?;
        let (candidate, mount) = state
            .gesture_registry
            .touch_scroll_mounts(pointer)
            .first()?;
        (winner.candidate_id == *candidate
            && owner == winner.widget_id
            && tree.concrete_mount(owner).as_ref() == Some(mount)
            && state
                .pointer_capture_manager
                .owns_touch_press(source.press, mount)
            && Self::gesture_owner_available(tree, owner)
            && tree.find_by_id(owner).and_then(|id| tree.node(id)).is_some_and(|node| {
                tree.composite_choice_input_ready(node) && node.widget.touch_scroll_is_live(source.press)
            }))
        .then_some(owner)
    }

    pub(crate) fn retire_stale_touch_scroll(
        &mut self,
        tree: &mut WidgetTree,
        state: &mut InteractionDomainState,
    ) {
        if let Some(mount) = state.kinetic_scroll_mount()
            && (!mount.is_live()
                || !Self::gesture_owner_available(tree, mount.owner())
                || state.focus_manager.active_scope().is_some_and(|root| {
                    !tree.is_descendant_of_node(mount.owner(), root)
                }))
        {
            state.interrupt_kinetic_scroll();
        }
        if let Some(source) = state.touch_scroll_source()
            && Self::current_touch_scroll_owner(tree, state, source).is_none()
        {
            self.cancel_pointer_session_with_tree(
                tree,
                source.press.pointer,
                CancellationReason::SystemCancel,
                state,
            );
        }
    }

    /// SPEC-HAL-128 Stage 3 Fix: Resolve all pending gesture commit requests for a pointer.
    /// SPEC-HAL-128 Stage 7: Now uses PointerType for deterministic arbitration.
    /// SPEC-HAL-128 Stage 8: Calls on_gesture_commit/on_gesture_cancel callbacks.
    ///
    /// Called once per pointer after event dispatch to enforce priority-based arbitration.
    /// INV-S7-P4: Primary gesture for pointer type wins, then secondary by stable order.
    ///
    /// INV-GA3: At most one gesture may commit per PointerId.
    /// INV-GA4: Priority-based resolution independent of dispatch order.
    /// INV-GA5: Commit only happens when explicitly requested.
    /// INV-GA6: On commit, pointer is captured for the winning widget.
    /// INV-GA7: Losers are structurally removed from the registry.
    /// INV-GS8: Winner receives on_gesture_commit callback.
    /// INV-GS9: Losers receive on_gesture_cancel callback.
    /// INV-S7-P7: No semantic fallbacks - if no legal commit, session resolves to none.
    fn resolve_gesture_commits_for_pointer(
        &mut self,
        tree: &mut WidgetTree,
        pointer_id: PointerId,
        domain_state: &mut InteractionDomainState,
    ) {
        self.resolve_gesture_requests(tree, pointer_id, domain_state, None);
    }

    fn resolve_gesture_requests(
        &mut self,
        tree: &mut WidgetTree,
        pointer_id: PointerId,
        domain_state: &mut InteractionDomainState,
        touch_scroll: Option<TouchScrollRequest>,
    ) {
        self.retire_invalid_absolute_geometry(tree, domain_state);
        // If no active session, nothing to resolve
        if domain_state
            .gesture_registry
            .session_id(pointer_id)
            .is_none()
        {
            return;
        }

        // SPEC-HAL-128 Stage 7: Get pointer type for priority resolution
        let pointer_type = match domain_state
            .gesture_registry
            .session_pointer_type(pointer_id)
        {
            Some(pt) => pt,
            None => return, // No session, nothing to resolve
        };

        // If already committed, drain pending requests and return
        if domain_state.gesture_registry.is_committed(pointer_id) {
            let _ = domain_state
                .gesture_registry
                .take_commit_requests(pointer_id);
            return;
        }

        // Take all pending commit requests
        let pending = domain_state
            .gesture_registry
            .take_commit_requests(pointer_id);
        if pending.is_empty() {
            return;
        }

        // INV-GA4 / INV-S7-P4: Use GestureArbiter::resolve_priority with pointer_type
        // for single-authority priority resolution.
        // Scope the borrow of gesture_registry to extract winner + loser info before mutation.
        let (winner_info, losers_info): (
            Option<(
                crate::core::event::GestureCandidateId,
                crate::utils::id::TreeWidgetId,
                crate::core::event::GestureKind,
                PointerMotionPolicy,
            )>,
            Vec<(
                crate::utils::id::TreeWidgetId,
                crate::core::event::GestureKind,
            )>,
        ) = {
            let pending_candidates: Vec<_> = pending
                .iter()
                .filter_map(|id| domain_state.gesture_registry.get_candidate(pointer_id, *id))
                .collect();

            // SPEC-HAL-128 Stage 7: Pass pointer_type for deterministic arbitration
            let winner = if let Some(scroll) = &touch_scroll {
                // HAL-396 §3: content scroll is a fallback for this Touch
                // profile. The arbiter retains the order among other kinds.
                let non_scroll: Vec<_> = pending_candidates
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        candidate.kind != GestureKind::Scroll
                            && Self::gesture_owner_available(tree, candidate.widget_id)
                    })
                    .collect();
                self.gesture_arbiter
                    .resolve_priority(&non_scroll, pointer_type)
                    .copied()
                    .or_else(|| {
                        pending_candidates
                            .iter()
                            .copied()
                            .find(|candidate| candidate.candidate_id == scroll.candidate)
                            .copied()
                    })
            } else {
                self.gesture_arbiter
                    .resolve_priority(&pending_candidates, pointer_type)
                    .copied()
            };

            let winner_info =
                winner.map(|c| (c.candidate_id, c.widget_id, c.kind, c.motion_policy));

            // SPEC-HAL-128 Stage 8: Collect losers for on_gesture_cancel callbacks
            let losers: Vec<_> = domain_state
                .gesture_registry
                .candidates(pointer_id)
                .iter()
                .filter(|c| winner.map(|w| w.candidate_id) != Some(c.candidate_id))
                .filter(|c| {
                    domain_state
                        .gesture_registry
                        .candidate_mount_is_live(pointer_id, c.candidate_id)
                })
                .map(|c| (c.widget_id, c.kind))
                .collect();

            (winner_info, losers)
        }; // borrow of gesture_registry ends here

        let (winner_id, winner_widget, winner_kind, motion_policy) = match winner_info {
            Some(info) => info,
            None => return,
        };

        let candidate_count = pending.len();
        let touch_scroll = touch_scroll.filter(|scroll| scroll.candidate == winner_id);
        let touch_press = domain_state
            .gesture_registry
            .physical_press(pointer_id)
            .filter(|press| press.pointer_type == PointerType::Touch)
            .map(|press| press.touch_identity());
        let mut transfer = None;
        let mut scroll_mount = None;
        if motion_policy == PointerMotionPolicy::Absolute {
            if !Self::gesture_owner_available(tree, winner_widget) {
                return;
            }
            if let Some(scroll) = &touch_scroll {
                scroll_mount = tree.concrete_mount(winner_widget);
                if scroll_mount.is_none()
                    || touch_press != Some(scroll.source.press)
                    || !Self::domain_accepts_touch_scroll(tree, domain_state)
                {
                    return;
                }
            }
            if let Some(owner) = domain_state
                .pointer_capture_manager
                .current_capture(pointer_id)
                && owner != winner_widget
            {
                let Some(scroll) = &touch_scroll else {
                    return;
                };
                let Some(mount) = tree.concrete_mount(owner) else {
                    return;
                };
                if !domain_state
                    .pointer_capture_manager
                    .owns_touch_press(scroll.source.press, &mount)
                    || !tree
                        .find_node_by_widget_id(owner)
                        .and_then(|node| tree.node(node))
                        .is_some_and(|node| node.widget.has_unfinished_touch_press(pointer_id))
                {
                    return;
                }
                transfer = Some(mount);
            }
        }

        // Absolute admission and capture refusal are settled before this
        // publication. No widget callback can interleave with the capture write.
        match self.gesture_arbiter.try_commit(
            &mut domain_state.gesture_registry,
            pointer_id,
            winner_id,
        ) {
            Ok(_result) => {
                // INV-GA7: Structurally remove losers from registry
                domain_state
                    .gesture_registry
                    .retain_only_winner(pointer_id, winner_id);

                // INV-GA6: Capture pointer for the winning widget
                let capture_result =
                    if let (Some(scroll), Some(mount)) = (&touch_scroll, scroll_mount) {
                        domain_state.pointer_capture_manager.capture_touch_scroll(
                            scroll.source.press,
                            transfer.as_ref(),
                            mount,
                        );
                        domain_state.begin_touch_scroll(winner_widget, scroll.source);
                        tree.bind_scroll_intent_admission(domain_state.scroll_intent_admission());
                        self.pending_focus_reveal = None;
                        Ok(())
                    } else {
                        domain_state
                            .pointer_capture_manager
                            .request_capture(pointer_id, winner_widget)
                    };
                if let Err(ref e) = capture_result {
                    tracing::error!(
                        "SPEC-HAL-128 Stage 3: Failed to capture pointer after commit: {:?}",
                        e
                    );
                } else {
                    // Keep legacy pointer_capture in sync for compatibility
                    let captured_button =
                        if matches!(motion_policy, PointerMotionPolicy::RelativeLocked { .. }) {
                            domain_state
                                .gesture_registry
                                .session_down_facts(pointer_id)
                                .map(|(button, _)| button)
                                .unwrap_or(PointerButton::Primary)
                        } else {
                            PointerButton::Primary
                        };
                    if pointer_id == PointerId::MOUSE {
                        domain_state
                            .pointer_capture
                            .capture(winner_widget, captured_button);
                    }
                    tracing::debug!(
                        "SPEC-HAL-128 Stage 3: Pointer captured for widget {:?} after gesture commit (priority-based)",
                        winner_widget.0
                    );
                }

                if let PointerMotionPolicy::RelativeLocked { restore } = motion_policy {
                    let admission = (|| {
                        if capture_result.is_err() {
                            return Err(RelativePointerAdmissionError::HardCaptureNotOwned);
                        }
                        let facts = self
                            .relative_window_facts
                            .get()
                            .ok_or(RelativePointerAdmissionError::CapabilityUnsupported)?;
                        if self.current_phase.get() != Some(InputPipelinePhase::Routing) {
                            return Err(RelativePointerAdmissionError::CapabilityUnsupported);
                        }
                        let (button, anchor) = domain_state
                            .gesture_registry
                            .session_down_facts(pointer_id)
                            .ok_or(RelativePointerAdmissionError::StaleBinding)?;
                        let candidate = *domain_state
                            .gesture_registry
                            .get_candidate(pointer_id, winner_id)
                            .ok_or(RelativePointerAdmissionError::StaleBinding)?;
                        let hard_capture = domain_state
                            .pointer_capture_manager
                            .current_capture_with_mode(pointer_id)
                            .and_then(|(owner, mode)| {
                                domain_state.pointer_capture.button.map(|button| {
                                    RelativePointerHardCaptureFacts {
                                        pointer_id,
                                        owner,
                                        button,
                                        mode,
                                    }
                                })
                            });
                        let authority = domain_state
                            .relative_pointer_authority
                            .as_mut()
                            .ok_or(RelativePointerAdmissionError::CapabilityUnsupported)?;
                        authority.admit(RelativePointerAdmissionFacts {
                            window_id: facts.window_id,
                            interaction_domain_id: facts.interaction_domain_id,
                            pointer_id,
                            gesture_session_id: winner_id.session,
                            candidate,
                            committed_candidate_id: domain_state
                                .gesture_registry
                                .committed_candidate(pointer_id)
                                .map(|candidate| candidate.candidate_id),
                            pointer_button: button,
                            down_anchor: (restore == CursorRestorePolicy::DownAnchor)
                                .then_some(anchor),
                            focused_window_id: facts.focused_window_id,
                            owner_available: tree.find_node_by_widget_id(winner_widget).is_some(),
                            hard_capture,
                        })
                    })();
                    if let Err(error) = admission {
                        self.notify_gesture_losers(tree, &losers_info, domain_state, touch_press);
                        self.deny_relative_candidate(
                            tree,
                            pointer_id,
                            winner_widget,
                            winner_kind,
                            error,
                            domain_state,
                        );
                        return;
                    }
                }

                if let Some(former) = transfer {
                    self.notify_pointer_capture_cancel(
                        tree,
                        former.owner(),
                        pointer_id,
                        CancellationReason::SystemCancel,
                        domain_state,
                        touch_press,
                    );
                }

                // SPEC-HAL-128 Stage 8 INV-GS9: Call on_gesture_cancel for losers
                self.notify_gesture_losers(tree, &losers_info, domain_state, touch_press);

                // SPEC-HAL-128 Stage 8 INV-GS8: Call on_gesture_commit for winner
                let tree_ptr = tree as *mut WidgetTree;
                let clipboard_ptr = self.clipboard_ptr();
                let combo_popup_hit = tree.combo_popup_input_geometry(winner_widget);
                if let Some(node_id) = tree.find_node_by_widget_id(winner_widget) {
                    if let Some(node) = tree.node_mut(node_id) {
                        let mut cx = crate::core::widget::EventContext::new(
                            winner_widget,
                            tree_ptr,
                            self.frame_time_ms,
                            clipboard_ptr,
                            None, // INV-TEXT-GEOM-PHASE: text_engine not needed for lifecycle callbacks
                            None, // SPEC-HAL-021 Stage 2.5: Animation not needed for gesture commit
                            motion_policy == PointerMotionPolicy::Absolute,
                            false, // SPEC-HAL-268: Not relevant for lifecycle callbacks
                            domain_state.keyboard_mode,
                        );
                        cx.set_touch_gesture_press(touch_press);
                        cx.set_combo_popup_hit(combo_popup_hit);
                        if let Some(scroll) = &touch_scroll {
                            cx.set_touch_scroll_motion(TouchScrollMotion {
                                press: scroll.source.press,
                                axis: scroll.source.axis,
                                delta: scroll.delta,
                                is_release: false,
                                sample: scroll.sample,
                            });
                        }
                        node.widget.on_gesture_commit(&mut cx, winner_kind);
                        tracing::debug!(
                            "SPEC-HAL-128 Stage 8: on_gesture_commit called for winner {:?} (kind {:?})",
                            winner_widget.0,
                            winner_kind
                        );
                    }
                }

                tracing::debug!(
                    "SPEC-HAL-128 Stage 3: Gesture commit resolved - winner {:?} (kind {:?}), {} candidates considered",
                    winner_id,
                    winner_kind,
                    candidate_count
                );
            }
            Err(e) => {
                tracing::debug!(
                    "SPEC-HAL-128 Stage 3: Gesture commit resolution failed: {:?}",
                    e
                );
            }
        }
    }

    /// SPEC-HAL-128 Stage 8: Register gesture candidates based on widget declarations.
    ///
    /// Called on PointerDown after session is created. For each widget in the event path:
    /// 1. Query widget's gesture_surface() (INV-GS1: static declaration)
    /// 2. Filter by PointerType legality (Stage 7 INV-PT2)
    /// 3. Register legal candidates in the registry
    ///
    /// INV-GS2: Widgets declare, system decides
    /// INV-GS3: Registration is the only widget action (via declaration)
    /// INV-GS4: No PointerType branching in widgets
    fn register_candidates_from_declarations(
        &mut self,
        tree: &mut WidgetTree,
        event_path: &[TreeWidgetId],
        pointer_id: PointerId,
        domain_state: &mut InteractionDomainState,
        geometry_only: bool,
    ) {
        for &widget_id in event_path {
            let Some(node) = tree
                .find_node_by_widget_id(widget_id)
                .and_then(|id| tree.node(id))
            else {
                continue;
            };
            if domain_state
                .gesture_registry
                .session_pointer_type(pointer_id)
                == Some(PointerType::Touch)
                && !node.widget.accepts_touch_pointer_input()
            {
                continue;
            }
            let dependency = node.widget.absolute_drag_geometry_dependency();
            if geometry_only && dependency.is_none() {
                continue;
            }
            let declarations: Vec<_> = node
                .widget
                .gesture_surface()
                .iter()
                .filter(|&&kind| {
                    kind != GestureKind::Scroll || !node.widget.accepts_touch_scroll_gesture()
                })
                .map(|kind| (*kind, node.widget.pointer_motion_policy(*kind)))
                .collect();
            if let Some(dependency) = dependency {
                assert!(
                    declarations
                        == [(
                            crate::core::event::GestureKind::Drag,
                            PointerMotionPolicy::Absolute
                        )],
                    "INV-ABSOLUTE-GEOMETRY-ADMISSION: static Absolute Drag required"
                );
                let press = domain_state
                    .gesture_registry
                    .physical_press(pointer_id)
                    .expect("INV-ABSOLUTE-GEOMETRY-QUALIFIED-HOST");
                if press.button != PointerButton::Primary {
                    continue;
                }
                let watch =
                    tree.freeze_absolute_drag_geometry(widget_id, dependency, &press.cutoff);
                domain_state
                    .gesture_registry
                    .watch_absolute_geometry(pointer_id, watch);
                tree.geometry_protected_owners.insert(widget_id);
            }
            for (kind, motion_policy) in declarations {
                domain_state
                    .gesture_registry
                    .register_candidate_request_with_motion_policy(
                        pointer_id,
                        widget_id,
                        kind,
                        None,
                        motion_policy,
                    );
            }
        }
    }

    /// SPEC-HAL-128 Stage 4: Cancel all pointer session state for a pointer.
    /// SPEC-HAL-128 Stage 8: Calls on_gesture_cancel for all candidates.
    ///
    /// INV-079-FORCE-TEARDOWN: This is a FORCE teardown function.
    /// Allowed in ANY phase (PreHitTest, HitTest, Routing, PostProcess, or None).
    /// Force teardown is used for: ESC cancel, overlay takeover, widget destruction,
    /// window focus loss, or any situation requiring immediate session termination.
    ///
    /// This is the single centralized cancellation path. All cancellation flows
    /// (Escape, overlay takeover, widget destruction, system cancel) MUST use this.
    ///
    /// Invariants enforced:
    /// - INV-PC4: Escape cancels everything
    /// - INV-PC5: Lossless cancellation (no partial state)
    /// - INV-PC7: Overlay supremacy
    /// - INV-GA-CANCEL: Cancellation beats priority (no commit after cancel)
    /// - INV-GS9: All candidates receive on_gesture_cancel
    ///
    /// After this call:
    /// - `is_committed(pointer)` == false
    /// - `is_captured(pointer)` == false
    /// - `has_session(pointer)` == false
    /// - No pending commits remain
    pub fn cancel_pointer_session_with_tree(
        &mut self,
        tree: &mut WidgetTree,
        pointer_id: PointerId,
        reason: CancellationReason,
        domain_state: &mut InteractionDomainState,
    ) {
        if domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
            .is_some_and(|activation| activation.binding().pointer_id() == pointer_id)
        {
            let relative_reason = match reason {
                CancellationReason::Escape => {
                    RelativePointerCancellationReason::ExplicitSemanticCancel
                }
                CancellationReason::WidgetDestroyed => {
                    RelativePointerCancellationReason::OwnerUnmounted
                }
                CancellationReason::OverlayTakeover | CancellationReason::SystemCancel => {
                    RelativePointerCancellationReason::CaptureLost
                }
            };
            self.terminate_relative_pointer(
                Some(tree),
                domain_state,
                RelativePointerExit::Cancelled(relative_reason),
                self.relative_window_facts.get(),
            );
            return;
        }
        tracing::info!(
            "SPEC-HAL-128 Stage 4: Cancelling pointer session {:?} for reason {:?}",
            pointer_id.0,
            reason
        );

        let callbacks_live = domain_state
            .gesture_registry
            .physical_press(pointer_id)
            .is_none_or(|press| tree.owns_geometry_cutoff(&press.cutoff));
        let capture_owner = domain_state
            .pointer_capture_manager
            .current_capture(pointer_id);
        let touch_press = domain_state
            .gesture_registry
            .physical_press(pointer_id)
            .filter(|press| press.pointer_type == PointerType::Touch)
            .map(|press| press.touch_identity());
        let touch_scroll_owner = domain_state
            .touch_scroll_source()
            .filter(|source| source.press.pointer == pointer_id)
            .and(domain_state.scroll_capture_owner);
        let stale_touch_scroll = touch_scroll_owner.is_some()
            && !domain_state
                .gesture_registry
                .touch_scroll_mounts(pointer_id)
                .first()
                .is_some_and(|(_, mount)| mount.is_live());
        if touch_scroll_owner.is_some() && callbacks_live {
            tree.discard_pending_scroll_intents();
        }
        if callbacks_live
            && domain_state
                .gesture_registry
                .physical_press(pointer_id)
                .is_some_and(|press| press.protected.get())
        {
            if let Some(owner) = capture_owner {
                self.geometry_capture_changes
                    .push((tree.geometry_causal_cutoff(), owner));
            }
        }

        for (pointer, _, watch) in domain_state.gesture_registry.geometry_watches() {
            if pointer == pointer_id {
                tree.geometry_protected_owners.remove(&watch.owner);
            }
        }

        // INV-GA-CANCEL: Drain any pending commits first (cancellation beats priority)
        // This prevents any commit from happening after cancel is initiated.
        let _ = domain_state
            .gesture_registry
            .take_commit_requests(pointer_id);

        // SPEC-HAL-128 Stage 8: Collect candidates for cancel callbacks before clearing
        let candidates_to_cancel: Vec<(TreeWidgetId, crate::core::event::GestureKind)> =
            domain_state
                .gesture_registry
                .candidates(pointer_id)
                .iter()
                .filter(|c| {
                    domain_state
                        .gesture_registry
                        .candidate_mount_is_live(pointer_id, c.candidate_id)
                })
                .map(|c| (c.widget_id, c.kind))
                .collect();

        // SPEC-HAL-128 Stage 8 INV-GS9: Call on_gesture_cancel for all candidates
        let tree_ptr = tree as *mut WidgetTree;
        let clipboard_ptr = self.clipboard_ptr();
        for (widget_id, kind) in &candidates_to_cancel {
            if !callbacks_live || (stale_touch_scroll && Some(*widget_id) == touch_scroll_owner) {
                continue;
            }
            if let Some(node_id) = tree.find_node_by_widget_id(*widget_id) {
                if let Some(node) = tree.node_mut(node_id) {
                    let mut cx = crate::core::widget::EventContext::new(
                        *widget_id,
                        tree_ptr,
                        self.frame_time_ms,
                        clipboard_ptr,
                        None, // INV-TEXT-GEOM-PHASE: text_engine not needed for lifecycle callbacks
                        None, // SPEC-HAL-021 Stage 2.5: Animation not needed for gesture cancel
                        false, // Capture not relevant for lifecycle callbacks
                        false, // SPEC-HAL-268: Not relevant for lifecycle callbacks
                        domain_state.keyboard_mode,
                    );
                    cx.set_touch_gesture_press(touch_press);
                    node.widget.on_gesture_cancel(&mut cx, *kind);
                    tracing::debug!(
                        "SPEC-HAL-128 Stage 8: on_gesture_cancel called for {:?} (kind {:?}, reason {:?})",
                        widget_id.0,
                        kind,
                        reason
                    );
                }
            }
        }

        // INV-PC5: Clear gesture session (removes candidates, committed state, pending_commits)
        domain_state.gesture_registry.clear_candidates(pointer_id);

        // INV-PC4/PC5: Release pointer capture
        self.restore_hidden_cursor_for_capture(pointer_id, domain_state);
        domain_state
            .pointer_capture_manager
            .cancel_capture(pointer_id);
        if pointer_id == PointerId::MOUSE {
            domain_state.pointer_capture.release();
        }
        domain_state.end_touch_scroll(pointer_id);

        // INV-PC5: Capture-only owners also need terminal rollback, without a
        // fabricated gesture candidate or a keyboard-focus lifecycle change.
        if let Some(owner) = capture_owner.filter(|_| callbacks_live && !stale_touch_scroll) {
            self.notify_pointer_capture_cancel(
                tree,
                owner,
                pointer_id,
                reason,
                domain_state,
                touch_press,
            );
        }

        tracing::debug!(
            "SPEC-HAL-128 Stage 4: Session cancelled - committed={}, captured={}, has_session={}",
            domain_state.gesture_registry.is_committed(pointer_id),
            domain_state.pointer_capture_manager.is_captured(pointer_id),
            domain_state.gesture_registry.has_session(pointer_id)
        );
    }

    fn notify_pointer_capture_cancel(
        &mut self,
        tree: &mut WidgetTree,
        owner: TreeWidgetId,
        pointer: PointerId,
        reason: CancellationReason,
        state: &InteractionDomainState,
        touch_press: Option<TouchPressIdentity>,
    ) {
        let tree_ptr = tree as *mut WidgetTree;
        if let Some(node_id) = tree.find_node_by_widget_id(owner) {
            if let Some(node) = tree.node_mut(node_id) {
                let mut cx = crate::core::widget::EventContext::new(
                    owner,
                    tree_ptr,
                    self.frame_time_ms,
                    self.clipboard_ptr(),
                    None,
                    None,
                    false,
                    false,
                    state.keyboard_mode,
                );
                cx.set_touch_gesture_press(touch_press);
                node.widget
                    .on_pointer_capture_cancel(&mut cx, pointer, reason);
            }
            tree.mark_paint_dirty(node_id);
        }
    }

    pub(crate) fn cancel_widget_interaction_for_availability(
        &mut self,
        tree: &mut WidgetTree,
        owner: TreeWidgetId,
        state: &mut InteractionDomainState,
    ) {
        let mut pointers = state.gesture_registry.pointers_for_widget(owner);
        pointers.extend(
            state
                .pointer_capture_manager
                .active_pointers()
                .into_iter()
                .filter(|pointer| {
                    state.pointer_capture_manager.current_capture(*pointer) == Some(owner)
                }),
        );
        pointers.sort_by_key(|pointer| pointer.0);
        pointers.dedup();
        let mut candidate_only = Vec::new();
        for pointer in pointers {
            let owns_session = state.pointer_capture_manager.current_capture(pointer)
                == Some(owner)
                || state
                    .gesture_registry
                    .committed_candidate(pointer)
                    .is_some_and(|candidate| candidate.widget_id == owner);
            if owns_session {
                self.cancel_pointer_session_with_tree(
                    tree,
                    pointer,
                    CancellationReason::SystemCancel,
                    state,
                );
            } else {
                candidate_only.extend(
                    state
                        .gesture_registry
                        .candidates(pointer)
                        .iter()
                        .filter(|candidate| candidate.widget_id == owner)
                        .map(|candidate| candidate.kind),
                );
            }
        }
        let tree_ptr = tree as *mut WidgetTree;
        for kind in candidate_only {
            if let Some(node_id) = tree.find_node_by_widget_id(owner) {
                if let Some(node) = tree.node_mut(node_id) {
                    let mut cx = crate::core::widget::EventContext::new(
                        owner,
                        tree_ptr,
                        self.frame_time_ms,
                        self.clipboard_ptr(),
                        None,
                        None,
                        false,
                        false,
                        state.keyboard_mode,
                    );
                    cx.set_availability_transition();
                    node.widget.on_gesture_cancel(&mut cx, kind);
                    self.process_lifecycle_requests(owner, &cx, state);
                }
            }
        }
        state.gesture_registry.remove_candidates_for_widget(owner);
        if state.scroll_capture_owner == Some(owner) {
            state.clear_scroll_capture();
        }
        if state.hovered_widget == Some(owner) {
            self.clear_hover(tree, state);
        }
    }

    /// SPEC-HAL-128 Stage 4: Cancel session without tree access (no callbacks).
    ///
    /// INV-079-FORCE-TEARDOWN: This is a FORCE teardown function.
    /// Allowed in ANY phase. Used when tree is unavailable.
    ///
    /// Use this only when tree is not available (e.g., widget destruction cleanup).
    /// For normal cancellation, prefer `cancel_pointer_session_with_tree`.
    pub fn cancel_pointer_session(
        &mut self,
        pointer_id: PointerId,
        reason: CancellationReason,
        domain_state: &mut InteractionDomainState,
    ) {
        if domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
            .is_some_and(|activation| activation.binding().pointer_id() == pointer_id)
        {
            let relative_reason = if reason == CancellationReason::WidgetDestroyed {
                RelativePointerCancellationReason::OwnerUnmounted
            } else {
                RelativePointerCancellationReason::CaptureLost
            };
            self.terminate_relative_pointer(
                None,
                domain_state,
                RelativePointerExit::Cancelled(relative_reason),
                None,
            );
            return;
        }
        tracing::info!(
            "SPEC-HAL-128 Stage 4: Cancelling pointer session {:?} for reason {:?} (no callbacks)",
            pointer_id.0,
            reason
        );

        let _ = domain_state
            .gesture_registry
            .take_commit_requests(pointer_id);
        domain_state.gesture_registry.clear_candidates(pointer_id);
        self.restore_hidden_cursor_for_capture(pointer_id, domain_state);
        domain_state
            .pointer_capture_manager
            .cancel_capture(pointer_id);
        if pointer_id == PointerId::MOUSE {
            domain_state.pointer_capture.release();
        }
        domain_state.end_touch_scroll(pointer_id);
    }

    /// SPEC-HAL-128 Stage 4 INV-PC7: Cancel all pointer sessions (overlay supremacy).
    /// SPEC-HAL-128 Stage 6: Now iterates ALL active pointers, not just MOUSE.
    /// SPEC-HAL-128 Stage 8: Calls on_gesture_cancel callbacks for all candidates.
    ///
    /// INV-079-FORCE-TEARDOWN: This is a FORCE teardown function.
    /// Allowed in ANY phase. Used for overlay takeover or ESC.
    ///
    /// Called when an overlay takes over input or Escape is pressed.
    /// Cancels ALL active pointer sessions and calls on_gesture_cancel for each candidate.
    /// After this call, no gesture or capture state exists for any pointer.
    pub fn cancel_all_pointer_sessions_with_tree(
        &mut self,
        tree: &mut WidgetTree,
        reason: CancellationReason,
        domain_state: &mut InteractionDomainState,
    ) {
        domain_state.interrupt_kinetic_scroll();
        // SPEC-HAL-128 Stage 6: Collect all active pointers from both subsystems
        let mut all_pointers = std::collections::HashSet::new();
        all_pointers.extend(domain_state.gesture_registry.active_pointers());
        all_pointers.extend(domain_state.pointer_capture_manager.active_pointers());
        if let Some(activation) = domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
        {
            all_pointers.insert(activation.binding().pointer_id());
        }
        if all_pointers.is_empty() {
            return;
        }

        tracing::info!(
            "SPEC-HAL-128 Stage 6/8: Cancelling ALL pointer sessions for reason {:?} (with callbacks)",
            reason
        );

        // Cancel each pointer session with callbacks
        for pointer_id in all_pointers {
            self.cancel_pointer_session_with_tree(tree, pointer_id, reason, domain_state);
        }
    }

    /// SPEC-HAL-128 Stage 4 INV-PC7: Cancel all sessions without tree (no callbacks).
    ///
    /// INV-079-FORCE-TEARDOWN: This is a FORCE teardown function.
    /// Allowed in ANY phase. Used when tree is unavailable.
    ///
    /// Use this only when tree is not available. For normal cancellation with
    /// callbacks, prefer `cancel_all_pointer_sessions_with_tree`.
    pub fn cancel_all_pointer_sessions(
        &mut self,
        reason: CancellationReason,
        domain_state: &mut InteractionDomainState,
    ) {
        let mut all_pointers = std::collections::HashSet::new();
        all_pointers.extend(domain_state.gesture_registry.active_pointers());
        all_pointers.extend(domain_state.pointer_capture_manager.active_pointers());
        if let Some(activation) = domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
        {
            all_pointers.insert(activation.binding().pointer_id());
        }
        if all_pointers.is_empty() {
            return;
        }

        tracing::info!(
            "SPEC-HAL-128 Stage 6: Cancelling ALL pointer sessions for reason {:?} (no callbacks)",
            reason
        );

        for pointer_id in all_pointers {
            self.cancel_pointer_session(pointer_id, reason, domain_state);
        }
    }

    // ========================================================================
    // SPEC-HAL-128 Stage 5: Widget Lifecycle Safety
    // ========================================================================

    /// SPEC-HAL-128 Stage 5 INV-WL1/INV-WL2: Handle widget destruction.
    /// SPEC-HAL-128 Stage 6: Now handles ALL pointers, not just MOUSE.
    ///
    /// INV-079-FORCE-TEARDOWN: This is a FORCE teardown function.
    /// Allowed in ANY phase. Called during tree mutation when widgets are removed.
    ///
    /// Called when widgets are removed from the tree. For each destroyed widget:
    /// - Releases any pointer capture held by the widget (for ALL pointers)
    /// - Removes gesture candidates for the widget (for ALL pointers)
    /// - Cancels any sessions where the widget was the only candidate
    ///
    /// INV-WL1: Widget destruction cancels gesture.
    /// INV-WL2: No dangling widget IDs remain after this call.
    ///
    /// This must be called AFTER the widgets are removed from the tree but
    /// while the EventRouter still has their state.
    pub fn on_widgets_destroyed(
        &mut self,
        widget_ids: &[TreeWidgetId],
        domain_state: &mut InteractionDomainState,
    ) {
        if domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
            .is_some_and(|activation| widget_ids.contains(&activation.binding().capture_owner()))
        {
            self.terminate_relative_pointer(
                None,
                domain_state,
                RelativePointerExit::Cancelled(RelativePointerCancellationReason::OwnerUnmounted),
                None,
            );
        }
        for &widget_id in widget_ids {
            if domain_state.scroll_capture_owner == Some(widget_id) {
                if let Some(source) = domain_state.touch_scroll_source() {
                    self.cancel_pointer_session(
                        source.press.pointer,
                        CancellationReason::WidgetDestroyed,
                        domain_state,
                    );
                } else {
                    domain_state.clear_wheel_scroll_capture();
                }
            }
            tracing::debug!(
                "SPEC-HAL-128 Stage 6: Processing destruction of widget {:?}",
                widget_id.0
            );

            self.restore_hidden_cursor_for_widget(widget_id);

            domain_state
                .gesture_registry
                .retire_dead_geometry_owner(widget_id);

            // INV-WL1: Release any pointer captures held by this widget (iterates ALL pointers internally)
            domain_state
                .pointer_capture_manager
                .release_all_for_widget(widget_id);

            // INV-WL1: Remove gesture candidates for this widget (iterates ALL sessions internally)
            domain_state
                .gesture_registry
                .remove_candidates_for_widget(widget_id);

            // Check if we need to clean up the legacy capture too
            if domain_state.pointer_capture.captured_widget == Some(widget_id) {
                domain_state.pointer_capture.release();
            }
        }

        // INV-WL2 verification (debug only): ensure no dangling references for ANY pointer
        #[cfg(debug_assertions)]
        for &widget_id in widget_ids {
            debug_assert!(
                !domain_state
                    .gesture_registry
                    .has_candidate_for_widget(widget_id),
                "SPEC-HAL-128 Stage 6 INV-WL2: Dangling candidate for destroyed widget {:?}",
                widget_id.0
            );
            // SPEC-HAL-128 Stage 6: Check ALL active pointers, not just MOUSE
            for pointer_id in domain_state.pointer_capture_manager.active_pointers() {
                debug_assert!(
                    domain_state
                        .pointer_capture_manager
                        .current_capture(pointer_id)
                        != Some(widget_id),
                    "SPEC-HAL-128 Stage 6 INV-WL2: Dangling capture for pointer {:?} on destroyed widget {:?}",
                    pointer_id.0,
                    widget_id.0
                );
            }
        }
    }

    /// SPEC-HAL-128 Stage 5 INV-PC6: Validate capture target before dispatch.
    ///
    /// If the captured widget no longer exists in the tree, cancels the capture.
    /// Returns the valid capture target, or None if capture was released.
    ///
    /// This is a guard called before dispatching captured events.
    pub fn validate_capture_target(
        &mut self,
        tree: &WidgetTree,
        pointer_id: PointerId,
        domain_state: &mut InteractionDomainState,
    ) -> Option<TreeWidgetId> {
        let captured_id = domain_state
            .pointer_capture_manager
            .current_capture(pointer_id)?;

        // Check if widget still exists in tree
        if tree.find_node_by_widget_id(captured_id).is_some() {
            Some(captured_id)
        } else {
            // Widget no longer exists - cancel capture
            tracing::warn!(
                "SPEC-HAL-128 Stage 5: Captured widget {:?} no longer in tree, cancelling",
                captured_id.0
            );
            self.cancel_pointer_session(
                pointer_id,
                CancellationReason::WidgetDestroyed,
                domain_state,
            );
            None
        }
    }

    // ========================================================================
    // SPEC-HAL-131E: Projected Input Dispatch
    // ========================================================================

    /// SPEC-HAL-131E: Dispatch a projected action based on a hit result.
    ///
    /// This is the single entry point for routing actions that target logical
    /// items in virtualizing containers. It handles both realized and unrealized
    /// items correctly.
    ///
    /// Routing rules:
    /// - If `realized_widget_id` exists → route to that widget directly
    /// - If unrealized → route to container via `handle_projected_action`
    ///
    /// Invariants:
    /// - INV-NO-IMPLICIT-REALIZE-1: No realization during dispatch
    /// - INV-STATE-IDENTITY-1: Identity-based routing only
    ///
    /// # Arguments
    /// * `tree` - The widget tree
    /// * `hit` - The projected hit result from input projection
    /// * `action` - The action to perform
    ///
    /// # Returns
    /// `true` if the action was handled, `false` otherwise.

    /// SPEC-HAL-131E: Helper for projected focus navigation.
    ///
    /// Creates a new hit with updated logical_index/logical_item_id and dispatches
    /// ProjectedAction::Focus. Returns the updated hit on success.
    ///
    /// Used by Up/Down and PageUp/PageDown/Home/End navigation paths.
    fn dispatch_projected_focus_to_index(
        &mut self,
        tree: &mut WidgetTree,
        base_hit: &crate::core::input_projection::ProjectedHitResult,
        next_index: usize,
        domain_state: &mut InteractionDomainState,
    ) -> Option<crate::core::input_projection::ProjectedHitResult> {
        let mut new_hit = base_hit.clone();
        new_hit.logical_index = Some(next_index);
        new_hit.logical_item_id = Some(crate::core::input_projection::LogicalItemId::new(
            next_index as u64,
        ));

        let action = crate::core::input_projection::ProjectedAction::Focus;
        if self.dispatch_projected_input(tree, &new_hit, action, domain_state) {
            Some(new_hit)
        } else {
            None
        }
    }

    pub fn dispatch_projected_input(
        &mut self,
        tree: &mut WidgetTree,
        hit: &crate::core::input_projection::ProjectedHitResult,
        action: crate::core::input_projection::ProjectedAction,
        domain_state: &mut InteractionDomainState,
    ) -> bool {
        use crate::core::input_projection::ProjectedAction;
        use crate::core::widget::EventContext;

        let tree_ptr = tree as *mut WidgetTree;
        let clipboard_ptr = self.clipboard_ptr();

        // SPEC-HAL-131E: Route based on realization status
        if let Some(realized_id) = hit.realized_widget_id {
            // Realized path: dispatch to the realized widget directly
            // Convert projected action to appropriate event/call
            if let Some(node_id) = tree.find_node_by_widget_id(realized_id) {
                if let Some(node) = tree.node_mut(node_id) {
                    let mut cx = EventContext::new(
                        realized_id,
                        tree_ptr,
                        self.frame_time_ms,
                        clipboard_ptr,
                        None, // INV-TEXT-GEOM-PHASE: text_engine not needed for lifecycle callbacks
                        None, // SPEC-HAL-021 Stage 2.5: Animation optional
                        false, // Capture not relevant for projected actions
                        false, // SPEC-HAL-268: Not relevant for projected actions
                        domain_state.keyboard_mode,
                    );

                    match action {
                        ProjectedAction::Select { .. } | ProjectedAction::Focus => {
                            // For realized widgets, we still route to container
                            // because selection/focus state is owned by the container
                            // Fall through to container routing below
                        }
                        ProjectedAction::Activate => {
                            // Activate can go directly to the realized widget
                            if node.widget.accepts_keyboard_activation() {
                                node.widget.on_keyboard_activate(&mut cx);
                                return true;
                            }
                        }
                    }
                }
            }
        }

        // Unrealized path OR realized with container-owned state:
        // Route to container with logical identity
        let container_id = hit.container_id;
        let logical_id = match hit.logical_item_id {
            Some(id) => id,
            None => {
                tracing::warn!(
                    "SPEC-HAL-131E: Cannot dispatch to container without logical_item_id"
                );
                return false;
            }
        };
        let logical_index = hit.logical_index.unwrap_or(logical_id.0 as usize);

        // Find container and dispatch
        if let Some(node_id) = tree.find_node_by_widget_id(container_id) {
            if let Some(node) = tree.node_mut(node_id) {
                let mut cx = EventContext::new(
                    container_id,
                    tree_ptr,
                    self.frame_time_ms,
                    clipboard_ptr,
                    None,  // INV-TEXT-GEOM-PHASE: text_engine not needed for lifecycle callbacks
                    None,  // SPEC-HAL-021 Stage 2.5: Animation optional
                    false, // Capture not relevant for projected actions
                    false, // SPEC-HAL-268: Not relevant for projected actions
                    domain_state.keyboard_mode,
                );

                // INV-NO-IMPLICIT-REALIZE-1: handle_projected_action must not trigger realization
                let handled =
                    node.widget
                        .handle_projected_action(&mut cx, logical_id, logical_index, action);

                if handled {
                    tracing::debug!(
                        "SPEC-HAL-131E: Projected action {:?} handled by container {:?} for logical_id {:?}",
                        action,
                        container_id.0,
                        logical_id.0
                    );

                    // INV-DRAG-CAPTURE-1: Set up pointer capture ONLY when starting drag (Shift+Click).
                    // Without capture, PointerUp/Move won't be delivered during drag.
                    // Plain click doesn't need capture - no drag to track.
                    if let ProjectedAction::Select { shift, ctrl_or_cmd } = action {
                        // Drag only starts with Shift and NOT Ctrl
                        if shift && !ctrl_or_cmd {
                            // INV-079-NO-CAPTURE-OUTSIDE-ROUTING: Capture request MUST be in Routing phase
                            #[cfg(debug_assertions)]
                            debug_assert!(
                                self.current_phase.get() == Some(InputPipelinePhase::Routing),
                                "INV-079-NO-CAPTURE-OUTSIDE-ROUTING: projected capture outside Routing"
                            );
                            if let Err(e) = domain_state
                                .pointer_capture_manager
                                .request_capture(PointerId::MOUSE, container_id)
                            {
                                tracing::warn!(
                                    "SPEC-HAL-131E: Capture request denied for {:?}: {:?}",
                                    container_id.0,
                                    e
                                );
                            } else {
                                // Keep legacy pointer_capture in sync
                                domain_state
                                    .pointer_capture
                                    .capture(container_id, PointerButton::Primary);
                            }
                        }
                    }
                }

                return handled;
            }
        }

        tracing::warn!(
            "SPEC-HAL-131E: Container {:?} not found for projected action",
            container_id.0
        );
        false
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPEC-HAL-038: Focus System Integration
// ============================================================================

impl EventRouter {
    /// SPEC-HAL-038: Request focus for a widget, handling lifecycle (on_blur/on_focus).
    ///
    /// This is the proper way to change focus - it:
    /// 1. Calls on_blur on the previously focused widget (if any)
    /// 2. Updates the focus manager
    /// 3. Calls on_focus on the newly focused widget
    ///
    /// Note: Does NOT check if widget is focusable - caller should check first.
    pub fn request_focus_for_widget(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        domain_state: &mut InteractionDomainState,
    ) {
        self.request_focus_with_text_engine(tree, widget_id, domain_state, None);
    }

    pub(crate) fn note_keyboard_focus_cue(
        &mut self,
        mount: Option<WidgetMountToken>,
        domain_state: &mut InteractionDomainState,
    ) {
        self.continuous_frames_requested |=
            domain_state.focus_manager.note_keyboard_focus_cue(mount);
    }

    pub(crate) fn request_focus_with_text_engine(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        domain_state: &mut InteractionDomainState,
        mut text_engine: Option<&mut dyn crate::core::text_engine::TextEngine>,
    ) {
        if tree
            .find_node_by_widget_id(widget_id)
            .and_then(|node| tree.node(node))
            .and_then(|node| crate::core::widget_availability::effective(node.widget.as_ref()))
            .is_some_and(|policy| !policy.enabled)
        {
            return;
        }

        let (old_focus, new_focus) = domain_state.focus_manager.request_focus(widget_id);

        // If no change, nothing to do
        if old_focus.is_none() && new_focus.is_none() {
            // SPEC-HAL-326 §12.x INV-F9: Clear pending directional tag even
            // on no-op so a stale tag doesn't leak to a later non-directional
            // request_focus_for_widget call.
            self.pending_focus_entry_direction = None;
            return;
        }

        // SPEC-HAL-328 §15: framework guarantees a continuous frame on focus change.
        self.continuous_frames_requested = true;

        // INV-FOCUS-ENTRY-SCROLL-NEUTRALITY: When entering focus (old_focus == None),
        // ensure any stale pending_focus_reveal is cleared. This can happen when:
        // 1. Focus was cleared explicitly (via clear_focus)
        // 2. Focus was cleared implicitly (via rebuild_focus_chain when widget left scope)
        // In both cases, the pending reveal is no longer valid.
        if old_focus.is_none() {
            self.pending_focus_reveal = None;
        }

        // Release Capture when focus moves away from a Capture-mode widget
        if old_focus.is_some()
            && domain_state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture
        {
            domain_state.keyboard_mode = crate::core::event::KeyboardAuthorityMode::Navigation;
            if let Some(old_id) = old_focus {
                if tree.find_node_by_widget_id(old_id).is_some() {
                    self.invoke_composite_lifecycle(
                        tree,
                        old_id,
                        CompositeLifecycle::CaptureLeave,
                        domain_state,
                        text_engine.as_deref_mut().map(|engine| engine as *mut _),
                        false,
                        false,
                    );
                }
            }
        }

        // Call on_blur on old focused widget
        if let Some(old_id) = old_focus {
            if tree.find_node_by_widget_id(old_id).is_some() {
                self.invoke_composite_lifecycle(
                    tree,
                    old_id,
                    CompositeLifecycle::Blur,
                    domain_state,
                    text_engine.as_deref_mut().map(|engine| engine as *mut _),
                    false,
                    false,
                );
            }
        }

        // Call on_focus on new focused widget
        if let Some(new_id) = new_focus {
            if tree.find_node_by_widget_id(new_id).is_some() {
                self.invoke_composite_lifecycle(
                    tree,
                    new_id,
                    CompositeLifecycle::Focus,
                    domain_state,
                    text_engine.as_deref_mut().map(|engine| engine as *mut _),
                    false,
                    false,
                );
            }

            // INV-FOCUS-ENTRY-SCROLL-NEUTRALITY (SPEC-HAL-038):
            // Focus acquisition MUST NOT mutate scroll state unless an explicit
            // reveal request exists. When old_focus is None (focus entry), there
            // is no prior focus context, so no reveal should occur.
            // Pointer-originated focus entry has zero scroll authority.
            //
            // Only schedule reveal when CHANGING focus (old_focus.is_some()),
            // not when establishing focus for the first time.
            if old_focus.is_some() && domain_state.touch_scroll_source().is_none() {
                self.pending_focus_reveal = Some(new_id);
            }

            // INV-FOCUS-ENTRY-SCROLL-NEUTRALITY: Debug assertion (debug/test builds only)
            // Engine-global invariant: focus entry (old_focus == None) MUST NOT set pending_focus_reveal.
            // This applies to ALL widgets, ALL containers, ALL focus sources.
            // See SPEC-HAL-038 §12.3.
            debug_assert!(
                !(old_focus.is_none() && self.pending_focus_reveal.is_some()),
                "INV-FOCUS-ENTRY-SCROLL-NEUTRALITY violated: pending_focus_reveal was set \
                 when old_focus was None. Focus entry must be scroll-neutral."
            );
        }

        // SPEC-HAL-131E: Invalidate projected hit when focus leaves the container
        // Prevents "ghost navigation" on a stale projected context
        if let Some(ref hit) = self.last_projected_hit {
            if new_focus != Some(hit.container_id) {
                self.last_projected_hit = None;
            }
        }
    }

    /// SPEC-HAL-326 §12.x INV-F9: Directional variant of
    /// `request_focus_for_widget`. Tags the upcoming `on_focus`
    /// dispatch with the direction the user is navigating from, so
    /// composite widgets with internal focus targets (RadioGroup,
    /// TabBar, etc.) can choose the spatially-correct internal entry
    /// point.
    ///
    /// Identical to `request_focus_for_widget` in every other respect:
    /// fires `on_blur` on the old focus, fires `on_focus` on the new
    /// focus and manages scroll reveal.
    pub fn request_focus_for_widget_directional(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        direction: Direction,
        domain_state: &mut InteractionDomainState,
    ) {
        self.pending_focus_entry_direction = Some(direction);
        self.request_focus_for_widget(tree, widget_id, domain_state);
    }

    /// SPEC-HAL-260: Enter keyboard Capture mode for a widget.
    ///
    /// Sets domain keyboard_mode to Capture AND calls on_capture_enter()
    /// on the widget. Used by deferred focus after layout mounts a
    /// widget that needs immediate editing.
    pub fn enter_capture_for_widget(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        node_id: crate::core::widget_tree::NodeId,
        domain_state: &mut InteractionDomainState,
    ) {
        self.enter_capture_with_text_engine(tree, widget_id, node_id, domain_state, None);
    }

    pub(crate) fn enter_capture_with_text_engine(
        &mut self,
        tree: &mut WidgetTree,
        widget_id: TreeWidgetId,
        node_id: crate::core::widget_tree::NodeId,
        domain_state: &mut InteractionDomainState,
        mut text_engine: Option<&mut dyn crate::core::text_engine::TextEngine>,
    ) {
        if !tree
            .node(node_id)
            .is_some_and(|node| {
                node.id == widget_id
                    && node.widget.capture_eligible()
                    && tree.composite_choice_input_ready(node)
            })
        {
            return;
        }

        domain_state.keyboard_mode = crate::core::event::KeyboardAuthorityMode::Capture;
        self.invoke_composite_lifecycle(
            tree,
            widget_id,
            CompositeLifecycle::CaptureEnter,
            domain_state,
            text_engine.as_deref_mut().map(|engine| engine as *mut _),
            false,
            false,
        );
        // SPEC-HAL-328 §15: framework guarantees a continuous frame on
        // pointer-initiated capture entry.
        self.continuous_frames_requested = true;
    }

    /// SPEC-HAL-038: Clear focus from the currently focused widget.
    ///
    /// Calls on_blur on the currently focused widget (if any) and clears focus.
    pub fn clear_focus(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
    ) {
        self.clear_focus_with_text_engine(tree, domain_state, None);
    }

    pub(crate) fn clear_focus_with_text_engine(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
        mut text_engine: Option<&mut dyn crate::core::text_engine::TextEngine>,
    ) {
        if let Some(old_id) = domain_state.focus_manager.clear_focus() {
            // Release Capture mode when focus is cleared (e.g. window deactivation)
            if domain_state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture {
                domain_state.keyboard_mode = crate::core::event::KeyboardAuthorityMode::Navigation;
                if tree.find_node_by_widget_id(old_id).is_some() {
                    self.invoke_composite_lifecycle(
                        tree,
                        old_id,
                        CompositeLifecycle::CaptureLeave,
                        domain_state,
                        text_engine.as_deref_mut().map(|engine| engine as *mut _),
                        false,
                        false,
                    );
                }
            }

            if tree.find_node_by_widget_id(old_id).is_some() {
                self.invoke_composite_lifecycle(
                    tree,
                    old_id,
                    CompositeLifecycle::Blur,
                    domain_state,
                    text_engine.as_deref_mut().map(|engine| engine as *mut _),
                    false,
                    false,
                );
            }
            // INV-FOCUS-ENTRY-SCROLL-NEUTRALITY: Clear pending focus reveal when focus is cleared.
            // The reveal target is no longer valid since focus has been cleared.
            self.pending_focus_reveal = None;
        }
    }

    /// SPEC-HAL-137 INV-OVL-INPUT-1 / SPEC-HAL-137B
    /// INV-OVL-FOCUS-TRAP-PARITY-1: modal keyboard routing must not target
    /// a foreign widget from the base tree.
    pub fn enforce_modal_input_boundary(
        &mut self,
        tree: &mut WidgetTree,
        modal_root: crate::core::widget_tree::NodeId,
        domain_state: &mut InteractionDomainState,
    ) {
        self.enforce_modal_input_boundary_with_text_engine(tree, modal_root, domain_state, None);
    }

    pub(crate) fn enforce_modal_input_boundary_with_text_engine(
        &mut self,
        tree: &mut WidgetTree,
        modal_root: crate::core::widget_tree::NodeId,
        domain_state: &mut InteractionDomainState,
        text_engine: Option<&mut dyn crate::core::text_engine::TextEngine>,
    ) {
        if let Some(focused_id) = domain_state.focus_manager.focused() {
            if !tree.is_descendant_of_node(focused_id, modal_root) {
                self.clear_focus_with_text_engine(tree, domain_state, text_engine);
            }
        }
    }

    /// SPEC-HAL-137B INV-OVL-FOCUS-TRAP-PARITY-1: when a modal focus scope is
    /// active, keyboard focus must enter and remain inside that scope.
    pub fn ensure_focus_within_active_scope(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
    ) {
        if domain_state.focus_manager.availability_keeps_focus_empty() {
            return;
        }
        let Some(scope_root) = domain_state.focus_manager.active_scope() else {
            return;
        };

        if let Some(focused_id) = domain_state.focus_manager.focused() {
            if tree.is_descendant_of_node(focused_id, scope_root) {
                return;
            }
            self.clear_focus(tree, domain_state);
        }

        domain_state
            .focus_manager
            .rebuild_focus_chain(tree, scope_root);
        if let Some(target_id) = domain_state.focus_manager.focus_chain_at(0) {
            self.request_focus_for_widget(tree, target_id, domain_state);
        }
    }

    /// Notify the focused widget that it has lost Capture mode.
    /// Called when Escape releases Capture in app.rs.
    pub fn notify_capture_leave(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) {
        self.notify_capture_leave_with_availability(tree, domain_state, text_engine, false);
    }

    pub(crate) fn notify_capture_leave_with_availability(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        availability: bool,
    ) {
        let Some(focused_id) = domain_state.focus_manager.focused() else {
            return;
        };
        self.invoke_composite_lifecycle(
            tree,
            focused_id,
            CompositeLifecycle::CaptureLeave,
            domain_state,
            Some(text_engine as *mut _),
            availability,
            false,
        );
    }

    pub(crate) fn physical_keyboard_capture_owner(
        tree: &WidgetTree,
        state: &InteractionDomainState,
    ) -> Option<TreeWidgetId> {
        if state.keyboard_mode != crate::core::event::KeyboardAuthorityMode::Capture {
            return None;
        }
        let owner = state.focus_manager.focused()?;
        let node = tree.node(tree.find_node_by_widget_id(owner)?)?;
        (node.widget.capture_eligible() && node.widget.physical_keyboard_capture()).then_some(owner)
    }

    pub(crate) fn interrupt_physical_keyboard_capture(
        &mut self,
        tree: &mut WidgetTree,
        state: &mut InteractionDomainState,
        interruption: crate::core::event::KeyboardCaptureInterruption,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) -> bool {
        let Some(owner) = Self::physical_keyboard_capture_owner(tree, state) else {
            return false;
        };
        let node_id = tree
            .find_node_by_widget_id(owner)
            .expect("live capture owner");
        let tree_ptr = tree as *mut WidgetTree;
        let mut cx = crate::core::widget::EventContext::new(
            owner,
            tree_ptr,
            self.frame_time_ms,
            self.clipboard_ptr(),
            Some(text_engine),
            None,
            false,
            false,
            state.keyboard_mode,
        );
        tree.node_mut(node_id)
            .expect("live capture owner")
            .widget
            .on_keyboard_capture_interrupted(&interruption, &mut cx);
        assert!(
            !cx.is_keyboard_capture_requested() && !cx.is_focus_requested(),
            "PHYSICAL-KEY-INTERRUPTION: settlement cannot recapture or change focus"
        );
        self.handle_engine_action(
            tree,
            crate::runtime::actions::ActionId::RELEASE_CAPTURE,
            None,
            state,
            text_engine,
            None,
        );
        true
    }

    /// SPEC-HAL-038: Check if a widget is focusable.
    fn is_widget_focusable(&self, tree: &WidgetTree, widget_id: TreeWidgetId) -> bool {
        if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
            if let Some(node) = tree.node(node_id) {
                return node.widget.focusable();
            }
        }
        false
    }

    /// INV-FOCUS-ON-CLICK-ANCESTOR-1: Find nearest focusable ancestor.
    ///
    /// On primary pointer down, if the hit target is not focusable, walk ancestors
    /// to find a focusable one. Returns None if no focusable ancestor exists.
    fn find_focusable_ancestor(
        &self,
        tree: &WidgetTree,
        widget_id: TreeWidgetId,
    ) -> Option<TreeWidgetId> {
        let mut node_id = tree.find_node_by_widget_id(widget_id)?;

        loop {
            let node = tree.node(node_id)?;

            // Check if this node is focusable
            if node.widget.focusable() {
                return Some(node.id);
            }

            // Move to parent
            node_id = node.parent?;
        }
    }

    /// SPEC-HAL-038: Check if a widget exists in the tree.
    fn widget_exists(&self, tree: &WidgetTree, widget_id: TreeWidgetId) -> bool {
        tree.find_node_by_widget_id(widget_id).is_some()
    }

    /// SPEC-HAL-038 Stage 10: Try to restore focus after window activation.
    ///
    /// Conditions for restoration:
    /// - Widget must exist in the tree
    /// - Widget must return focusable() == true
    /// - No focus scope is active (modal would block restoration)
    /// - No widget is currently focused (avoid overwriting user-triggered focus)
    ///
    /// If restoration succeeds, the memory is consumed. If it fails, memory is preserved
    /// for a potential later attempt.
    fn try_restore_focus(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
    ) {
        // Check preconditions before consuming the memory
        let Some(last_focus) = domain_state.focus_manager.take_last_focus() else {
            return;
        };

        // Condition 1: No widget currently focused (don't overwrite user-triggered focus)
        if domain_state.focus_manager.focused().is_some() {
            // DO NOT re-store memory — user intent overrides memory
            return;
        }

        // Condition 2: No focus scope is active
        if domain_state.focus_manager.active_scope().is_some() {
            // memory already consumed by take_last_focus(); do NOT restore it
            return;
        }

        // Condition 3: Widget must exist
        if !self.widget_exists(tree, last_focus) {
            return;
        }

        // Condition 4: Widget must be focusable
        if !self.is_widget_focusable(tree, last_focus) {
            return;
        }

        // All conditions passed - restore focus
        self.request_focus_for_widget(tree, last_focus, domain_state);
    }

    /// SPEC-HAL-038 Stage 2: Handle Tab key navigation.
    ///
    /// Rebuilds focus chain, computes next/prev target, and changes focus.
    /// Returns true if Tab was handled (focus chain not empty).
    ///
    /// SPEC-HAL-038 Stage 3: If a focus scope is active, Tab navigation is
    /// constrained to that scope's subtree (handled inside rebuild_focus_chain).
    ///
    /// INV-FOCUS-VIEWPORT-ENTRY: When no widget is focused (focus entry via TAB),
    /// the target is the first *visible* focusable widget in the viewport, not tree order.
    ///
    /// # Arguments
    /// * `tree` - The widget tree
    /// * `root` - The root node for focus chain enumeration
    /// * `reverse` - If true, navigate backwards (Shift+Tab)
    pub fn handle_tab_navigation(
        &mut self,
        tree: &mut WidgetTree,
        root: crate::core::widget_tree::NodeId,
        reverse: bool,
        domain_state: &mut InteractionDomainState,
    ) -> bool {
        // SPEC-HAL-038 Stage 3: Use active scope as effective root if present
        let effective_root = domain_state.focus_manager.active_scope().unwrap_or(root);

        // INV-FOCUS-VIEWPORT-ENTRY: Check if this is focus entry (no current focus)
        let is_focus_entry = domain_state.focus_manager.focused().is_none();

        if is_focus_entry {
            // Focus entry: use viewport-aware selection
            //
            // INV-FOCUS-COORDSPACE: Use the ACTUAL ROOT's rect as viewport (window bounds),
            // not the effective_root (focus scope). The viewport must be in screen space.
            // SPEC-HAL-141: Use getter for private geometry field
            let viewport = tree.get_absolute_paint_rect(root).unwrap_or_default();

            // Navigation does not use focus memory. On focus entry, use
            // viewport-aware selection inside the current effective root.
            if let Some(target_id) = domain_state.focus_manager.first_visible_focusable(
                tree,
                effective_root,
                viewport,
                reverse,
            ) {
                self.request_focus_for_widget(tree, target_id, domain_state);
                return true;
            }

            // Fallback: no visible focusables, use tree-order.
            domain_state.focus_manager.rebuild_focus_chain(tree, root);
            let target = if reverse {
                domain_state.focus_manager.tab_prev()
            } else {
                domain_state.focus_manager.tab_next()
            };
            if let Some(target_id) = target {
                self.request_focus_for_widget(tree, target_id, domain_state);
                return true;
            }
            return false;
        }

        // Focus traversal: use existing tree-order navigation
        // Rebuild focus chain before navigation
        // SPEC-HAL-038 Stage 3: rebuild_focus_chain uses active_scope() as
        // effective root if a scope is active
        domain_state.focus_manager.rebuild_focus_chain(tree, root);

        // Get next/prev target
        let target = if reverse {
            domain_state.focus_manager.tab_prev()
        } else {
            domain_state.focus_manager.tab_next()
        };

        // If we have a target, focus it
        if let Some(target_id) = target {
            self.request_focus_for_widget(tree, target_id, domain_state);
            true
        } else {
            // No focusable widgets
            false
        }
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 3: Focus Scope Management
    // ========================================================================

    /// SPEC-HAL-038 Stage 3: Push a focus scope onto the stack.
    ///
    /// Call this when opening a modal or overlay that should trap Tab focus.
    /// The scope root should be the NodeId of the modal's root widget.
    pub fn push_focus_scope(
        &mut self,
        scope_root: crate::core::widget_tree::NodeId,
        domain_state: &mut InteractionDomainState,
    ) {
        domain_state.focus_manager.push_scope(scope_root);
        tracing::debug!(
            "SPEC-HAL-038 Stage 4: Pushed focus scope, stack depth={}",
            domain_state.focus_manager.scope_depth()
        );
    }

    /// SPEC-HAL-038 Stage 3: Pop the active focus scope.
    ///
    /// Call this when closing a modal or overlay. If the currently focused
    /// widget was inside the popped scope, focus is cleared.
    /// SPEC-HAL-038 Stage 7: Also clears hover if it was inside the popped scope.
    pub fn pop_focus_scope(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
    ) {
        // Check if focus/hover was inside the scope before popping
        let (should_clear_focus, should_clear_hover) =
            if let Some(scope_root) = domain_state.focus_manager.active_scope() {
                let focus_in_scope = domain_state
                    .focus_manager
                    .is_focused_in_scope(tree, scope_root);
                let hover_in_scope = domain_state
                    .hovered_widget
                    .map(|h| tree.is_descendant_of_node(h, scope_root))
                    .unwrap_or(false);
                (focus_in_scope, hover_in_scope)
            } else {
                (false, false)
            };

        // Pop the scope
        if let Some(popped) = domain_state.focus_manager.pop_scope() {
            tracing::debug!(
                "SPEC-HAL-038 Stage 3/7: Popped focus scope {:?}, clear_focus={}, clear_hover={}",
                popped,
                should_clear_focus,
                should_clear_hover
            );

            // Clear focus if it was inside the popped scope
            if should_clear_focus {
                self.clear_focus(tree, domain_state);
            }

            // SPEC-HAL-038 Stage 7: Clear hover if it was inside the popped scope
            if should_clear_hover {
                self.clear_hover(tree, domain_state);
            }
        }
    }

    /// SPEC-HAL-038 Stage 3/4: Synchronize focus scopes with modal state.
    ///
    /// Clears the scope stack and rebuilds it from the given modal mounted nodes.
    /// SPEC-HAL-038 Stage 4: Only nodes whose widgets return `is_focus_scope() == true`
    /// become actual scope roots. Non-scope modals are ignored.
    ///
    /// If the currently focused widget is no longer in any scope, focus is preserved
    /// (it will be in the global scope).
    ///
    /// Call this after rebuild_active_view to sync scopes with current modals.
    pub fn sync_focus_scopes(
        &mut self,
        tree: &mut WidgetTree,
        modal_nodes: &[crate::core::widget_tree::NodeId],
        domain_state: &mut InteractionDomainState,
    ) {
        // Early exit: no modals and no existing scopes means nothing to sync
        if modal_nodes.is_empty() && domain_state.focus_manager.scope_depth() == 0 {
            return;
        }

        // Track if focus was in the old scope stack
        let old_focused_in_scope =
            if let Some(scope_root) = domain_state.focus_manager.active_scope() {
                domain_state
                    .focus_manager
                    .is_focused_in_scope(tree, scope_root)
            } else {
                false
            };

        // Clear old scopes
        while domain_state.focus_manager.pop_scope().is_some() {}

        // SPEC-HAL-038 Stage 4: Filter to only nodes that are actual focus scopes
        let scope_nodes = tree.filter_focus_scopes(modal_nodes);

        // Push new scopes (bottom-to-top order)
        for node in &scope_nodes {
            domain_state.focus_manager.push_scope(*node);
        }

        // SPEC-HAL-038 Stage 4: Clear focus when leaving all scopes
        // If focus was inside a scope and now there are no scopes, clear focus.
        // This ensures that when a modal closes, focus doesn't remain on a widget
        // that was inside the modal.
        if old_focused_in_scope && scope_nodes.is_empty() {
            self.clear_focus(tree, domain_state);
        }

        tracing::debug!(
            "SPEC-HAL-038 Stage 4: Synced focus scopes, candidates={}, actual={}",
            modal_nodes.len(),
            scope_nodes.len()
        );
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 5: ESC Focus Reset
    // SPEC-HAL-128 Stage 4: ESC Cancellation (INV-PC4)
    // ========================================================================

    /// SPEC-HAL-038 Stage 5 / SPEC-HAL-128 Stage 4: Handle ESC key press.
    /// SPEC-HAL-128 Stage 6: ESC cancels ALL pointers (intentional global override).
    ///
    /// Behavior (in priority order):
    /// 1. SPEC-HAL-128 INV-PC4: If ANY gesture session active OR ANY pointer captured → cancel all
    /// 2. If a focus scope is active (modal/dialog): pop the scope
    /// 3. Else: return false (ESC unhandled; root-scope no-op)
    ///
    /// Returns true if ESC was handled.
    pub fn handle_escape(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
    ) -> bool {
        // SPEC-HAL-128 Stage 6 INV-PC4: First, check if ANY active gesture sessions or captures exist
        // Escape is an intentional global override that cancels ALL pointers.
        let has_any_session = !domain_state.gesture_registry.active_pointers().is_empty();
        let has_any_capture = !domain_state
            .pointer_capture_manager
            .active_pointers()
            .is_empty();

        if has_any_session || has_any_capture {
            // SPEC-HAL-128 Stage 6/8: Cancel ALL pointer sessions with callbacks
            self.cancel_all_pointer_sessions_with_tree(
                tree,
                CancellationReason::Escape,
                domain_state,
            );
            tracing::debug!(
                "SPEC-HAL-128 Stage 6: ESC cancelled ALL pointer sessions (had_sessions={}, had_captures={})",
                has_any_session,
                has_any_capture
            );
            return true;
        }

        // If a scope is active, pop it
        if domain_state.focus_manager.active_scope().is_some() {
            self.pop_focus_scope(tree, domain_state);
            tracing::debug!("SPEC-HAL-038 Stage 5: ESC popped focus scope");
            return true;
        }

        // SPEC-HAL-328 §6 / INV-KBD-MODE-5: Escape in Navigation at root
        // scope is a NO-OP. Focus does not change.
        false
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 5: Directional Navigation
    // ========================================================================

    /// SPEC-HAL-038 Stage 5: Navigate focus directionally using arrow keys.
    ///
    /// Algorithm:
    /// 1. Get effective scope root (active scope or global root)
    /// 2. Rebuild focus chain within scope
    /// 3. Get current focused widget's geometry
    /// 4. Filter candidates in the specified direction
    /// 5. Rank by: perpendicular offset (primary), travel distance (secondary), DFS order (tiebreaker)
    /// 6. Focus the closest candidate
    ///
    /// Returns true if navigation occurred.
    pub fn navigate_directional(
        &mut self,
        tree: &mut WidgetTree,
        root: crate::core::widget_tree::NodeId,
        direction: Direction,
        domain_state: &mut InteractionDomainState,
    ) -> bool {
        // Rebuild focus chain (respects active scope)
        domain_state.focus_manager.rebuild_focus_chain(tree, root);

        // Get current focused widget
        let Some(focused_id) = domain_state.focus_manager.focused() else {
            // SPEC-HAL-328 §8.5: arrows with no focused widget enter at
            // the first focusable widget in the current scope.
            if let Some(target_id) = domain_state.focus_manager.tab_next() {
                self.request_focus_for_widget(tree, target_id, domain_state);
                return true;
            }
            return false;
        };

        // Get focused widget's absolute rect
        // SPEC-HAL-141: Use getter for private geometry field
        let Some(focused_node_id) = tree.find_node_by_widget_id(focused_id) else {
            return false;
        };
        let focused_rect = tree
            .get_absolute_paint_rect(focused_node_id)
            .unwrap_or_default();
        // SPEC-HAL-328 §8.1: edge-based directional filtering. A candidate
        // qualifies only if its leading edge in the search direction lies on
        // the far side of the focused widget's trailing edge.
        let focused_left = focused_rect.x;
        let focused_right = focused_rect.x + focused_rect.width;
        let focused_top = focused_rect.y;
        let focused_bottom = focused_rect.y + focused_rect.height;
        let focused_center_x = focused_rect.x + focused_rect.width / 2.0;
        let focused_center_y = focused_rect.y + focused_rect.height / 2.0;

        let mut candidates: Vec<(TreeWidgetId, f32, f32, usize)> = Vec::new();

        for (idx, &candidate_id) in domain_state.focus_manager.focus_chain().iter().enumerate() {
            if candidate_id == focused_id {
                continue;
            }

            let Some(node_id) = tree.find_node_by_widget_id(candidate_id) else {
                continue;
            };
            let rect = tree.get_absolute_paint_rect(node_id).unwrap_or_default();
            let cand_left = rect.x;
            let cand_right = rect.x + rect.width;
            let cand_top = rect.y;
            let cand_bottom = rect.y + rect.height;
            let center_x = rect.x + rect.width / 2.0;
            let center_y = rect.y + rect.height / 2.0;

            let in_direction = match direction {
                Direction::Up => cand_bottom <= focused_top,
                Direction::Down => cand_top >= focused_bottom,
                Direction::Left => cand_right <= focused_left,
                Direction::Right => cand_left >= focused_right,
            };

            if in_direction {
                candidates.push((candidate_id, center_x, center_y, idx));
            }
        }

        if candidates.is_empty() {
            return false;
        }

        candidates.sort_by(|a, b| {
            let (_, ax, ay, a_idx) = *a;
            let (_, bx, by, b_idx) = *b;

            match direction {
                Direction::Up | Direction::Down => {
                    // Primary: travel distance (y, minimize)
                    let travel_a = (ay - focused_center_y).abs();
                    let travel_b = (by - focused_center_y).abs();
                    if (travel_a - travel_b).abs() > 1.0 {
                        return travel_a.partial_cmp(&travel_b).unwrap();
                    }
                    // Tiebreaker: perpendicular distance (x, minimize)
                    let perp_a = (ax - focused_center_x).abs();
                    let perp_b = (bx - focused_center_x).abs();
                    if (perp_a - perp_b).abs() > 1.0 {
                        return perp_a.partial_cmp(&perp_b).unwrap();
                    }
                }
                Direction::Left | Direction::Right => {
                    // Primary: travel distance (x, minimize)
                    let travel_a = (ax - focused_center_x).abs();
                    let travel_b = (bx - focused_center_x).abs();
                    if (travel_a - travel_b).abs() > 1.0 {
                        return travel_a.partial_cmp(&travel_b).unwrap();
                    }
                    // Tiebreaker: perpendicular distance (y, minimize)
                    let perp_a = (ay - focused_center_y).abs();
                    let perp_b = (by - focused_center_y).abs();
                    if (perp_a - perp_b).abs() > 1.0 {
                        return perp_a.partial_cmp(&perp_b).unwrap();
                    }
                }
            }
            // Final tiebreaker: DFS order
            a_idx.cmp(&b_idx)
        });

        // Focus the best candidate
        let (target_id, _, _, _) = candidates[0];
        self.request_focus_for_widget_directional(tree, target_id, direction, domain_state);

        tracing::debug!(
            "SPEC-HAL-038 Stage 5: Directional {:?} from {:?} to {:?}",
            direction,
            focused_id.0,
            target_id.0
        );

        true
    }

    /// SPEC-HAL-326: Tree-structured directional navigation.
    ///
    /// Returns the TreeWidgetId to focus, or None if no
    /// tree-based navigation is possible (falls back to
    /// geometric).
    fn navigate_tree_structured(
        tree: &WidgetTree,
        focused_id: TreeWidgetId,
        direction: Direction,
    ) -> Option<TreeWidgetId> {
        let focused_node_id = tree.find_node_by_widget_id(focused_id)?;

        // SPEC-HAL-326 §3.5 / INV-FNAV-6 (v2): Sibling entry uses spatial
        // scoring against the focused widget's rect — directional edge
        // gap primary, perpendicular distance (sticky cursor) tiebreaker.
        // The full rect is propagated to navigate_sibling and
        // navigate_exit_container so the spatial pick has both the
        // crossing edge and the sticky cursor reference.
        let focused_rect = tree
            .get_absolute_paint_rect(focused_node_id)
            .unwrap_or_default();

        // Walk up from focused widget to find the nearest
        // ancestor with a layout_axis
        let (container_node_id, container_axis) = find_axis_container(tree, focused_node_id)?;

        // Determine if direction is along or across the axis
        let along_axis = match (container_axis, direction) {
            (Axis::Horizontal, Direction::Left | Direction::Right) => true,
            (Axis::Vertical, Direction::Up | Direction::Down) => true,
            _ => false,
        };

        if along_axis {
            // Navigate within container: next/prev sibling
            navigate_sibling(
                tree,
                container_node_id,
                focused_node_id,
                direction,
                focused_rect,
            )
            .or_else(|| {
                // At container boundary: exit to parent
                navigate_exit_container(tree, container_node_id, direction, focused_rect)
            })
        } else {
            // Navigate across container: exit to parent
            navigate_exit_container(tree, container_node_id, direction, focused_rect)
        }
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 6: Keyboard Activation
    // ========================================================================

    /// SPEC-HAL-038 Stage 6: Attempt to activate the focused widget via keyboard.
    ///
    /// Checks if the focused widget accepts keyboard activation (Enter/Space).
    /// If so, calls `on_keyboard_activate()` on the widget.
    ///
    /// Returns true if activation occurred, false otherwise.
    pub fn activate_focused_widget(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) -> bool {
        use crate::core::widget::EventContext;

        // Get currently focused widget
        let Some(focused_id) = domain_state.focus_manager.focused() else {
            return false;
        };

        // Resolve to NodeId
        let Some(node_id) = tree.find_node_by_widget_id(focused_id) else {
            return false;
        };

        // Check if widget accepts keyboard activation (immutable borrow first)
        let accepts = tree
            .node(node_id)
            .map(|n| n.widget.accepts_keyboard_activation())
            .unwrap_or(false);

        if !accepts {
            return false;
        }

        // Call on_keyboard_activate (mutable borrow)
        let tree_ptr = tree as *mut WidgetTree;
        let clipboard_ptr = self.clipboard_ptr();
        if let Some(node) = tree.node_mut(node_id) {
            let mut cx = EventContext::new(
                focused_id,
                tree_ptr,
                self.frame_time_ms,
                clipboard_ptr,
                None,      // INV-TEXT-GEOM-PHASE: text_engine not needed for lifecycle callbacks
                animation, // SPEC-HAL-021 Stage 4: animation capability for press-pulse feedback
                false,     // Capture not relevant for lifecycle callbacks
                false,     // SPEC-HAL-268: Not relevant for lifecycle callbacks
                domain_state.keyboard_mode,
            );
            node.widget.on_keyboard_activate(&mut cx);
            // Propagate continuous-frames request from the widget so the
            // visual update lands on the same frame as the activation.
            if cx.continuous_frames_requested() {
                self.continuous_frames_requested = true;
            }
            tracing::debug!(
                "SPEC-HAL-038 Stage 6: Activated TreeWidgetId({:?}) via keyboard",
                focused_id.0
            );
            return true;
        }

        false
    }

    // ========================================================================
    // SPEC-HAL-176: Engine Action Dispatch
    // ========================================================================

    /// SPEC-HAL-176: Execute an engine-level action.
    ///
    /// Called by process_frame when binding lookup resolves to an engine ActionId.
    /// The router has full context (tree, domain_state, focus) to execute these.
    ///
    /// Returns true if the action was handled.
    pub fn handle_engine_action(
        &mut self,
        tree: &mut WidgetTree,
        action_id: crate::runtime::actions::ActionId,
        root: Option<crate::core::widget_tree::NodeId>,
        domain_state: &mut InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) -> bool {
        use crate::core::event::KeyboardAuthorityMode;
        use crate::runtime::actions::ActionId;

        let focused_id = domain_state.focus_manager.focused();

        match action_id {
            ActionId::FOCUS_NEXT => {
                if let Some(root_id) = root {
                    self.handle_tab_navigation(tree, root_id, false, domain_state)
                } else {
                    false
                }
            }
            ActionId::FOCUS_PREV => {
                if let Some(root_id) = root {
                    self.handle_tab_navigation(tree, root_id, true, domain_state)
                } else {
                    false
                }
            }
            ActionId::ACTIVATE => self.activate_focused_widget(tree, domain_state, animation),
            ActionId::EXIT_SCOPE => self.handle_escape(tree, domain_state),
            ActionId::ENTER_WIDGET => {
                let Some(focused_id) = domain_state.focus_manager.focused() else {
                    return false;
                };
                let Some(node_id) = tree.find_node_by_widget_id(focused_id) else {
                    return false;
                };

                // Read widget declarations (immutable borrow)
                let (is_capture, is_activate, _is_scope) = {
                    let Some(node) = tree.node(node_id) else {
                        return false;
                    };
                    if !tree.composite_choice_input_ready(node) {
                        return false;
                    }
                    (
                        node.widget.capture_eligible(),
                        node.widget.accepts_keyboard_activation(),
                        node.widget.defines_focus_scope(),
                    )
                };

                if is_capture {
                    // Enter capture mode
                    domain_state.keyboard_mode = KeyboardAuthorityMode::Capture;
                    // Notify widget it entered Capture
                    self.invoke_composite_lifecycle(
                        tree,
                        focused_id,
                        CompositeLifecycle::CaptureEnter,
                        domain_state,
                        Some(text_engine as *mut _),
                        false,
                        false,
                    );
                    // SPEC-HAL-328 §15: framework guarantees a continuous frame
                    // on mode transition.
                    self.continuous_frames_requested = true;
                    tracing::debug!(
                        "SPEC-HAL-176: Entered Capture mode for widget {:?}",
                        focused_id
                    );
                    return true;
                }
                if is_activate {
                    // SPEC-HAL-328 §3.1 + §3.3: non-capturable widget with
                    // accepts_keyboard_activation() → fire activation.
                    return self.activate_focused_widget(tree, domain_state, animation);
                }
                // defines_focus_scope → Phase 3 (push scope, not yet implemented)
                false
            }
            ActionId::RELEASE_CAPTURE => {
                if domain_state.keyboard_mode == KeyboardAuthorityMode::Capture {
                    domain_state.keyboard_mode = KeyboardAuthorityMode::Navigation;
                    self.notify_capture_leave(tree, domain_state, text_engine);
                }
                // SPEC-HAL-328 §15: framework guarantees a continuous frame
                // on mode transition.
                self.continuous_frames_requested = true;
                tracing::debug!("SPEC-HAL-176: Released capture, returned to Navigation mode");
                true
            }
            ActionId::FOCUS_LEFT => {
                // Widget-first: always fall through to route_input_event.
                // The widget gets Left first (tab switching, slider values,
                // cursor movement). If widget returns false, tree navigation
                // runs in the route_input_event fallback path.
                false
            }
            ActionId::FOCUS_RIGHT => false,
            ActionId::FOCUS_UP => {
                if let Some(root_id) = root {
                    let tree_nav = focused_id
                        .and_then(|fid| Self::navigate_tree_structured(tree, fid, Direction::Up));
                    if let Some(target) = tree_nav {
                        self.request_focus_for_widget_directional(
                            tree,
                            target,
                            Direction::Up,
                            domain_state,
                        );
                        true
                    } else {
                        self.navigate_directional(tree, root_id, Direction::Up, domain_state)
                    }
                } else {
                    false
                }
            }
            ActionId::FOCUS_DOWN => {
                if let Some(root_id) = root {
                    let tree_nav = focused_id
                        .and_then(|fid| Self::navigate_tree_structured(tree, fid, Direction::Down));
                    if let Some(target) = tree_nav {
                        self.request_focus_for_widget_directional(
                            tree,
                            target,
                            Direction::Down,
                            domain_state,
                        );
                        true
                    } else {
                        self.navigate_directional(tree, root_id, Direction::Down, domain_state)
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 7: Hover Lifecycle
    // ========================================================================

    /// SPEC-HAL-038 Stage 7: Update hover state and fire lifecycle methods.
    /// SPEC-HAL-021 Stage 4: Animation capability injection for hover enter/leave.
    ///
    /// This handles the transition between hovered widgets:
    /// - If new_hovered differs from current, fire on_hover_leave on old and on_hover_enter on new
    /// - Respects focus scope boundaries (hover is cleared if new target is outside active scope)
    ///
    /// # Arguments
    /// * `tree` - The widget tree
    /// * `new_hovered` - The new hovered widget (if any)
    /// * `animation` - Optional animation capability from TransitionLayer
    ///
    /// INV-TL-2: Router accepts animation capability but does not own or create it.
    /// Animation capability is injected ONLY for on_hover_enter and on_hover_leave.
    ///
    /// Called by PointerMove routing.
    pub fn update_hover(
        &mut self,
        tree: &mut WidgetTree,
        new_hovered: Option<TreeWidgetId>,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
    ) {
        use crate::core::widget::EventContext;

        // Check scope boundary: if new_hovered is outside active scope, treat as None
        let new_hovered = new_hovered.filter(|owner| {
            !tree
                .find_node_by_widget_id(*owner)
                .and_then(|node| tree.node(node))
                .and_then(|node| crate::core::widget_availability::effective(node.widget.as_ref()))
                .is_some_and(|policy| !policy.enabled)
        });
        let effective_new = if let Some(widget_id) = new_hovered {
            if let Some(scope_root) = domain_state.focus_manager.active_scope() {
                // Check if widget is descendant of scope root
                if !tree.is_descendant_of_node(widget_id, scope_root) {
                    None // Outside scope, hover clears
                } else {
                    Some(widget_id)
                }
            } else {
                Some(widget_id) // No active scope, allow any widget
            }
        } else {
            None
        };

        // No change
        if domain_state.hovered_widget == effective_new {
            return;
        }

        let tree_ptr = tree as *mut WidgetTree;
        let clipboard_ptr = self.clipboard_ptr();

        // SPEC-HAL-021 Stage 4: Animation capability is consumed by the first callback that uses it.
        // We split the Option so on_hover_leave gets first chance, then on_hover_enter.
        // In practice, one or the other fires (not both in typical transitions).
        let mut animation = animation;

        // Fire on_hover_leave on previous
        // SPEC-HAL-021 Stage 4: Inject animation capability for on_hover_leave
        if let Some(old_id) = domain_state.hovered_widget {
            if let Some(node_id) = tree.find_node_by_widget_id(old_id) {
                // A hidden rail can be observed while the actual hover target
                // is a descendant. Retire that observation when leaving its subtree.
                let mut ancestor = tree.node(node_id).and_then(|node| node.parent);
                while let Some(id) = ancestor {
                    if effective_new.is_some_and(|owner| tree.is_descendant_of_node(owner, id)) {
                        break;
                    }
                    let Some(node) = tree.node_mut(id) else {
                        break;
                    };
                    ancestor = node.parent;
                    if node.widget.as_any_mut().downcast_mut::<ScrollArea>()
                        .is_some_and(ScrollArea::clear_scrollbar_rail_hover)
                    {
                        tree.mark_paint_dirty(id);
                    }
                }
                if let Some(node) = tree.node_mut(node_id) {
                    // Take animation for this callback (leaves None for on_hover_enter)
                    let anim_for_leave = animation.take();
                    let mut cx = EventContext::new(
                        old_id,
                        tree_ptr,
                        self.frame_time_ms,
                        clipboard_ptr,
                        None, // INV-TEXT-GEOM-PHASE: text_engine not needed for lifecycle callbacks
                        anim_for_leave,
                        false, // Capture not relevant for lifecycle callbacks
                        false, // SPEC-HAL-268: Not relevant for lifecycle callbacks
                        domain_state.keyboard_mode,
                    );
                    node.widget.on_hover_leave(&mut cx);
                    tree.mark_paint_dirty(node_id);
                    tracing::debug!(
                        "SPEC-HAL-021 Stage 4: on_hover_leave for TreeWidgetId({:?}) with animation={}",
                        old_id.0,
                        cx.has_animation()
                    );
                }
            }
        }

        // Fire on_hover_enter on new
        // SPEC-HAL-021 Stage 4: Inject animation capability for on_hover_enter
        // Note: If on_hover_leave consumed animation above, this gets None
        if let Some(new_id) = effective_new {
            if let Some(node_id) = tree.find_node_by_widget_id(new_id) {
                if let Some(node) = tree.node_mut(node_id) {
                    let mut cx = EventContext::new(
                        new_id,
                        tree_ptr,
                        self.frame_time_ms,
                        clipboard_ptr,
                        None, // INV-TEXT-GEOM-PHASE: text_engine not needed for lifecycle callbacks
                        animation, // Remaining animation (or None if consumed by on_hover_leave)
                        false, // Capture not relevant for lifecycle callbacks
                        false, // SPEC-HAL-268: Not relevant for lifecycle callbacks
                        domain_state.keyboard_mode,
                    );
                    node.widget.on_hover_enter(&mut cx);
                    tree.mark_paint_dirty(node_id);
                    tracing::debug!(
                        "SPEC-HAL-021 Stage 4: on_hover_enter for TreeWidgetId({:?}) with animation={}",
                        new_id.0,
                        cx.has_animation()
                    );
                }
            }
        }

        domain_state.hovered_widget = effective_new;
    }

    /// SPEC-HAL-038 Stage 7: Clear hover state.
    /// SPEC-HAL-021 Stage 4: No animation capability for scope/window-level clears.
    ///
    /// Fires on_hover_leave on the currently hovered widget (if any) and clears hover.
    /// Called when a scope is popped, the pointer leaves the window, or the
    /// window loses focus.
    ///
    /// Note: Animation capability is NOT injected here because this is not a
    /// direct hover transition from user pointer movement.
    pub fn clear_hover(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
    ) {
        self.update_hover(tree, None, None, domain_state);
    }

    /// SPEC-HAL-038 Stage 7: Get the currently hovered widget.
    pub fn hovered(&self, domain_state: &InteractionDomainState) -> Option<TreeWidgetId> {
        domain_state.hovered_widget
    }
}

// ============================================================================
// SPEC-HAL-326: Tree-Structured Focus Navigation helpers (free functions)
// ============================================================================

/// Walk up from `node_id` to find the nearest ancestor
/// whose widget returns Some from layout_axis().
/// Returns (ancestor_node_id, axis).
fn find_axis_container(tree: &WidgetTree, node_id: NodeId) -> Option<(NodeId, Axis)> {
    let mut current = tree.node(node_id)?.parent;
    while let Some(parent_id) = current {
        if let Some(parent_node) = tree.node(parent_id) {
            if let Some(axis) = parent_node.widget.layout_axis() {
                return Some((parent_id, axis));
            }
            current = parent_node.parent;
        } else {
            break;
        }
    }
    None
}

/// Find the next/prev focusable sibling (or descendant of a sibling)
/// in the container's child list.
///
/// SPEC-HAL-326 §3.5 / INV-FNAV-6 (v2): Entry into the chosen sibling
/// subtree uses spatial scoring for vertical traversal (Up/Down) and
/// DFS first/last for horizontal traversal (Left/Right). The sibling
/// root's declared `layout_axis()` is no longer consulted — entry is
/// determined by where in the subtree the focusable positions lie
/// relative to the focused widget's edge and sticky cursor.
fn navigate_sibling(
    tree: &WidgetTree,
    container_id: NodeId,
    current_node_id: NodeId,
    direction: Direction,
    focused_rect: RectF,
) -> Option<TreeWidgetId> {
    let children = collect_children(tree, container_id);

    let current_child_idx = children.iter().position(|&child_id| {
        child_id == current_node_id || tree.is_descendant(current_node_id, child_id)
    })?;

    let forward = matches!(direction, Direction::Right | Direction::Down);

    let pick = |child_id: NodeId| -> Option<TreeWidgetId> {
        match direction {
            Direction::Up | Direction::Down => nearest_focusable_in_subtree_by_spatial_proximity(
                tree,
                child_id,
                direction,
                focused_rect,
            ),
            Direction::Left | Direction::Right => {
                // Horizontal entry: leading-edge focusable. forward=true
                // (Right) → leftmost; forward=false (Left) → rightmost.
                first_focusable_in_subtree(tree, child_id, forward)
            }
        }
    };

    if forward {
        for idx in (current_child_idx + 1)..children.len() {
            if let Some(target) = pick(children[idx]) {
                return Some(target);
            }
        }
    } else {
        for idx in (0..current_child_idx).rev() {
            if let Some(target) = pick(children[idx]) {
                return Some(target);
            }
        }
    }

    None // At boundary — no more siblings
}

/// Exit the current container and navigate at the parent level.
fn navigate_exit_container(
    tree: &WidgetTree,
    container_id: NodeId,
    direction: Direction,
    focused_rect: RectF,
) -> Option<TreeWidgetId> {
    // Find the container's parent axis container
    let (parent_container_id, parent_axis) = find_axis_container(tree, container_id)?;

    let along_parent = match (parent_axis, direction) {
        (Axis::Horizontal, Direction::Left | Direction::Right) => true,
        (Axis::Vertical, Direction::Up | Direction::Down) => true,
        _ => false,
    };

    if along_parent {
        // Navigate siblings at parent level
        navigate_sibling(
            tree,
            parent_container_id,
            container_id,
            direction,
            focused_rect,
        )
        .or_else(|| {
            // Recurse: exit parent container too
            navigate_exit_container(tree, parent_container_id, direction, focused_rect)
        })
    } else {
        // Still cross-axis — keep exiting
        navigate_exit_container(tree, parent_container_id, direction, focused_rect)
    }
}

/// Find the first (or last) focusable widget in a subtree.
/// DFS traversal, returns first focusable found.
fn first_focusable_in_subtree(
    tree: &WidgetTree,
    root_id: NodeId,
    first: bool, // true = first (DFS forward), false = last (DFS reverse)
) -> Option<TreeWidgetId> {
    let node = tree.node(root_id)?;

    // Check if this node itself is focusable
    if node.widget.focusable() {
        return Some(node.id);
    }

    // Recurse into children
    let children = collect_children(tree, root_id);
    let iter: Box<dyn Iterator<Item = &NodeId>> = if first {
        Box::new(children.iter())
    } else {
        Box::new(children.iter().rev())
    };

    for &child_id in iter {
        if let Some(target) = first_focusable_in_subtree(tree, child_id, first) {
            return Some(target);
        }
    }

    None
}

/// SPEC-HAL-326 §3.5 / INV-FNAV-6 (v2): Find the focusable in a sibling
/// subtree whose position is the most natural target for cross-axis
/// vertical entry (Up/Down navigation).
///
/// Scoring:
///   - Primary (minimize): directional edge gap.
///     Up   → |focused_rect.top    - candidate.bottom_edge|
///     Down → |candidate.top_edge  - focused_rect.bottom|
///   - Tiebreaker (minimize): perpendicular distance from the focused
///     widget's left edge (sticky cursor ref_x = focused_rect.x).
///
/// Only valid for Direction::Up and Direction::Down. Horizontal entry
/// (Left/Right) uses `first_focusable_in_subtree` directly.
fn nearest_focusable_in_subtree_by_spatial_proximity(
    tree: &WidgetTree,
    root_id: NodeId,
    direction: Direction,
    focused_rect: RectF,
) -> Option<TreeWidgetId> {
    let mut candidates: Vec<(TreeWidgetId, RectF)> = Vec::new();
    collect_focusable_descendants_with_rect(tree, root_id, &mut candidates);

    let focused_top = focused_rect.y;
    let focused_bottom = focused_rect.y + focused_rect.height;
    let ref_x = focused_rect.x;

    candidates
        .into_iter()
        .min_by(|(_, ra), (_, rb)| {
            let (primary_a, primary_b) = match direction {
                Direction::Up => (
                    (focused_top - (ra.y + ra.height)).abs(),
                    (focused_top - (rb.y + rb.height)).abs(),
                ),
                Direction::Down => ((ra.y - focused_bottom).abs(), (rb.y - focused_bottom).abs()),
                // Horizontal entry uses first_focusable_in_subtree;
                // this function should not be called for Left/Right.
                Direction::Left | Direction::Right => return std::cmp::Ordering::Equal,
            };
            // Compare primary with a 1px epsilon so widgets at nearly
            // the same edge count as tied and fall through to the
            // perpendicular tiebreaker. Matches the convention used
            // by navigate_directional at :2860-2868.
            if (primary_a - primary_b).abs() > 1.0 {
                return primary_a
                    .partial_cmp(&primary_b)
                    .unwrap_or(std::cmp::Ordering::Equal);
            }
            let perp_a = (ra.x - ref_x).abs();
            let perp_b = (rb.x - ref_x).abs();
            perp_a
                .partial_cmp(&perp_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, _)| id)
}

/// Collect every focusable descendant of `node_id` with its absolute
/// paint rect. Used by spatial scoring in `nearest_focusable_in_subtree_by_spatial_proximity`.
fn collect_focusable_descendants_with_rect(
    tree: &WidgetTree,
    node_id: NodeId,
    out: &mut Vec<(TreeWidgetId, RectF)>,
) {
    let Some(node) = tree.node(node_id) else {
        return;
    };

    if node.widget.focusable() {
        let rect = tree.get_absolute_paint_rect(node_id).unwrap_or_default();
        out.push((node.id, rect));
    }

    let mut child = node.first_child;
    while let Some(child_id) = child {
        collect_focusable_descendants_with_rect(tree, child_id, out);
        if let Some(c) = tree.node(child_id) {
            child = c.next_sibling;
        } else {
            break;
        }
    }
}

/// Collect child NodeIds from linked list into a Vec.
fn collect_children(tree: &WidgetTree, parent_id: NodeId) -> Vec<NodeId> {
    let mut children = Vec::new();
    let Some(parent) = tree.node(parent_id) else {
        return children;
    };
    let mut current = parent.first_child;
    while let Some(child_id) = current {
        children.push(child_id);
        if let Some(child) = tree.node(child_id) {
            current = child.next_sibling;
        } else {
            break;
        }
    }
    children
}

// ============================================================================
// SPEC-HAL-033: Fused Input Event Routing
// ============================================================================

impl EventRouter {
    /// SPEC-HAL-241: Walk up from a hit widget to find the first ancestor that accepts a drop.
    fn find_eligible_drop_target(
        tree: &WidgetTree,
        start: TreeWidgetId,
        payload_type: std::any::TypeId,
    ) -> Option<TreeWidgetId> {
        let mut current_node = tree.find_node_by_widget_id(start);
        while let Some(node_id) = current_node {
            if tree.query_accepts_drop(node_id, payload_type) {
                return tree.widget_id_for_node(node_id);
            }
            current_node = tree.parent(node_id);
        }
        None
    }

    /// Route a fused input event through the widget tree
    /// SPEC-HAL-033: Converts FusedInputEvent to existing Event types for widget dispatch
    /// SPEC-HAL-038 Stage 2: Added optional root parameter for Tab navigation
    /// SPEC-HAL-021 Stage 5: Animation capability injection for target widget input.
    #[allow(deprecated)] // Uses deprecated route_pointer_event/route_key_event internally during transition
    /// SPEC-HAL-021 Stage 3: Route input event with optional animation capability.
    ///
    /// # Arguments
    /// * `tree` - The widget tree to route through
    /// * `event` - The fused input event to route
    /// * `root` - Optional root node for focus scope (Tab navigation)
    /// * `text_engine` - Text engine for synchronous layout cache rebuild (INV-TEXT-GEOM-PHASE)
    /// * `animation` - Optional animation capability from TransitionLayer
    /// * `domain_state` - Interaction domain state (SPEC-HAL-151)
    ///
    /// SPEC-HAL-151: All interaction state is passed explicitly via domain_state.
    /// INV-TL-2: Router accepts animation capability but does not own or create it.
    /// Animation capability is forwarded to target widget interactions and hover lifecycle.
    pub fn route_input_event(
        &mut self,
        tree: &mut WidgetTree,
        event: FusedInputEvent,
        root: Option<crate::core::widget_tree::NodeId>,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
        _binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
    ) {
        self.route_input_event_impl(
            tree,
            event,
            root,
            text_engine,
            animation,
            domain_state,
            _binding_registry,
            None,
            None,
            None,
        );
    }

    pub fn route_session_input_event(
        &mut self,
        tree: &mut WidgetTree,
        event: crate::core::event::SessionQualifiedInput,
        root: Option<crate::core::widget_tree::NodeId>,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
        binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
    ) {
        self.route_session_input_event_inner(
            tree,
            event,
            root,
            text_engine,
            animation,
            domain_state,
            binding_registry,
            None,
        );
    }

    pub(crate) fn route_session_input_event_for_app(
        &mut self,
        tree: &mut WidgetTree,
        event: crate::core::event::SessionQualifiedInput,
        root: NodeId,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
        binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
        target: &CapturedAppKeyTarget,
        facts: Option<RelativePointerWindowFacts>,
    ) -> CapturedAppKeyOutcome {
        if facts.is_some_and(|facts| {
            facts.window_id != event.event().window_id()
                || facts.focused_window_id != Some(facts.window_id)
                || facts.interaction_domain_id != domain_state.focus_manager.interaction_domain_id()
        }) {
            return CapturedAppKeyOutcome::NotEligible;
        }
        let previous = self.relative_window_facts.replace(facts);
        let outcome = self.route_session_input_event_inner(
            tree,
            event,
            Some(root),
            text_engine,
            animation,
            domain_state,
            binding_registry,
            Some(target),
        );
        self.relative_window_facts.set(previous);
        outcome
    }

    fn route_session_input_event_inner(
        &mut self,
        tree: &mut WidgetTree,
        event: crate::core::event::SessionQualifiedInput,
        root: Option<NodeId>,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
        binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
        captured_target: Option<&CapturedAppKeyTarget>,
    ) -> CapturedAppKeyOutcome {
        #[cfg(any(target_arch = "wasm32", test))]
        {
            self.touch_keyboard_activation = None;
        }
        if !self.prepare_session_input(tree, &event, domain_state) {
            return CapturedAppKeyOutcome::NotEligible;
        }
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        if matches!(event.event(), FusedInputEvent::PointerDown { .. } | FusedInputEvent::KeyDown { .. }
                | FusedInputEvent::FocusChanged { focused: false, .. })
            || matches!(event.event(), FusedInputEvent::TouchDown { .. })
                && event.touch_motion().is_some_and(|motion| motion.first_contact)
        {
            self.retire_host_choice_window(event.event().window_id());
        }
        if let (
            Some(press),
            FusedInputEvent::PointerDown {
                click_count, pos, ..
            }
            | FusedInputEvent::TouchDown {
                click_count, pos, ..
            },
        ) = (&event.press, event.event())
        {
            let captured = self.validate_capture_target(tree, press.pointer, domain_state);
            let protected_hit = tree.hit_test(pos.x, pos.y).is_some_and(|hit| {
                self.build_event_path(tree, hit.widget_id)
                    .iter()
                    .any(|owner| {
                        tree.find_node_by_widget_id(*owner)
                            .and_then(|id| tree.node(id))
                            .is_some_and(|node| {
                                node.widget.absolute_drag_geometry_dependency().is_some()
                            })
                    })
            });
            // A reservation cannot replace a live captured press. Deliver the
            // additional button event through the existing captured-target route.
            if captured.is_none() && (*click_count == 1 || protected_hit) {
                if domain_state
                    .gesture_registry
                    .session_id(press.pointer)
                    .is_some()
                {
                    self.cancel_pointer_session_with_tree(
                        tree,
                        press.pointer,
                        CancellationReason::SystemCancel,
                        domain_state,
                    );
                }
                if !domain_state
                    .gesture_registry
                    .install_physical_press(press.clone())
                {
                    return CapturedAppKeyOutcome::NotEligible;
                }
            }
        }
        let previous_owners: Vec<_> = domain_state
            .gesture_registry
            .geometry_watches()
            .into_iter()
            .map(|(_, _, watch)| watch.owner)
            .collect();
        let mut outcome = CapturedAppKeyOutcome::NotEligible;
        self.route_input_event_impl(
            tree,
            event.event().clone(),
            root,
            text_engine,
            animation,
            domain_state,
            binding_registry,
            event.press.clone(),
            event.touch_motion(),
            captured_target.map(|target| (target, &mut outcome)),
        );
        let current_owners: Vec<_> = domain_state
            .gesture_registry
            .geometry_watches()
            .into_iter()
            .map(|(_, _, watch)| watch.owner)
            .collect();
        for owner in previous_owners {
            if !current_owners.contains(&owner) {
                tree.geometry_protected_owners.remove(&owner);
            }
        }
        event.finish_delivery(false);
        if let Some(press) = &event.press {
            if matches!(event.event(), FusedInputEvent::PointerDown { .. })
                && !press.protected.get()
            {
                press.eligible.set(false);
            }
        }
        outcome
    }

    pub(crate) fn route_session_input_event_with_relative_facts(
        &mut self,
        tree: &mut WidgetTree,
        event: crate::core::event::SessionQualifiedInput,
        root: Option<crate::core::widget_tree::NodeId>,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
        binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
        facts: RelativePointerWindowFacts,
    ) {
        self.relative_window_facts.set(Some(facts));
        self.route_session_input_event(
            tree,
            event,
            root,
            text_engine,
            animation,
            domain_state,
            binding_registry,
        );
        self.relative_window_facts.set(None);
    }

    pub(crate) fn prepare_session_input(
        &mut self,
        tree: &mut WidgetTree,
        event: &crate::core::event::SessionQualifiedInput,
        domain_state: &mut InteractionDomainState,
    ) -> bool {
        if event.is_replayed() {
            return false;
        }
        let Some(press) = &event.press else {
            Self::assert_absolute_input_lineage(event.event(), false, domain_state);
            self.retire_invalid_absolute_geometry(tree, domain_state);
            return true;
        };
        if !domain_state.gesture_registry.owns_press(press)
            || !tree.owns_geometry_cutoff(&press.cutoff)
            || press.domain != domain_state.focus_manager.interaction_domain_id
            || press.window != event.event().window_id()
        {
            return false;
        }
        if press.protected.get() {
            if press.state.get() == crate::core::event::PhysicalPressState::Completed
                || domain_state.gesture_registry.session_id(press.pointer) != Some(press.session)
            {
                return false;
            }
            self.retire_invalid_absolute_geometry(tree, domain_state);
            return press.state.get() != crate::core::event::PhysicalPressState::Completed;
        }
        if press.pointer_type == PointerType::Touch {
            self.retire_invalid_absolute_geometry(tree, domain_state);
            if matches!(event.event(), FusedInputEvent::TouchDown { .. }) {
                return domain_state.gesture_registry.can_install_press(press);
            }
            return match press.state.get() {
                PhysicalPressState::Routed => domain_state
                    .gesture_registry
                    .physical_press(press.pointer)
                    .is_some_and(|live| std::rc::Rc::ptr_eq(press, &live)),
                // A modal can consume Down before registration. AppRoot still
                // owns that contact's dismissal shield through its terminal.
                PhysicalPressState::Reserved => {
                    !press.eligible.get()
                        && !domain_state.gesture_registry.has_session(press.pointer)
                }
                PhysicalPressState::Completed => false,
            };
        }
        if matches!(
            event.event(),
            FusedInputEvent::PointerDown { .. } | FusedInputEvent::TouchDown { .. }
        ) && !domain_state.gesture_registry.can_install_press(press)
        {
            return false;
        }
        self.retire_invalid_absolute_geometry(tree, domain_state);
        true
    }

    pub(crate) fn retire_invalid_absolute_geometry(
        &mut self,
        tree: &mut WidgetTree,
        domain_state: &mut InteractionDomainState,
    ) -> Vec<TreeWidgetId> {
        let mut changed = Vec::new();
        for (pointer, press, watch) in domain_state.gesture_registry.geometry_watches() {
            if domain_state.gesture_registry.session_id(pointer) != Some(press.session)
                || !domain_state.gesture_registry.owns_press(&press)
            {
                continue;
            }
            if !tree.absolute_drag_geometry_is_current(&watch) {
                press
                    .state
                    .set(crate::core::event::PhysicalPressState::Completed);
                if let Some(owner) = domain_state
                    .pointer_capture_manager
                    .current_capture(pointer)
                {
                    if tree.owns_geometry_cutoff(&press.cutoff) && tree.find_by_id(owner).is_some()
                    {
                        changed.push(owner);
                    }
                }
                self.cancel_pointer_session_with_tree(
                    tree,
                    pointer,
                    CancellationReason::SystemCancel,
                    domain_state,
                );
                tree.geometry_protected_owners.remove(&watch.owner);
            }
        }
        changed.sort_unstable_by_key(|owner| owner.0);
        changed.dedup();
        changed
    }

    pub(crate) fn take_geometry_capture_changes(&mut self, tree: &WidgetTree) -> Vec<TreeWidgetId> {
        let mut changed: Vec<_> = std::mem::take(&mut self.geometry_capture_changes)
            .into_iter()
            .filter_map(|(cutoff, owner)| {
                (tree.owns_geometry_cutoff(&cutoff) && tree.find_by_id(owner).is_some())
                    .then_some(owner)
            })
            .collect();
        changed.sort_unstable_by_key(|owner| owner.0);
        changed.dedup();
        changed
    }

    pub(crate) fn geometry_pending_focus_reveal(&self) -> Option<TreeWidgetId> {
        self.pending_focus_reveal
    }

    fn assert_absolute_input_lineage(
        event: &FusedInputEvent,
        qualified: bool,
        state: &InteractionDomainState,
    ) {
        assert!(
            qualified
                || !matches!(
                    event,
                    FusedInputEvent::PointerDown { .. }
                        | FusedInputEvent::TouchDown { .. }
                        | FusedInputEvent::TouchMove { .. }
                        | FusedInputEvent::TouchUp { .. }
                        | FusedInputEvent::TouchCancel { .. }
                        | FusedInputEvent::PointerMove { .. }
                        | FusedInputEvent::PointerUp { .. }
                        | FusedInputEvent::DragStart { .. }
                        | FusedInputEvent::DragUpdate { .. }
                        | FusedInputEvent::DragEnd { .. }
                )
                || state.gesture_registry.geometry_watches().is_empty(),
            "INV-ABSOLUTE-GEOMETRY-QUALIFIED-HOST: protected pointer input requires its original seal"
        );
    }

    fn route_input_event_impl(
        &mut self,
        tree: &mut WidgetTree,
        event: FusedInputEvent,
        root: Option<crate::core::widget_tree::NodeId>,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        mut animation: Option<crate::core::animation::AnimationContext<'_>>,
        domain_state: &mut InteractionDomainState,
        _binding_registry: &mut crate::runtime::binding_registry::BindingRegistry,
        qualified_press: Option<std::rc::Rc<crate::core::event::PhysicalGesturePress>>,
        touch_motion: Option<QualifiedTouchMotion>,
        mut captured_app: Option<(&CapturedAppKeyTarget, &mut CapturedAppKeyOutcome)>,
    ) {
        crate::core::widget_availability::observe_keyboard_boundary(tree, domain_state, &event);
        let (input_pointer_id, input_pointer_type) = match &event {
            FusedInputEvent::TouchDown { pointer_id, .. }
            | FusedInputEvent::TouchMove { pointer_id, .. }
            | FusedInputEvent::TouchUp { pointer_id, .. }
            | FusedInputEvent::TouchCancel { pointer_id, .. } => (*pointer_id, PointerType::Touch),
            _ => (PointerId::MOUSE, PointerType::Mouse),
        };
        Self::assert_absolute_input_lineage(&event, qualified_press.is_some(), domain_state);
        self.retire_stale_touch_scroll(tree, domain_state);
        if touch_motion.is_some() && matches!(&event, FusedInputEvent::TouchDown { .. }) {
            domain_state.interrupt_kinetic_scroll();
        }
        if input_pointer_type == PointerType::Touch
            && qualified_press
                .as_ref()
                .is_some_and(|press| press.state.get() == PhysicalPressState::Completed)
        {
            return;
        }
        if input_pointer_type == PointerType::Touch
            && qualified_press.is_none()
            && domain_state
                .gesture_registry
                .physical_press(input_pointer_id)
                .is_some()
        {
            return;
        }
        let claimed_relative_event = Self::claims_relative_event(&event, domain_state);
        if let Some(binding) = domain_state
            .relative_pointer_authority
            .as_ref()
            .and_then(|authority| authority.active())
            .map(|activation| activation.binding())
        {
            let owner_mounted = tree
                .find_node_by_widget_id(binding.capture_owner())
                .is_some();
            let capture_valid = domain_state
                .pointer_capture_manager
                .current_capture_with_mode(binding.pointer_id())
                == Some((binding.capture_owner(), CaptureMode::Hard))
                && domain_state.pointer_capture.captured_widget == Some(binding.capture_owner())
                && domain_state.pointer_capture.button == Some(binding.pointer_button());
            let session_valid = domain_state
                .gesture_registry
                .committed_gesture(binding.pointer_id())
                == Some(binding.winning_candidate_id())
                && domain_state
                    .gesture_registry
                    .session_id(binding.pointer_id())
                    == Some(binding.gesture_session_id());
            if !owner_mounted || !capture_valid || !session_valid {
                let reason = if owner_mounted {
                    RelativePointerCancellationReason::CaptureLost
                } else {
                    RelativePointerCancellationReason::OwnerUnmounted
                };
                self.cancel_active_relative_with_tree(tree, domain_state, reason, None);
                if claimed_relative_event {
                    return;
                }
            }
            if event.window_id() != binding.window_id()
                && matches!(
                    event,
                    FusedInputEvent::PointerUp { .. } | FusedInputEvent::DragEnd { .. }
                )
            {
                return;
            }
        }
        self.validate_capture_target(tree, input_pointer_id, domain_state);
        if captured_app.as_ref().is_some_and(|(target, _)| {
            root.is_none_or(|root| !target.is_current(tree, domain_state, &event, root))
        }) {
            return;
        }

        // INV-TEXT-GEOM-PHASE: Store raw pointer for threading through dispatch calls.
        // We use a raw pointer here because Rust's borrow checker cannot track that we only
        // use one mutable borrow at a time across the various match arms and sequential calls.
        // The pointer is valid for the entire duration of this function.
        let text_engine_ptr: Option<*mut dyn crate::core::text_engine::TextEngine> =
            Some(text_engine as *mut dyn crate::core::text_engine::TextEngine);

        match event {
            FusedInputEvent::RelativePointerMotion {
                lease_id,
                window_id,
                pointer_id,
                delta,
                ..
            } => {
                let Some(facts) = self.relative_window_facts.get() else {
                    return;
                };
                let Some(binding) =
                    domain_state
                        .relative_pointer_authority
                        .as_ref()
                        .and_then(|authority| {
                            authority.binding_for_motion(
                                lease_id,
                                window_id,
                                facts.interaction_domain_id,
                                pointer_id,
                            )
                        })
                else {
                    return;
                };
                if domain_state
                    .pointer_capture_manager
                    .current_capture_with_mode(pointer_id)
                    != Some((binding.capture_owner(), CaptureMode::Hard))
                    || domain_state.gesture_registry.committed_gesture(pointer_id)
                        != Some(binding.winning_candidate_id())
                    || domain_state.gesture_registry.session_id(pointer_id)
                        != Some(binding.gesture_session_id())
                {
                    self.cancel_active_relative_with_tree(
                        tree,
                        domain_state,
                        RelativePointerCancellationReason::CaptureLost,
                        None,
                    );
                    return;
                }
                if tree
                    .find_node_by_widget_id(binding.capture_owner())
                    .is_none()
                {
                    self.cancel_active_relative_with_tree(
                        tree,
                        domain_state,
                        RelativePointerCancellationReason::OwnerUnmounted,
                        None,
                    );
                    return;
                }
                let event_kind = "RelativePointerMotion";
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.dispatch_to_widget(
                    tree,
                    binding.capture_owner(),
                    &Event::RelativePointerMotion(RelativePointerMotionEvent {
                        lease_id,
                        pointer_id,
                        delta,
                    }),
                    EventPhase::Target,
                    text_engine_ptr,
                    domain_state,
                    None,
                );
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }
            // SPEC-HAL-021 Stage 4: PointerMove with animation injection for hover
            // SPEC-HAL-079 Phase-2: Full pipeline enforcement
            FusedInputEvent::PointerMove { pos, .. } | FusedInputEvent::TouchMove { pos, .. } => {
                let pointer_id = input_pointer_id;
                let event_kind = "PointerMove";

                // SPEC-HAL-038: Do NOT release scroll_capture_owner on PointerMove.
                // Wheel scrolling happens while pointer moves (trackpad especially).
                // Only PointerDown/PointerUp release scroll capture.

                // ===== PHASE 1: PreHitTest =====
                // Placeholder today - input fusion already ran externally
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                // Check capture state (not a hit-test, just a lookup)
                let captured_id = domain_state
                    .pointer_capture_manager
                    .current_capture(input_pointer_id);
                // Hit-test only if not captured
                let hit_result = if captured_id.is_none() {
                    tree.hit_test_for_routing(pos.x, pos.y)
                } else {
                    None
                };
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                if touch_motion.is_some_and(|motion| {
                    self.route_active_touch_scroll(tree, motion, root, false, domain_state)
                        || self.route_active_touch_drag(tree, motion, false, domain_state)
                }) {
                    self.complete_remaining_phases(
                        InputPipelinePhase::Routing,
                        pointer_id,
                        event_kind,
                    );
                    return;
                }

                // SPEC-HAL-079: Check soft capture exit during Routing phase.
                // CRITICAL: Queries CURRENT layout rect from tree (not cached).
                // INV-079-NO-CAPTURE-OUTSIDE-ROUTING: Capture mutations in Routing only.
                let soft_capture_owner = domain_state
                    .pointer_capture_manager
                    .current_capture(input_pointer_id);
                if domain_state
                    .pointer_capture_manager
                    .check_soft_capture_exit(input_pointer_id, (pos.x, pos.y), tree)
                {
                    if let Some(widget_id) = soft_capture_owner {
                        self.restore_hidden_cursor_for_widget(widget_id);
                    }
                    tracing::debug!("SPEC-HAL-079: Soft capture auto-released on pointer exit");
                }

                // Re-check capture after potential soft capture release
                let captured_id = domain_state
                    .pointer_capture_manager
                    .current_capture(input_pointer_id);

                if let Some(captured_id) = captured_id {
                    // Captured: route move to captured widget, no hover tracking
                    // SPEC-HAL-021 Stage 4: No animation for captured move (no hover transition)
                    if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                        let mut pointer_event = PointerEvent {
                            id: input_pointer_id,
                            global_pos: (pos.x, pos.y),
                            local_pos: (pos.x, pos.y),
                            kind: PointerEventKind::Move,
                            button: None,
                            modifiers: self.current_modifiers,
                            pointer_type: input_pointer_type,
                        };
                        // Compute local coords relative to captured widget
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            pointer_event.local_pos = (
                                pos.x + scroll_offset.0 - abs_rect.x,
                                pos.y + scroll_offset.1 - abs_rect.y,
                            );
                        }
                        self.route_to_widget(
                            tree,
                            captured_id,
                            &Event::Pointer(pointer_event),
                            text_engine_ptr,
                            domain_state,
                            None,
                        );
                    }
                } else if let Some((hit_result, route_origin)) = hit_result {
                    // Not captured: hover tracking with animation injection
                    // SPEC-HAL-021 Stage 4: Inject animation for hover enter/leave
                    let target_id = hit_result.widget_id;

                    // Update hover with animation capability
                    if input_pointer_type != PointerType::Touch {
                        self.update_hover(tree, Some(target_id), animation, domain_state);
                    }

                    // Dispatch pointer move event (no animation for move itself)
                    let pointer_event = PointerEvent {
                        id: input_pointer_id,
                        global_pos: (pos.x, pos.y),
                        local_pos: (pos.x, pos.y),
                        kind: PointerEventKind::Move,
                        button: None,
                        modifiers: self.current_modifiers,
                        pointer_type: input_pointer_type,
                    };
                    let event_path = self.build_event_path(tree, target_id);
                    self.dispatch_pointer_event(
                        tree,
                        &event_path,
                        pointer_event,
                        route_origin,
                        text_engine_ptr,
                        domain_state,
                        None,
                    );
                } else {
                    // Pointer moved outside all widgets, clear hover
                    // SPEC-HAL-021 Stage 4: Inject animation for hover leave
                    if input_pointer_type != PointerType::Touch {
                        self.update_hover(tree, None, animation, domain_state);
                    }
                }

                if let Some(motion) = touch_motion {
                    Self::request_touch_drag(tree, motion, domain_state);
                }
                let scroll = touch_motion.and_then(|motion| {
                    Self::request_touch_scroll(tree, motion, root, domain_state)
                });
                self.resolve_gesture_requests(tree, pointer_id, domain_state, scroll);
                if let Some(motion) = touch_motion {
                    self.route_active_touch_drag(tree, motion, false, domain_state);
                }
                if let Some(motion) = touch_motion
                    && !(motion.position.distance(motion.origin)
                        < crate::core::input_fusion::DRAG_DISTANCE_THRESHOLD)
                {
                    domain_state.gesture_registry.take_touch_tap(motion.press);
                }
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            // SPEC-HAL-079 Phase-2: Full pipeline enforcement for PointerDown
            FusedInputEvent::PointerDown {
                pos,
                button,
                click_count,
                ..
            }
            | FusedInputEvent::TouchDown {
                pos,
                button,
                click_count,
                ..
            } => {
                let pointer_id = input_pointer_id;
                let event_kind = "PointerDown";

                // Step 1 — Instrumentation: Log click_count on every PointerDown.
                #[cfg(debug_assertions)]
                tracing::debug!(
                    "[CORE] PointerDown click_count={} button={:?} pos=({:.1}, {:.1})",
                    click_count,
                    button,
                    pos.x,
                    pos.y
                );

                // SPEC-HAL-038: Release scroll_capture_owner on PointerDown.
                // Click ends wheel scroll authority.
                if domain_state.touch_scroll_source().is_none()
                    && let Some(owner) = domain_state.scroll_capture_owner
                    && (input_pointer_type != PointerType::Touch
                        || touch_motion.is_some_and(|motion| {
                            motion.first_contact
                                && !domain_state
                                    .pointer_capture_manager
                                    .active_pointers()
                                    .into_iter()
                                    .any(|pointer| {
                                        domain_state.pointer_capture_manager.is_captured_by(pointer, owner)
                                    })
                        }))
                {
                    tracing::debug!("SPEC-HAL-038: Scroll capture released on PointerDown");
                    domain_state.clear_wheel_scroll_capture();
                }

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                // Placeholder - input fusion already ran externally
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // Check capture state (not a mutation, just lookup)
                let is_captured = domain_state
                    .pointer_capture_manager
                    .is_captured(input_pointer_id);

                // Hit-test for routing
                let hit_result = tree.hit_test_for_routing(pos.x, pos.y);

                // Hit-test for focus assignment (primary button, not captured)
                let popup_inert_press = input_pointer_type != PointerType::Touch
                    && button == PointerButton::Primary
                    && !is_captured
                    && hit_result.as_ref().is_some_and(|(hit, origin)| {
                        *origin == PointerRouteOrigin::Overlay
                            && Self::combo_popup_inert_press_at(tree, hit, pos)
                    });
                let focus_hit = if input_pointer_type != PointerType::Touch
                    && button == PointerButton::Primary
                    && !is_captured
                    && !popup_inert_press
                {
                    hit_result.as_ref().map(|(hit, _)| hit)
                } else {
                    None
                };

                let overlay_hit = hit_result
                    .as_ref()
                    .is_some_and(|(_, origin)| *origin == PointerRouteOrigin::Overlay);

                // Hit-test for projected routing (virtualized items)
                let projected_hit = if input_pointer_type != PointerType::Touch
                    && button == PointerButton::Primary
                    && !overlay_hit
                {
                    tree.hit_test_projected(pos.x, pos.y)
                } else {
                    None
                };

                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                let dismissal = touch_motion.and_then(|motion| {
                    let target = hit_result.as_ref().map(|(hit, _)| hit.widget_id);
                    let tap = Self::touch_dismissal_at(tree, pos, target, root)?;
                    domain_state.gesture_registry.register_touch_tap(motion.press, tap);
                    Some(())
                }).is_some();

                if input_pointer_type != PointerType::Touch
                    && button == PointerButton::Primary
                    && !is_captured
                {
                    let target = hit_result.as_ref().map(|(hit, _)| hit.widget_id);
                    tree.dismiss_nonmodal_popup_for_press(pos, target);
                    if !popup_inert_press {
                        self.clear_focused_read_only_selection_on_outside_down(
                            tree,
                            target,
                            domain_state,
                        );
                    }
                }

                // Focus assignment from HitTest results
                // SPEC-HAL-038 Stage 7: Only assign focus on Primary button, not during capture
                // INV-FOCUS-ON-CLICK-ANCESTOR-1: Walk ancestors if target not focusable
                if let Some(hr) = focus_hit {
                    let target_id = hr.widget_id;
                    let focus_target = if self.is_widget_focusable(tree, target_id) {
                        Some(target_id)
                    } else {
                        self.find_focusable_ancestor(tree, target_id)
                    };
                    if let Some(widget_id) = focus_target {
                        self.request_focus_with_text_engine(
                            tree,
                            widget_id,
                            domain_state,
                            Some(text_engine),
                        );
                        if domain_state.focus_manager.is_focused(widget_id) {
                            self.continuous_frames_requested |=
                                domain_state.focus_manager.clear_keyboard_focus_cue();
                        }
                    }
                }

                // ================================================================
                // SPEC-HAL-157: Capture-First Routing Authority
                // ================================================================
                let capture_consumed = if !dismissal && let Some((hr, route_origin)) = hit_result.as_ref() {
                    let target_id = hr.widget_id;
                    let event_path = self.build_event_path(tree, target_id);

                    let capture_event = PointerEvent {
                        id: input_pointer_id,
                        global_pos: (pos.x, pos.y),
                        local_pos: (pos.x, pos.y),
                        kind: PointerEventKind::Down,
                        button: Some(button),
                        modifiers: self.current_modifiers,
                        pointer_type: input_pointer_type,
                    };

                    // SPEC-HAL-157 (INV-157-1): Run capture phase first
                    self.dispatch_capture_phase_only(
                        tree,
                        &event_path,
                        capture_event,
                        *route_origin,
                        text_engine_ptr,
                        domain_state,
                    )
                } else {
                    false
                };

                if capture_consumed && click_count < 2 {
                    // SPEC-HAL-157 (INV-157-2): Single click fully handled by capture phase
                    self.resolve_gesture_commits_for_pointer(tree, pointer_id, domain_state);
                    // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                    self.complete_remaining_phases(
                        InputPipelinePhase::Routing,
                        pointer_id,
                        event_kind,
                    );
                    return;
                }

                // SPEC-HAL-131E: Try projected routing (skip if capture handled Down)
                if !capture_consumed {
                    if let Some(hit) = projected_hit {
                        let action = if click_count >= 2 {
                            crate::core::input_projection::ProjectedAction::Activate
                        } else {
                            crate::core::input_projection::ProjectedAction::select_with_modifiers(
                                self.current_modifiers,
                            )
                        };
                        self.last_projected_hit = Some(hit.clone());
                        if self.dispatch_projected_input(tree, &hit, action, domain_state) {
                            // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                            self.complete_remaining_phases(
                                InputPipelinePhase::Routing,
                                pointer_id,
                                event_kind,
                            );
                            return;
                        }
                    }
                }

                // Synthesize event kind from click_count
                let kind = if click_count >= 3 {
                    PointerEventKind::TripleClick
                } else if click_count == 2 {
                    PointerEventKind::DoubleClick
                } else {
                    PointerEventKind::Down
                };

                #[cfg(debug_assertions)]
                tracing::debug!(
                    "[CORE] Synthesized kind={:?} from click_count={}",
                    kind,
                    click_count
                );

                let pointer_event = PointerEvent {
                    id: input_pointer_id,
                    global_pos: (pos.x, pos.y),
                    local_pos: (pos.x, pos.y),
                    kind,
                    button: Some(button),
                    modifiers: self.current_modifiers,
                    pointer_type: input_pointer_type,
                };

                // SPEC-HAL-128 Stage 2.1: Begin gesture session on physical PointerDown
                if qualified_press.is_none() && matches!(pointer_event.kind, PointerEventKind::Down)
                {
                    domain_state.gesture_registry.begin_session_with_down_facts(
                        input_pointer_id,
                        pointer_event.pointer_type,
                        Some((button, pos)),
                    );
                }

                // Re-check capture after capture phase dispatch
                if let Some(captured_id) = domain_state
                    .pointer_capture_manager
                    .current_capture(input_pointer_id)
                {
                    // SPEC-HAL-038 Stage 8: During capture, route Down to captured widget
                    let mut local_event = pointer_event.clone();
                    if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            local_event.local_pos = (
                                pointer_event.global_pos.0 + scroll_offset.0 - abs_rect.x,
                                pointer_event.global_pos.1 + scroll_offset.1 - abs_rect.y,
                            );
                        }
                    }
                    self.route_to_widget(
                        tree,
                        captured_id,
                        &Event::Pointer(local_event),
                        text_engine_ptr,
                        domain_state,
                        animation,
                    );
                } else if let Some((hr, route_origin)) = hit_result.as_ref() {
                    // SPEC-HAL-157: No capture, run target+bubble (capture already ran)
                    let target_id = hr.widget_id;

                    // SPEC-HAL-038 Stage 7: PointerDown implicitly sets hover
                    if input_pointer_type != PointerType::Touch {
                        self.update_hover(tree, Some(target_id), None, domain_state);
                    }

                    let event_path = self.build_event_path(tree, target_id);

                    if touch_motion.is_some_and(|motion| motion.first_contact) {
                        let scroll_path = if *route_origin == PointerRouteOrigin::Overlay {
                            // A ScrollArea's own rail is in scope. An ancestor
                            // beneath another overlay is not its input owner.
                            std::slice::from_ref(&target_id)
                        } else {
                            event_path.as_slice()
                        };
                        for &owner in scroll_path {
                            if root.is_none_or(|root| tree.is_descendant_of_node(owner, root))
                                && tree
                                    .find_node_by_widget_id(owner)
                                    .and_then(|id| tree.node(id))
                                    .is_some_and(|node| {
                                        node.widget.accepts_touch_scroll_gesture()
                                            && (!node.widget.touch_scroll_requires_overlay()
                                                || *route_origin == PointerRouteOrigin::Overlay)
                                            && node
                                                .widget
                                                .gesture_surface()
                                                .contains(&GestureKind::Scroll)
                                    })
                                && let Some(mount) = tree.concrete_mount(owner)
                            {
                                domain_state
                                    .gesture_registry
                                    .register_touch_scroll_candidate(pointer_id, mount);
                            }
                        }
                    }

                    if !dismissal && let Some(motion) = touch_motion
                        && let Some(tap) = Self::touch_tap_at(tree, hr, *route_origin, pos, root)
                    {
                        if let Some(axis) = tree.find_by_id(target_id)
                            .and_then(|id| tree.node(id))
                            .filter(|node| node.widget.gesture_surface().contains(&GestureKind::Drag))
                            .and_then(|node| node.widget.touch_drag_axis(tap.target))
                        {
                            domain_state.gesture_registry.register_touch_drag(motion.press, tap.mount.clone(), axis);
                        }
                        domain_state.gesture_registry.register_touch_tap(motion.press, tap);
                    }

                    // SPEC-HAL-128 Stage 8: Register gesture candidates
                    if matches!(pointer_event.kind, PointerEventKind::Down)
                        || qualified_press.as_ref().is_some_and(|press| {
                            domain_state.gesture_registry.session_id(pointer_id)
                                == Some(press.session)
                        })
                    {
                        self.register_candidates_from_declarations(
                            tree,
                            &event_path,
                            input_pointer_id,
                            domain_state,
                            !matches!(pointer_event.kind, PointerEventKind::Down),
                        );
                    }

                    self.retire_invalid_absolute_geometry(tree, domain_state);
                    if qualified_press.as_ref().is_some_and(|press| {
                        press.protected.get()
                            && press.state.get()
                                == crate::core::event::PhysicalPressState::Completed
                    }) {
                        self.complete_remaining_phases(
                            InputPipelinePhase::Routing,
                            pointer_id,
                            event_kind,
                        );
                        return;
                    }

                    // SPEC-HAL-157 (INV-157-3): Run target+bubble only
                    if !dismissal {
                        self.dispatch_target_and_bubble(
                            tree, &event_path, pointer_event.clone(), *route_origin,
                            text_engine_ptr, domain_state, animation,
                        );
                    }
                }

                self.resolve_gesture_commits_for_pointer(tree, pointer_id, domain_state);
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            // SPEC-HAL-079 Phase-2: Full pipeline enforcement for PointerUp
            FusedInputEvent::PointerUp { pos, button, .. }
            | FusedInputEvent::TouchUp { pos, button, .. } => {
                let pointer_id = input_pointer_id;
                let event_kind = "PointerUp";
                let relative_binding = domain_state
                    .relative_pointer_authority
                    .as_ref()
                    .and_then(|authority| authority.active())
                    .map(|activation| activation.binding())
                    .filter(|binding| binding.pointer_id() == pointer_id);
                let completes_relative =
                    relative_binding.is_some_and(|binding| binding.pointer_button() == button);

                // SPEC-HAL-038: Release scroll_capture_owner on PointerUp.
                if input_pointer_type != PointerType::Touch
                    && domain_state.touch_scroll_source().is_none()
                    && domain_state.scroll_capture_owner.is_some()
                {
                    tracing::debug!("SPEC-HAL-038: Scroll capture released on PointerUp");
                    domain_state.clear_wheel_scroll_capture();
                }

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                let captured_id = domain_state
                    .pointer_capture_manager
                    .current_capture(input_pointer_id);
                let hit_result = if captured_id.is_none() {
                    tree.hit_test_for_routing(pos.x, pos.y)
                } else {
                    None
                };
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                let pointer_event = PointerEvent {
                    id: input_pointer_id,
                    global_pos: (pos.x, pos.y),
                    local_pos: (pos.x, pos.y),
                    kind: PointerEventKind::Up,
                    button: Some(button),
                    modifiers: self.current_modifiers,
                    pointer_type: input_pointer_type,
                };

                let pan_ended = touch_motion.is_some_and(|motion| {
                    self.route_active_touch_scroll(tree, motion, root, true, domain_state)
                        || self.route_active_touch_drag(tree, motion, true, domain_state)
                });
                let pending = touch_motion.and_then(|motion| {
                    domain_state.gesture_registry.take_touch_tap(motion.press)
                });
                if pan_ended {
                    // Final position already went to the winning gesture.
                } else if let (Some(pending), Some(motion)) = (pending, touch_motion) {
                    if motion.position.distance(motion.origin)
                        < crate::core::input_fusion::DRAG_DISTANCE_THRESHOLD
                        && !domain_state.gesture_registry.is_committed(input_pointer_id)
                    {
                        if pending.target == crate::core::event::TouchTarget::DismissPopup {
                            let target = hit_result.as_ref().map(|(hit, _)| hit.widget_id);
                            if Self::touch_dismissal_at(tree, pos, target, root).as_ref() == Some(&pending) {
                                tree.dismiss_nonmodal_popup(pending.mount.owner());
                            }
                        } else if let Some((hit, origin)) = hit_result.as_ref()
                            && Self::touch_tap_at(tree, hit, *origin, pos, root).as_ref() == Some(&pending)
                        {
                            if self.is_widget_focusable(tree, hit.widget_id) {
                                self.request_focus_with_text_engine(
                                    tree, hit.widget_id, domain_state, Some(text_engine),
                                );
                            }
                            #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
                            let host_choice = self.request_host_choice(
                                tree, root, domain_state, &pending, motion, text_engine,
                            );
                            #[cfg(not(any(target_arch = "wasm32", target_os = "ios", test)))]
                            let host_choice = false;
                            let editor_tap = pending.target == crate::core::event::TouchTarget::Editor;
                            if editor_tap
                                && domain_state.focus_manager.focused() == Some(hit.widget_id)
                                && domain_state.keyboard_mode != KeyboardAuthorityMode::Capture
                                && let Some(node) = tree.find_by_id(hit.widget_id)
                            {
                                self.enter_capture_with_text_engine(
                                    tree, hit.widget_id, node, domain_state, Some(text_engine),
                                );
                            }
                            let _handled = if host_choice {
                                true
                            } else {
                                self.dispatch_widget_input(
                                    tree, hit.widget_id, Some(&Event::Pointer(pointer_event)),
                                    EventPhase::Target, *origin, text_engine_ptr, domain_state, animation,
                                    WidgetInput::TouchTap(TouchTap {
                                        press: motion.press, target: pending.target,
                                        local_position: Point::new(hit.local_pos.0, hit.local_pos.1),
                                    }),
                                )
                            };
                            #[cfg(any(target_arch = "wasm32", test))]
                            if _handled && editor_tap {
                                self.touch_keyboard_activation = Some((motion, hit.widget_id));
                            }
                        }
                    }
                } else if let Some(captured_id) = captured_id {
                    // Route Up to captured widget
                    let mut local_event = pointer_event.clone();
                    if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            local_event.local_pos = (
                                pos.x + scroll_offset.0 - abs_rect.x,
                                pos.y + scroll_offset.1 - abs_rect.y,
                            );
                        }
                    }
                    self.route_to_widget(
                        tree,
                        captured_id,
                        &Event::Pointer(local_event),
                        text_engine_ptr,
                        domain_state,
                        animation,
                    );
                } else if let Some((hit_result, route_origin)) = hit_result {
                    // No capture: route through event path
                    let event_path = self.build_event_path(tree, hit_result.widget_id);
                    self.dispatch_pointer_event(
                        tree,
                        &event_path,
                        pointer_event,
                        route_origin,
                        text_engine_ptr,
                        domain_state,
                        animation,
                    );
                }

                self.resolve_gesture_commits_for_pointer(tree, pointer_id, domain_state);
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);

                // SPEC-HAL-128 Stage 3.H: Session teardown on PointerUp
                // INV-LIFE-ORDER: Teardown order is:
                //   1. Deliver PointerUp (if captured) - done above in dispatch
                //   2. Resolve commit (if applicable) - gesture resolution placeholder
                //   3. Release capture - done below
                //   4. Clear gesture session - done below
                // INV-PC3: Capture ends on pointer up
                // INV-PC-UP: PointerUp was delivered BEFORE we reach this point
                // INV-079-SESSION-TEARDOWN-IN-POSTPROCESS: Session teardown MUST be in PostProcess
                #[cfg(debug_assertions)]
                debug_assert!(
                    self.current_phase.get() == Some(InputPipelinePhase::PostProcess),
                    "INV-079-SESSION-TEARDOWN-IN-POSTPROCESS: PointerUp teardown outside PostProcess"
                );
                if completes_relative {
                    self.terminate_relative_pointer(
                        Some(tree),
                        domain_state,
                        RelativePointerExit::Completed,
                        self.relative_window_facts.get(),
                    );
                } else if relative_binding.is_none() {
                    self.restore_hidden_cursor_for_capture(input_pointer_id, domain_state);
                    domain_state
                        .pointer_capture_manager
                        .cancel_capture(input_pointer_id);
                    if pointer_id == PointerId::MOUSE {
                        domain_state.pointer_capture.release();
                    }
                    domain_state
                        .gesture_registry
                        .clear_candidates(input_pointer_id);
                    domain_state.end_touch_scroll(input_pointer_id);
                }

                // INV-PC-3: No capture exists after PostProcess completes
                #[cfg(debug_assertions)]
                if relative_binding.is_none() || completes_relative {
                    debug_assert!(
                        !domain_state
                            .pointer_capture_manager
                            .is_captured(input_pointer_id),
                        "INV-PC-3 VIOLATION: Capture persists after PointerUp PostProcess"
                    );
                }

                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            FusedInputEvent::TouchCancel { pointer_id, .. } => {
                let event_kind = "TouchCancel";
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.cancel_pointer_session_with_tree(
                    tree,
                    pointer_id,
                    CancellationReason::SystemCancel,
                    domain_state,
                );
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            // SPEC-HAL-079 Phase-2: Full pipeline enforcement for DragStart
            FusedInputEvent::DragStart { origin, button, .. } => {
                let pointer_id = PointerId::MOUSE;
                let event_kind = "DragStart";

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                let captured_id = domain_state
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // SPEC-HAL-128: Route DragStart to captured widget for explicit activation.
                let pointer_event = PointerEvent {
                    id: PointerId::MOUSE,
                    global_pos: (origin.x, origin.y),
                    local_pos: (origin.x, origin.y),
                    kind: PointerEventKind::DragStart,
                    button: Some(button),
                    modifiers: self.current_modifiers,
                    pointer_type: PointerType::Mouse,
                };

                if let Some(captured_id) = captured_id {
                    let mut local_event = pointer_event.clone();
                    if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            local_event.local_pos = (
                                origin.x + scroll_offset.0 - abs_rect.x,
                                origin.y + scroll_offset.1 - abs_rect.y,
                            );
                        }
                    }
                    self.route_to_widget(
                        tree,
                        captured_id,
                        &Event::Pointer(local_event),
                        text_engine_ptr,
                        domain_state,
                        None,
                    );

                    // SPEC-HAL-241: Check if captured widget wants to initiate drag-and-drop
                    if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                        if let Some(payload) = tree.query_drag_payload(node_id) {
                            let source_size = tree
                                .get_rect(node_id)
                                .map(|r| crate::core::math::Size::new(r.width, r.height))
                                .unwrap_or(crate::core::math::Size::new(100.0, 30.0));
                            domain_state.drag_drop.start(DragSession::new(
                                captured_id,
                                payload,
                                Point::new(origin.x, origin.y),
                                source_size,
                            ));
                        }
                    }
                } else if let Some((hit_result, route_origin)) =
                    tree.hit_test_for_routing(origin.x, origin.y)
                {
                    let target_id = hit_result.widget_id;
                    let event_path = self.build_event_path(tree, target_id);
                    self.dispatch_pointer_event(
                        tree,
                        &event_path,
                        pointer_event,
                        route_origin,
                        text_engine_ptr,
                        domain_state,
                        None,
                    );
                }

                self.resolve_gesture_commits_for_pointer(tree, pointer_id, domain_state);
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            // SPEC-HAL-079 Phase-2: Full pipeline enforcement for DragUpdate
            FusedInputEvent::DragUpdate { current, .. } => {
                let pointer_id = PointerId::MOUSE;
                let event_kind = "DragUpdate";

                // SPEC-HAL-038: Do NOT release scroll_capture_owner on DragUpdate.

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                let captured_id = domain_state
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                let pointer_event = PointerEvent {
                    id: PointerId::MOUSE,
                    global_pos: (current.x, current.y),
                    local_pos: (current.x, current.y),
                    kind: PointerEventKind::Move,
                    button: None,
                    modifiers: self.current_modifiers,
                    pointer_type: PointerType::Mouse,
                };

                if let Some(captured_id) = captured_id {
                    let mut local_event = pointer_event.clone();
                    if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                        let scroll_offset = tree.cumulative_scroll_offset(node_id);
                        if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                            local_event.local_pos = (
                                current.x + scroll_offset.0 - abs_rect.x,
                                current.y + scroll_offset.1 - abs_rect.y,
                            );
                        }
                    }
                    self.route_to_widget(
                        tree,
                        captured_id,
                        &Event::Pointer(local_event),
                        text_engine_ptr,
                        domain_state,
                        None,
                    );
                }

                // SPEC-HAL-241: Track drop target enter/leave during active drag session
                if domain_state.drag_drop.active() {
                    let payload_type = domain_state.drag_drop.session().unwrap().payload_type;
                    let old_target = domain_state.drag_drop.session().unwrap().current_target;

                    // Hit test at current position to find widget under cursor
                    let new_target = tree.hit_test(current.x, current.y).and_then(|hit| {
                        // Walk up from hit widget to find first eligible drop target
                        Self::find_eligible_drop_target(tree, hit.widget_id, payload_type)
                    });

                    if new_target != old_target {
                        // Leave old target
                        if let Some(old_id) = old_target {
                            if let Some(node_id) = tree.find_node_by_widget_id(old_id) {
                                if let Some(node) = tree.node_mut(node_id) {
                                    node.widget.on_drag_leave();
                                }
                            }
                        }
                        // Enter new target
                        if let Some(new_id) = new_target {
                            if let Some(node_id) = tree.find_node_by_widget_id(new_id) {
                                if let Some(node) = tree.node_mut(node_id) {
                                    node.widget.on_drag_enter();
                                }
                            }
                        }
                        let session = domain_state.drag_drop.session_mut().unwrap();
                        session.current_target = new_target;
                    }
                    // Update current position
                    let session = domain_state.drag_drop.session_mut().unwrap();
                    session.current_pos = Point::new(current.x, current.y);
                }

                self.resolve_gesture_commits_for_pointer(tree, pointer_id, domain_state);
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            // SPEC-HAL-079 Phase-2: Full pipeline enforcement for DragEnd
            FusedInputEvent::DragEnd {
                current,
                button,
                cancelled,
                ..
            } => {
                let pointer_id = PointerId::MOUSE;
                let event_kind = "DragEnd";
                let relative_drag = domain_state
                    .relative_pointer_authority
                    .as_ref()
                    .and_then(|authority| authority.active())
                    .is_some_and(|activation| activation.binding().pointer_button() == button);
                if relative_drag && cancelled {
                    self.cancel_active_relative_with_tree(
                        tree,
                        domain_state,
                        RelativePointerCancellationReason::ExplicitSemanticCancel,
                        self.relative_window_facts.get(),
                    );
                }

                // SPEC-HAL-038: Do NOT release scroll_capture_owner on DragEnd.

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                let captured_id = domain_state
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                if !cancelled && !relative_drag {
                    let pointer_event = PointerEvent {
                        id: PointerId::MOUSE,
                        global_pos: (current.x, current.y),
                        local_pos: (current.x, current.y),
                        kind: PointerEventKind::Up,
                        button: Some(button),
                        modifiers: self.current_modifiers,
                        pointer_type: PointerType::Mouse,
                    };

                    if let Some(captured_id) = captured_id {
                        let mut local_event = pointer_event.clone();
                        if let Some(node_id) = tree.find_node_by_widget_id(captured_id) {
                            let scroll_offset = tree.cumulative_scroll_offset(node_id);
                            if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                                local_event.local_pos = (
                                    current.x + scroll_offset.0 - abs_rect.x,
                                    current.y + scroll_offset.1 - abs_rect.y,
                                );
                            }
                        }
                        self.route_to_widget(
                            tree,
                            captured_id,
                            &Event::Pointer(local_event),
                            text_engine_ptr,
                            domain_state,
                            None,
                        );
                    }
                }
                // Cancelled absolute drags skip Up and commit resolution; terminal cancellation runs in PostProcess.

                if !cancelled || relative_drag {
                    self.resolve_gesture_commits_for_pointer(tree, pointer_id, domain_state);
                }
                // SPEC-HAL-241: End drag-and-drop session
                if domain_state.drag_drop.active() {
                    // Deliver drop payload via begin_fade_out (keeps session alive for ghost)
                    let had_target = domain_state
                        .drag_drop
                        .session()
                        .and_then(|s| s.current_target);
                    if let Some(payload) = domain_state.drag_drop.begin_fade_out() {
                        if !cancelled {
                            if let Some(target_id) = had_target {
                                if let Some(node_id) = tree.find_node_by_widget_id(target_id) {
                                    if let Some(node) = tree.node_mut(node_id) {
                                        node.widget.on_drop(payload);
                                        node.widget.on_drag_leave();
                                    }
                                }
                            }
                        } else if let Some(target_id) = had_target {
                            // Cancel: clear highlight on current target
                            if let Some(node_id) = tree.find_node_by_widget_id(target_id) {
                                if let Some(node) = tree.node_mut(node_id) {
                                    node.widget.on_drag_leave();
                                }
                            }
                        }
                    }
                }

                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);

                // SPEC-HAL-128: Session teardown on DragEnd (both cancelled and non-cancelled)
                // INV-PC3: Capture ends when drag interaction completes
                // INV-079-SESSION-TEARDOWN-IN-POSTPROCESS: Session teardown MUST be in PostProcess
                #[cfg(debug_assertions)]
                debug_assert!(
                    self.current_phase.get() == Some(InputPipelinePhase::PostProcess),
                    "INV-079-SESSION-TEARDOWN-IN-POSTPROCESS: DragEnd teardown outside PostProcess"
                );
                if !relative_drag && cancelled {
                    self.cancel_pointer_session_with_tree(
                        tree,
                        pointer_id,
                        CancellationReason::SystemCancel,
                        domain_state,
                    );
                } else if !relative_drag {
                    self.restore_hidden_cursor_for_capture(PointerId::MOUSE, domain_state);
                    domain_state
                        .pointer_capture_manager
                        .cancel_capture(PointerId::MOUSE);
                    domain_state.pointer_capture.release();
                    domain_state
                        .gesture_registry
                        .clear_candidates(PointerId::MOUSE);
                }

                // INV-PC-3: No capture exists after PostProcess completes
                #[cfg(debug_assertions)]
                if !relative_drag || cancelled {
                    debug_assert!(
                        !domain_state
                            .pointer_capture_manager
                            .is_captured(PointerId::MOUSE),
                        "INV-PC-3 VIOLATION: Capture persists after DragEnd PostProcess"
                    );
                }

                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            // SPEC-HAL-079 Phase-2: Full pipeline enforcement for Scroll
            FusedInputEvent::Scroll { pos, delta, .. } => {
                let pointer_id = PointerId::MOUSE;
                let event_kind = "Scroll";

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                let pan_active = domain_state.touch_scroll_source().is_some();
                let hit_result = if pan_active {
                    None
                } else {
                    tree.hit_test_for_routing(pos.x, pos.y)
                };
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                if let Some((hit_result, route_origin)) = hit_result {
                    let target_id = hit_result.widget_id;
                    let event_path = self.build_event_path(tree, target_id);

                    // Find nearest scrollable ancestor (target → root)
                    let mut scroll_target = None;
                    for i in (0..event_path.len()).rev() {
                        let widget_id = event_path[i];
                        if let Some(node_id) = tree.find_node_by_widget_id(widget_id) {
                            let wants_scroll = tree
                                .node(node_id)
                                .map(|node| node.widget.wants_scroll())
                                .unwrap_or(false);

                            if wants_scroll {
                                scroll_target = Some(widget_id);
                                break;
                            }
                        }
                    }

                    if let Some(scroll_widget_id) = scroll_target {
                        domain_state.interrupt_kinetic_scroll();
                        domain_state.scroll_capture_owner = Some(scroll_widget_id);

                        let pointer_event = PointerEvent {
                            id: PointerId::MOUSE,
                            global_pos: (pos.x, pos.y),
                            local_pos: (pos.x, pos.y),
                            kind: PointerEventKind::Scroll {
                                delta_x: delta.0,
                                delta_y: delta.1,
                            },
                            button: None,
                            modifiers: self.current_modifiers,
                            pointer_type: PointerType::Mouse,
                        };
                        let event_path = self.build_event_path(tree, scroll_widget_id);
                        self.dispatch_pointer_event(
                            tree,
                            &event_path,
                            pointer_event,
                            route_origin,
                            text_engine_ptr,
                            domain_state,
                            animation.take(),
                        );
                    } else {
                        // No scrollable widget found — clear stale capture
                        domain_state.clear_wheel_scroll_capture();
                    }
                } else {
                    // Hit-test missed — clear stale capture
                    domain_state.clear_wheel_scroll_capture();
                }

                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            FusedInputEvent::KeyDown {
                time,
                key_code,
                physical_key,
                reservation,
                modifiers,
                repeat,
                ..
            } => {
                if let Some(reservation) = reservation {
                    if reservation == crate::core::event::KeyboardKeyReservation::Clipboard
                        && Self::physical_keyboard_capture_owner(tree, domain_state).is_some()
                    {
                        domain_state.swallow_text_at = Some(time);
                    } else {
                        self.interrupt_physical_keyboard_capture(
                            tree,
                            domain_state,
                            crate::core::event::KeyboardCaptureInterruption {
                                reason:
                                    crate::core::event::KeyboardCaptureInterruptionReason::Reserved(
                                        reservation,
                                    ),
                                key: Some(KeyEvent {
                                    key_code,
                                    physical_key,
                                    is_down: true,
                                    is_repeat: repeat,
                                    modifiers,
                                    text: None,
                                }),
                            },
                            text_engine,
                        );
                        if reservation == crate::core::event::KeyboardKeyReservation::Fullscreen {
                            domain_state.swallow_text_at = Some(time);
                        }
                        return;
                    }
                }
                if key_code == KeyCode::Escape
                    && self.interrupt_physical_keyboard_capture(
                        tree,
                        domain_state,
                        crate::core::event::KeyboardCaptureInterruption {
                            reason: crate::core::event::KeyboardCaptureInterruptionReason::Escape,
                            key: Some(KeyEvent {
                                key_code,
                                physical_key,
                                is_down: true,
                                is_repeat: repeat,
                                modifiers,
                                text: None,
                            }),
                        },
                        text_engine,
                    )
                {
                    domain_state.swallow_text_at = Some(time);
                    return;
                }
                // SPEC-HAL-079 Phase-2: Full pipeline enforcement for KeyDown
                let pointer_id = PointerId::MOUSE; // Keyboard events use MOUSE as synthetic ID
                let event_kind = "KeyDown";

                // Update current modifiers state for pointer events
                self.current_modifiers = modifiers;

                // SPEC-HAL-268: Double-tap Ctrl detection
                if key_code == KeyCode::Control && !repeat {
                    match self.ctrl_double_tap {
                        CtrlDoubleTapState::WaitingSecondPress(t)
                            if t.elapsed() < std::time::Duration::from_millis(300) =>
                        {
                            self.ctrl_double_tap = CtrlDoubleTapState::Active;
                        }
                        _ => {
                            // First press or timeout — reset (will transition on release)
                            self.ctrl_double_tap = CtrlDoubleTapState::Idle;
                        }
                    }
                }
                // Any non-modifier key aborts waiting state
                if !matches!(
                    key_code,
                    KeyCode::Control | KeyCode::Shift | KeyCode::Alt | KeyCode::Meta
                ) {
                    if matches!(
                        self.ctrl_double_tap,
                        CtrlDoubleTapState::WaitingSecondPress(_)
                    ) {
                        self.ctrl_double_tap = CtrlDoubleTapState::Idle;
                    }
                }

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest (no-op for keyboard) =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                if domain_state.ui_interaction_policy()
                    == crate::core::event::UiInteractionPolicy::PointerDriven
                    && domain_state.keyboard_mode == KeyboardAuthorityMode::Navigation
                    && key_code != KeyCode::Escape
                {
                    domain_state.swallow_text_at = Some(time);
                    self.complete_remaining_phases(
                        InputPipelinePhase::Routing,
                        pointer_id,
                        event_kind,
                    );
                    return;
                }

                // Framework-level hjkl→Arrows + i→Enter translation (Navigation mode only).
                // In Capture mode keys pass through unchanged so 'h' etc. type the letter.
                let translated_key =
                    if domain_state.keyboard_mode == KeyboardAuthorityMode::Navigation {
                        match key_code {
                            KeyCode::Character('h') => KeyCode::Left,
                            KeyCode::Character('j') => KeyCode::Down,
                            KeyCode::Character('k') => KeyCode::Up,
                            KeyCode::Character('l') => KeyCode::Right,
                            KeyCode::Character('i') => KeyCode::Enter,
                            other => other,
                        }
                    } else {
                        key_code
                    };

                // SPEC-HAL-107: Build KeyEvent first, dispatch to focused widget before fallbacks
                let key_event = KeyEvent {
                    key_code: translated_key,

                    physical_key,
                    is_down: true,
                    is_repeat: repeat,
                    modifiers,
                    text: None,
                };

                // SPEC-HAL-107: If there's a focused widget, dispatch to it first.
                // If it handles the key, skip all fallback logic.
                if let Some(focused_id) = domain_state.focus_manager.focused() {
                    let activation_mount =
                        if !repeat && matches!(translated_key, KeyCode::Enter | KeyCode::Space) {
                            tree.concrete_mount(focused_id)
                        } else {
                            None
                        };
                    let handled = self.dispatch_to_widget(
                        tree,
                        focused_id,
                        &Event::Key(key_event.clone()),
                        EventPhase::Target,
                        text_engine_ptr,
                        domain_state,
                        None,
                    );
                    if handled {
                        self.note_keyboard_focus_cue(activation_mount, domain_state);
                        if let Some((_, outcome)) = captured_app.as_mut() {
                            **outcome = CapturedAppKeyOutcome::WidgetHandled;
                        }
                        // SPEC-HAL-176: Widget consumed the Key — arm the
                        // paired-TextInput suppression so the residue Text
                        // event from this same physical press (matched by
                        // `time` assigned in platform/window.rs
                        // convert_window_event) is swallowed by
                        // dispatch_engine_action's TextInput branch instead
                        // of reaching the widget. Symmetric to the engine-
                        // level consumption flag write in
                        // dispatch_engine_action's KeyDown branch.
                        domain_state.swallow_text_at = Some(time);

                        // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                        self.complete_remaining_phases(
                            InputPipelinePhase::Routing,
                            pointer_id,
                            event_kind,
                        );
                        return;
                    }
                }

                if let Some((_, outcome)) = captured_app.as_mut() {
                    **outcome = CapturedAppKeyOutcome::AppFallback;
                    self.complete_remaining_phases(
                        InputPipelinePhase::Routing,
                        pointer_id,
                        event_kind,
                    );
                    return;
                }
                // HAL328 §§2.3/20: no App recipient is not permission to
                // reinterpret an unhandled modified key as focus navigation.
                if domain_state.keyboard_mode == KeyboardAuthorityMode::Capture
                    && (modifiers.ctrl || modifiers.alt || modifiers.meta)
                {
                    self.complete_remaining_phases(
                        InputPipelinePhase::Routing,
                        pointer_id,
                        event_kind,
                    );
                    return;
                }

                // SPEC-HAL-176: In Capture mode, widget owns all keys.
                // If widget didn't handle the key, no fallback navigation.
                // Plain Tab/Shift+Tab proceed to the original focus-navigation chain.
                // (Escape is handled by app.rs engine action dispatch before reaching here.)
                let tab_from_capture = domain_state.keyboard_mode
                    == crate::core::event::KeyboardAuthorityMode::Capture
                    && key_code == KeyCode::Tab;
                if domain_state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture
                {
                    if key_code == KeyCode::Tab {
                        if !self.interrupt_physical_keyboard_capture(
                            tree,
                            domain_state,
                            crate::core::event::KeyboardCaptureInterruption {
                                reason: crate::core::event::KeyboardCaptureInterruptionReason::Tab,
                                key: Some(key_event.clone()),
                            },
                            text_engine,
                        ) {
                            domain_state.keyboard_mode =
                                crate::core::event::KeyboardAuthorityMode::Navigation;
                            let te = unsafe { &mut *text_engine_ptr.unwrap() };
                            self.notify_capture_leave(tree, domain_state, te);
                        }
                        if domain_state.ui_interaction_policy()
                            == crate::core::event::UiInteractionPolicy::PointerDriven
                        {
                            self.continuous_frames_requested = true;
                            domain_state.swallow_text_at = Some(time);
                            self.complete_remaining_phases(
                                InputPipelinePhase::Routing,
                                pointer_id,
                                event_kind,
                            );
                            return;
                        }
                    } else {
                        self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);
                        self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                        self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                        return;
                    }
                }

                // Fallback logic: only reached if no focused widget or widget didn't handle key

                // SPEC-HAL-038 Stage 5: Intercept ESC for focus reset (before Tab/Arrow)
                if key_code == KeyCode::Escape {
                    if self.handle_escape(tree, domain_state) {
                        // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                        self.complete_remaining_phases(
                            InputPipelinePhase::Routing,
                            pointer_id,
                            event_kind,
                        );
                        return;
                    }
                    // ESC unhandled, fall through to normal routing
                }

                // SPEC-HAL-038 Stage 6: Intercept Enter/Space for keyboard activation
                // Only on initial press (not repeat) to prevent spam activations
                if matches!(key_code, KeyCode::Enter | KeyCode::Space) && !repeat {
                    // SPEC-HAL-131E: Try projected routing for keyboard activation
                    // If we have an active projected hit context, use it for Enter/Space
                    if let Some(hit) = self.last_projected_hit.clone() {
                        // Verify container still has focus
                        if domain_state.focus_manager.focused() == Some(hit.container_id) {
                            let action = crate::core::input_projection::ProjectedAction::Activate;
                            if self.dispatch_projected_input(tree, &hit, action, domain_state) {
                                // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                                self.complete_remaining_phases(
                                    InputPipelinePhase::Routing,
                                    pointer_id,
                                    event_kind,
                                );
                                return; // Projected routing handled the activation
                            } else {
                                // Dispatch failed (container rejected or item invalid) - clear stale context
                                self.last_projected_hit = None;
                            }
                        } else {
                            // Focus left container - clear stale context
                            self.last_projected_hit = None;
                        }
                    }

                    let mount = domain_state.focus_manager.focused_mount(tree);
                    if self.activate_focused_widget(tree, domain_state, animation.take()) {
                        self.note_keyboard_focus_cue(mount, domain_state);
                        // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                        self.complete_remaining_phases(
                            InputPipelinePhase::Routing,
                            pointer_id,
                            event_kind,
                        );
                        return;
                    }
                    // No focused widget or widget doesn't accept activation, fall through
                }

                // SPEC-HAL-038 Stage 2: Intercept Tab for focus navigation
                if key_code == KeyCode::Tab && !(modifiers.ctrl || modifiers.alt || modifiers.meta)
                {
                    if let Some(root_id) = root {
                        let reverse = modifiers.shift;
                        if self.handle_tab_navigation(tree, root_id, reverse, domain_state) {
                            self.note_keyboard_focus_cue(
                                domain_state.focus_manager.focused_mount(tree),
                                domain_state,
                            );
                        }

                        // SPEC-HAL-328 §3.7: Tab auto-capture chain. If Tab
                        // came from Capture mode and the new target is
                        // capture_eligible, enter Capture on it.
                        if tab_from_capture {
                            if let Some(new_id) = domain_state.focus_manager.focused() {
                                if let Some(node_id) = tree.find_node_by_widget_id(new_id) {
                                    let eligible = tree
                                        .node(node_id)
                                        .is_some_and(|node| {
                                            node.widget.capture_eligible()
                                                && tree.composite_choice_input_ready(node)
                                        });
                                    if eligible {
                                        domain_state.keyboard_mode =
                                            crate::core::event::KeyboardAuthorityMode::Capture;
                                        self.invoke_composite_lifecycle(
                                            tree,
                                            new_id,
                                            CompositeLifecycle::CaptureEnter,
                                            domain_state,
                                            text_engine_ptr,
                                            false,
                                            false,
                                        );
                                    }
                                }
                            }
                            // SPEC-HAL-328 §15: framework guarantees a
                            // continuous frame on mode transition.
                            self.continuous_frames_requested = true;
                        }

                        // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                        self.complete_remaining_phases(
                            InputPipelinePhase::Routing,
                            pointer_id,
                            event_kind,
                        );
                        // Tab handled, skip normal routing
                        return;
                    }
                    // No root provided, fall through to normal routing
                }

                // SPEC-HAL-326: Tree-structured Left/Right fallback.
                // Only reached if widget didn't handle Left/Right.
                if matches!(translated_key, KeyCode::Left | KeyCode::Right) {
                    let direction = if translated_key == KeyCode::Left {
                        Direction::Left
                    } else {
                        Direction::Right
                    };
                    if let Some(focused_id) = domain_state.focus_manager.focused() {
                        if let Some(target) =
                            Self::navigate_tree_structured(tree, focused_id, direction)
                        {
                            self.request_focus_for_widget_directional(
                                tree,
                                target,
                                direction,
                                domain_state,
                            );
                            self.note_keyboard_focus_cue(tree.concrete_mount(target), domain_state);
                            self.complete_remaining_phases(
                                InputPipelinePhase::Routing,
                                pointer_id,
                                event_kind,
                            );
                            return;
                        }
                    }
                }

                // SPEC-HAL-131E: Intercept Up/Down for projected focus navigation
                if matches!(key_code, KeyCode::Up | KeyCode::Down) {
                    if let Some(hit) = self.last_projected_hit.clone() {
                        // Verify container still has focus
                        if domain_state.focus_manager.focused() == Some(hit.container_id) {
                            if let Some(current_index) = hit.logical_index {
                                // Compute next logical index
                                // INV-NO-IMPLICIT-REALIZE-1: No realization, just index math
                                let next_index = match key_code {
                                    KeyCode::Up => current_index.saturating_sub(1),
                                    KeyCode::Down => current_index + 1, // Container validates bounds
                                    _ => unreachable!(),
                                };

                                // Dispatch via helper; update/clear last_projected_hit
                                if let Some(new_hit) = self.dispatch_projected_focus_to_index(
                                    tree,
                                    &hit,
                                    next_index,
                                    domain_state,
                                ) {
                                    self.last_projected_hit = Some(new_hit);
                                    // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                                    self.complete_remaining_phases(
                                        InputPipelinePhase::Routing,
                                        pointer_id,
                                        event_kind,
                                    );
                                    return; // Projected routing handled navigation
                                } else {
                                    self.last_projected_hit = None;
                                }
                            }
                        } else {
                            // Focus left container - clear stale context
                            self.last_projected_hit = None;
                        }
                    }
                }

                // SPEC-HAL-131E: Intercept PageUp/PageDown/Home/End for projected focus navigation
                if matches!(
                    key_code,
                    KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
                ) {
                    if let Some(hit) = self.last_projected_hit.clone() {
                        // Verify container still has focus
                        if domain_state.focus_manager.focused() == Some(hit.container_id) {
                            if let Some(current_index) = hit.logical_index {
                                // Query container for item_count and page_size
                                // INV-NO-IMPLICIT-REALIZE-1: No realization, just queries
                                let mut item_count: Option<usize> = None;
                                let page_size: usize = 10; // Default page size

                                if let Some(node_id) = tree.find_node_by_widget_id(hit.container_id)
                                {
                                    if let Some(node) = tree.node(node_id) {
                                        // Get item_count via InputProjectionProvider
                                        if let Some(provider) =
                                            node.widget.as_input_projection_provider()
                                        {
                                            item_count = Some(provider.logical_item_count());
                                        }
                                    }
                                }

                                // Compute target index based on key
                                let next_index = match (key_code, item_count) {
                                    (KeyCode::Home, _) => Some(0),
                                    (KeyCode::End, Some(count)) if count > 0 => Some(count - 1),
                                    (KeyCode::End, _) => None, // No items or unknown count
                                    (KeyCode::PageUp, _) => {
                                        Some(current_index.saturating_sub(page_size))
                                    }
                                    (KeyCode::PageDown, Some(count)) => {
                                        // Clamp to last valid index
                                        let target = current_index + page_size;
                                        if count > 0 {
                                            Some(target.min(count - 1))
                                        } else {
                                            None
                                        }
                                    }
                                    (KeyCode::PageDown, None) => {
                                        // Container will validate bounds
                                        Some(current_index + page_size)
                                    }
                                    _ => None,
                                };

                                // Dispatch via helper; update/clear last_projected_hit
                                if let Some(next_index) = next_index {
                                    if let Some(new_hit) = self.dispatch_projected_focus_to_index(
                                        tree,
                                        &hit,
                                        next_index,
                                        domain_state,
                                    ) {
                                        self.last_projected_hit = Some(new_hit);
                                        // INV-079-NO-SKIP-PHASES: Complete remaining phases before return
                                        self.complete_remaining_phases(
                                            InputPipelinePhase::Routing,
                                            pointer_id,
                                            event_kind,
                                        );
                                        return; // Projected routing handled navigation
                                    } else {
                                        self.last_projected_hit = None;
                                    }
                                }
                            }
                        } else {
                            // Focus left container - clear stale context
                            self.last_projected_hit = None;
                        }
                    }
                }

                match translated_key {
                    KeyCode::Left | KeyCode::Right => {
                        // SPEC-HAL-328 §8.4: Tree navigation wins when an
                        // axis ancestor exists. If no ancestor declares an
                        // axis, spatial navigation is the sole resolver.
                        let direction = if translated_key == KeyCode::Left {
                            Direction::Left
                        } else {
                            Direction::Right
                        };
                        if let Some(focused_id) = domain_state.focus_manager.focused() {
                            let has_axis_container = tree
                                .find_node_by_widget_id(focused_id)
                                .and_then(|node_id| find_axis_container(tree, node_id))
                                .is_some();
                            if let Some(target) =
                                Self::navigate_tree_structured(tree, focused_id, direction)
                            {
                                self.request_focus_for_widget_directional(
                                    tree,
                                    target,
                                    direction,
                                    domain_state,
                                );
                                self.note_keyboard_focus_cue(
                                    tree.concrete_mount(target),
                                    domain_state,
                                );
                                self.complete_remaining_phases(
                                    InputPipelinePhase::Routing,
                                    pointer_id,
                                    event_kind,
                                );
                                return;
                            }
                            if !has_axis_container {
                                if let Some(root_id) = root {
                                    if self.navigate_directional(
                                        tree,
                                        root_id,
                                        direction,
                                        domain_state,
                                    ) {
                                        self.note_keyboard_focus_cue(
                                            domain_state.focus_manager.focused_mount(tree),
                                            domain_state,
                                        );
                                        self.complete_remaining_phases(
                                            InputPipelinePhase::Routing,
                                            pointer_id,
                                            event_kind,
                                        );
                                        return;
                                    }
                                }
                            }
                        } else if let Some(root_id) = root {
                            if self.navigate_directional(tree, root_id, direction, domain_state) {
                                self.note_keyboard_focus_cue(
                                    domain_state.focus_manager.focused_mount(tree),
                                    domain_state,
                                );
                                self.complete_remaining_phases(
                                    InputPipelinePhase::Routing,
                                    pointer_id,
                                    event_kind,
                                );
                                return;
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Down => {
                        // Tree navigation first, geometric fallback for Up/Down
                        let direction = if translated_key == KeyCode::Up {
                            Direction::Up
                        } else {
                            Direction::Down
                        };
                        if let Some(focused_id) = domain_state.focus_manager.focused() {
                            if let Some(target) =
                                Self::navigate_tree_structured(tree, focused_id, direction)
                            {
                                self.request_focus_for_widget_directional(
                                    tree,
                                    target,
                                    direction,
                                    domain_state,
                                );
                                self.note_keyboard_focus_cue(
                                    tree.concrete_mount(target),
                                    domain_state,
                                );
                                self.complete_remaining_phases(
                                    InputPipelinePhase::Routing,
                                    pointer_id,
                                    event_kind,
                                );
                                return;
                            }
                        }
                        // Geometric fallback for Up/Down only
                        if let Some(root_id) = root {
                            if self.navigate_directional(tree, root_id, direction, domain_state) {
                                self.note_keyboard_focus_cue(
                                    domain_state.focus_manager.focused_mount(tree),
                                    domain_state,
                                );
                                self.complete_remaining_phases(
                                    InputPipelinePhase::Routing,
                                    pointer_id,
                                    event_kind,
                                );
                                return;
                            }
                        }
                    }
                    _ => {}
                }

                // No fallback handled it, route normally (will be a no-op if already dispatched)
                self.route_focused_key_event(tree, key_event, domain_state, animation.take());

                // ===== End of PHASE 3: Routing =====
                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            FusedInputEvent::KeyUp {
                key_code,
                physical_key,
                reservation,
                modifiers,
                ..
            } => {
                if reservation.is_some()
                    && !(reservation == Some(crate::core::event::KeyboardKeyReservation::Clipboard)
                        && Self::physical_keyboard_capture_owner(tree, domain_state).is_some())
                {
                    return;
                }
                // SPEC-HAL-079 Phase-2: Full pipeline enforcement for KeyUp
                let pointer_id = PointerId::MOUSE; // Keyboard events use MOUSE as synthetic ID
                let event_kind = "KeyUp";

                // Update current modifiers state for pointer events
                self.current_modifiers = modifiers;

                // SPEC-HAL-268: Double-tap Ctrl detection
                if key_code == KeyCode::Control {
                    match self.ctrl_double_tap {
                        CtrlDoubleTapState::Active => {
                            // Ctrl released while active — deactivate
                            self.ctrl_double_tap = CtrlDoubleTapState::Idle;
                        }
                        CtrlDoubleTapState::Idle => {
                            // First Ctrl release — start waiting for second press
                            self.ctrl_double_tap =
                                CtrlDoubleTapState::WaitingSecondPress(web_time::Instant::now());
                        }
                        CtrlDoubleTapState::WaitingSecondPress(_) => {
                            // Released again while waiting — reset
                            self.ctrl_double_tap = CtrlDoubleTapState::Idle;
                        }
                    }
                }

                // ===== PHASE 1: PreHitTest =====
                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                // ===== PHASE 2: HitTest (no-op for keyboard) =====
                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                // ===== PHASE 3: Routing =====
                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                let key_event = KeyEvent {
                    key_code,
                    physical_key,
                    is_down: false,
                    is_repeat: false,
                    modifiers,
                    text: None,
                };
                if domain_state.ui_interaction_policy()
                    != crate::core::event::UiInteractionPolicy::PointerDriven
                    || domain_state.keyboard_mode == KeyboardAuthorityMode::Capture
                {
                    self.route_focused_key_event(tree, key_event, domain_state, animation.take());
                }

                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                // ===== PHASE 4: PostProcess =====
                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            FusedInputEvent::TextInput {
                time,
                text,
                modifiers,
                ..
            } => {
                if domain_state.swallow_text_at.take() == Some(time) {
                    return;
                }
                if domain_state.ui_interaction_policy()
                    == crate::core::event::UiInteractionPolicy::PointerDriven
                    && domain_state.keyboard_mode != KeyboardAuthorityMode::Capture
                {
                    return;
                }
                // SPEC-HAL-176: TextInput is a one-shot Press event for
                // composed/IME text. It is dispatched as a Text event
                // to the focused widget — meaningful only in Capture
                // mode where TextField/TextArea consume it for
                // insertion. Bindings (hjkl, 'i', etc.) flow through
                // paired Key events emitted alongside by the platform
                // layer; route_input_event's KeyDown branch handles
                // their translation and widget dispatch. No translation
                // or binding lookup happens here.
                let pointer_id = PointerId::MOUSE;
                let event_kind = "TextInput";

                self.enter_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PreHitTest, pointer_id, event_kind);

                self.enter_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::HitTest, pointer_id, event_kind);

                self.enter_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                if let Some(focused_id) = domain_state.focus_manager.focused() {
                    let text_event = crate::core::event::TextInputEvent { text, modifiers };
                    self.dispatch_to_widget(
                        tree,
                        focused_id,
                        &Event::Text(text_event),
                        EventPhase::Target,
                        text_engine_ptr,
                        domain_state,
                        None,
                    );
                }

                self.exit_phase(InputPipelinePhase::Routing, pointer_id, event_kind);

                self.enter_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
                self.exit_phase(InputPipelinePhase::PostProcess, pointer_id, event_kind);
            }

            FusedInputEvent::Shortcut { .. } => {
                // Shortcut events are handled at fusion level
                // For now, log and ignore - can be extended later
                tracing::debug!("Shortcut event received (not routed to widgets yet)");
            }

            FusedInputEvent::FocusChanged { focused, .. } => {
                // SPEC-HAL-038 Stage 9/10: Window focus events are internal only (not dispatched to widgets)
                if !focused {
                    // Window deactivation: remember focus before clearing
                    if let Some(current_focus) = domain_state.focus_manager.focused() {
                        domain_state.focus_manager.remember_focus(current_focus);
                        tracing::debug!(
                            "SPEC-HAL-038 Stage 10: Remembered focus {:?} before deactivation",
                            current_focus.0
                        );
                    }
                    self.cancel_all_pointer_sessions_with_tree(
                        tree,
                        CancellationReason::SystemCancel,
                        domain_state,
                    );
                    self.clear_hover(tree, domain_state);
                    self.interrupt_physical_keyboard_capture(
                        tree,
                        domain_state,
                        crate::core::event::KeyboardCaptureInterruption {
                            reason:
                                crate::core::event::KeyboardCaptureInterruptionReason::FocusLost,
                            key: None,
                        },
                        text_engine,
                    );
                    self.clear_focus(tree, domain_state);

                    // Reset modifier state on focus loss.
                    // If user holds SHIFT, loses focus, releases SHIFT outside,
                    // then returns — we must not think SHIFT is still held.
                    self.current_modifiers = Modifiers::default();

                    tracing::debug!(
                        "SPEC-HAL-038 Stage 9: Window lost focus, blurred widget, reset modifiers, cancelled captures"
                    );
                } else {
                    // SPEC-HAL-038 Stage 10: Window activation - try to restore focus
                    self.try_restore_focus(tree, domain_state);
                }
            }

            FusedInputEvent::Tick { .. } => {
                // Tick events are not routed to widgets currently
                // Can be used for animations in the future
            }

            FusedInputEvent::Resize { .. } => {
                // INV-RESIZE-SIGNAL-1: Resize events are delivered to Screen::handle_input,
                // not routed through the widget tree. The Screen is responsible for updating
                // any stateful widgets (List, ScrollArea) that depend on viewport dimensions.
            }
        }
    }
}

// ============================================================================
// SPEC-HAL-024 Stage 6: Event Router Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::{Constraints, LayoutResult};
    use crate::core::math::{Point, RectF, Size};
    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
    use crate::core::widget_tree::{NodeId, WidgetDirty, WidgetTree};
    use crate::utils::id::{TreeWidgetId, WidgetInternalId};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn routed_click_count_matches_delivered_pointer_kind() {
        use crate::core::event::{PhysicalKeyCode, PointerDevice};
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use PointerEventKind::{DoubleClick, Down, TripleClick, Up};

        type Observation = (EventPhase, Option<PointerEventKind>, u32);

        struct ClickCountProbe {
            id: WidgetInternalId,
            observations: Rc<RefCell<Vec<Observation>>>,
        }

        impl WidgetDowncast for ClickCountProbe {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        impl Widget for ClickCountProbe {
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

            fn handle_event(
                &mut self,
                cx: &mut crate::core::widget::EventContext,
                event: &Event,
                phase: EventPhase,
            ) -> bool {
                let (kind, handled) = match event {
                    Event::Pointer(pointer) => (Some(pointer.kind), false),
                    Event::Key(_) => (None, true),
                    _ => return false,
                };
                self.observations
                    .borrow_mut()
                    .push((phase, kind, cx.click_count()));
                handled
            }
        }

        let observations = Rc::new(RefCell::new(Vec::new()));
        let probe = || {
            Box::new(ClickCountProbe {
                id: WidgetInternalId::new(),
                observations: observations.clone(),
            })
        };
        let mut tree = WidgetTree::new();
        let root = tree.create_root(probe(), None, 0);
        let target = tree.add_child(root, probe(), None, 0);
        let target_id = tree.widget_id_for_node(target).unwrap();
        {
            let _guard = LayoutAuthorityGuard::enter();
            tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 50.0));
            tree.set_rect(target, RectF::new(100.0, 0.0, 100.0, 50.0));
            crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
            crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }

        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        let mut route = |domain: &mut InteractionDomainState, event| {
            router.route_input_event(
                &mut tree,
                event,
                Some(root),
                &mut text,
                None,
                domain,
                &mut bindings,
            );
        };
        let down = |pos, click_count| FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: web_time::Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count,
        };
        let up = |pos| FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: web_time::Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        };

        let pos = Point::new(150.0, 10.0);
        for (click_count, kind) in [(1, Down), (2, DoubleClick), (3, TripleClick), (1, Down)] {
            route(&mut domain, down(pos, click_count));
            let click_count = u32::from(click_count);
            assert_eq!(
                observations.take(),
                [
                    (EventPhase::Capture, Some(Down), 1),
                    (EventPhase::Target, Some(kind), click_count),
                    (EventPhase::Bubble, Some(kind), click_count),
                ],
            );
            route(&mut domain, up(pos));
            assert_eq!(
                observations.take(),
                [
                    (EventPhase::Capture, Some(Up), 1),
                    (EventPhase::Target, Some(Up), 1),
                    (EventPhase::Bubble, Some(Up), 1),
                ],
            );
        }

        assert_eq!(domain.focus_manager.focused(), Some(target_id));
        route(
            &mut domain,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                key_code: KeyCode::Character('a'),
                physical_key: PhysicalKeyCode::KeyA,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
        );
        assert_eq!(observations.take(), [(EventPhase::Target, None, 1)]);

        domain
            .pointer_capture_manager
            .request_capture(PointerId::MOUSE, target_id)
            .unwrap();
        let outside = Point::new(300.0, 10.0);
        route(&mut domain, down(outside, 2));
        assert_eq!(
            observations.take(),
            [(EventPhase::Target, Some(DoubleClick), 2)]
        );
        route(&mut domain, up(outside));
        assert_eq!(observations.take(), [(EventPhase::Target, Some(Up), 1)]);
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
    }

    struct CaptureContextProbe {
        id: WidgetInternalId,
        redirect_to: Option<TreeWidgetId>,
        preempt_pointer_down: bool,
        release_on_down: bool,
        contexts: Rc<RefCell<Vec<(crate::core::event::KeyboardAuthorityMode, usize)>>>,
    }

    impl WidgetDowncast for CaptureContextProbe {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl CaptureContextProbe {
        fn record_context(&self, cx: &mut crate::core::widget::EventContext) {
            let mode = cx.keyboard_mode;
            let engine = cx.text_engine();
            let engine_id =
                engine as *mut dyn crate::core::text_engine::TextEngine as *mut () as usize;
            let (width, height) = engine.measure("Capture", 16.0);
            assert!(width > 0.0 && height > 0.0);
            self.contexts.borrow_mut().push((mode, engine_id));
        }
    }

    impl Widget for CaptureContextProbe {
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

        fn capture_eligible(&self) -> bool {
            true
        }

        fn handle_event(
            &mut self,
            cx: &mut crate::core::widget::EventContext,
            event: &Event,
            phase: EventPhase,
        ) -> bool {
            if self.preempt_pointer_down
                && phase == EventPhase::Capture
                && matches!(event, Event::Pointer(pe) if pe.kind == PointerEventKind::Down)
            {
                return true;
            }
            if phase != EventPhase::Target {
                return false;
            }
            match event {
                Event::Pointer(pe) if pe.kind == PointerEventKind::Down => {
                    if let Some(target) = self.redirect_to {
                        cx.redirect_focus(target);
                    } else {
                        cx.request_keyboard_capture();
                    }
                    if self.release_on_down {
                        cx.release_keyboard_capture();
                    }
                    true
                }
                Event::Pointer(pe) if pe.kind == PointerEventKind::Up => {
                    cx.release_keyboard_capture();
                    true
                }
                _ => false,
            }
        }

        fn on_capture_enter(&mut self, cx: &mut crate::core::widget::EventContext) {
            self.record_context(cx);
        }

        fn on_capture_leave(&mut self, cx: &mut crate::core::widget::EventContext) {
            self.record_context(cx);
        }
    }

    fn assert_dispatch_capture_context(
        redirect: bool,
        release_on_down: bool,
        initial_capture: bool,
    ) {
        use crate::core::event::{KeyboardAuthorityMode, PointerDevice, WindowId};
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::actions::ActionId;

        let contexts = Rc::new(RefCell::new(Vec::new()));
        let probe = || {
            Box::new(CaptureContextProbe {
                id: WidgetInternalId::new(),
                redirect_to: None,
                preempt_pointer_down: false,
                release_on_down,
                contexts: contexts.clone(),
            })
        };
        let mut tree = WidgetTree::new();
        let root = tree.create_root(probe(), None, 0);
        let target = if redirect {
            let target = tree.add_child(root, probe(), None, 0);
            let owner = tree.widget_id_for_node(target).unwrap();
            tree.node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<CaptureContextProbe>()
                .unwrap()
                .redirect_to = Some(owner);
            target
        } else {
            root
        };
        {
            let _guard = LayoutAuthorityGuard::enter();
            tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 50.0));
            if redirect {
                tree.set_rect(target, RectF::new(100.0, 0.0, 100.0, 50.0));
            }
            crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
            crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }
        let owner = tree.widget_id_for_node(target).unwrap();
        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        let engine_id = &mut text as *mut FontManagerTextEngine as usize;
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        let mut expected = Vec::new();
        if initial_capture {
            let old_owner = tree.widget_id_for_node(root).unwrap();
            router.request_focus_for_widget(&mut tree, old_owner, &mut domain);
            assert!(router.handle_engine_action(
                &mut tree,
                ActionId::ENTER_WIDGET,
                Some(root),
                &mut domain,
                &mut text,
                None,
            ));
            expected.push((KeyboardAuthorityMode::Capture, engine_id));
        }

        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos: Point::new(10.0, 10.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        if initial_capture {
            expected.push((KeyboardAuthorityMode::Navigation, engine_id));
        }
        expected.push((KeyboardAuthorityMode::Capture, engine_id));
        if release_on_down {
            expected.push((KeyboardAuthorityMode::Navigation, engine_id));
            assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
            assert_eq!(*contexts.borrow(), expected);
            return;
        }
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(*contexts.borrow(), expected);

        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos: Point::new(if redirect { 150.0 } else { 10.0 }, 10.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
        expected.push((KeyboardAuthorityMode::Navigation, engine_id));
        assert_eq!(*contexts.borrow(), expected);
    }

    #[test]
    fn capture_context_widget_request_keeps_current_routing_text_engine() {
        assert_dispatch_capture_context(false, false, false);
    }

    #[test]
    fn capture_context_focus_redirect_keeps_current_routing_text_engine() {
        assert_dispatch_capture_context(true, false, false);
    }

    #[test]
    fn capture_context_captured_focus_redirect_keeps_old_and_new_current_engine() {
        assert_dispatch_capture_context(true, false, true);
    }

    #[test]
    fn pointer_capture_entry_preserves_explicit_target_release() {
        assert_dispatch_capture_context(false, true, false);
    }

    #[test]
    fn release_capture_action_notifies_current_engine_once_and_preserves_owners() {
        use crate::core::event::KeyboardAuthorityMode;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::actions::ActionId;

        let contexts = Rc::new(RefCell::new(Vec::new()));
        let probe = || {
            Box::new(CaptureContextProbe {
                id: WidgetInternalId::new(),
                redirect_to: None,
                preempt_pointer_down: false,
                release_on_down: false,
                contexts: contexts.clone(),
            })
        };
        let mut tree = WidgetTree::new();
        let root = tree.create_root(probe(), None, 0);
        let pointer_node = tree.add_child(root, probe(), None, 0);
        let keyboard_owner = tree.widget_id_for_node(root).unwrap();
        let pointer_owner = tree.widget_id_for_node(pointer_node).unwrap();
        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        let engine_id = &mut text as *mut FontManagerTextEngine as usize;
        router.request_focus_for_widget(&mut tree, keyboard_owner, &mut domain);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            &mut domain,
            &mut text,
            None,
        ));
        domain
            .pointer_capture_manager
            .request_capture(PointerId::MOUSE, pointer_owner)
            .unwrap();
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::RELEASE_CAPTURE,
            Some(root),
            &mut domain,
            &mut text,
            None,
        ));
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(domain.focus_manager.focused(), Some(keyboard_owner));
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(pointer_owner)
        );
        let expected = [
            (KeyboardAuthorityMode::Capture, engine_id),
            (KeyboardAuthorityMode::Navigation, engine_id),
        ];
        assert_eq!(*contexts.borrow(), expected);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::RELEASE_CAPTURE,
            Some(root),
            &mut domain,
            &mut text,
            None,
        ));
        assert_eq!(*contexts.borrow(), expected);
        assert_eq!(domain.focus_manager.focused(), Some(keyboard_owner));
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(pointer_owner)
        );
    }

    #[test]
    fn modal_boundary_capture_context_keeps_current_engine_and_pointer_owner() {
        use crate::core::event::KeyboardAuthorityMode;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::actions::ActionId;

        let contexts = Rc::new(RefCell::new(Vec::new()));
        let probe = || {
            Box::new(CaptureContextProbe {
                id: WidgetInternalId::new(),
                redirect_to: None,
                preempt_pointer_down: false,
                release_on_down: false,
                contexts: contexts.clone(),
            })
        };
        let mut tree = WidgetTree::new();
        let root = tree.create_root(probe(), None, 0);
        let modal_root = tree.add_child(root, probe(), None, 0);
        let keyboard_owner = tree.widget_id_for_node(root).unwrap();
        let pointer_owner = tree.widget_id_for_node(modal_root).unwrap();
        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        let engine_id = &mut text as *mut FontManagerTextEngine as usize;
        router.request_focus_for_widget(&mut tree, keyboard_owner, &mut domain);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            &mut domain,
            &mut text,
            None,
        ));
        domain
            .pointer_capture_manager
            .request_capture(PointerId::MOUSE, pointer_owner)
            .unwrap();
        router.enforce_modal_input_boundary_with_text_engine(
            &mut tree,
            modal_root,
            &mut domain,
            Some(&mut text),
        );
        assert_eq!(domain.focus_manager.focused(), None);
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert!(router.pending_focus_reveal.is_none());
        let expected = [
            (KeyboardAuthorityMode::Capture, engine_id),
            (KeyboardAuthorityMode::Navigation, engine_id),
        ];
        assert_eq!(*contexts.borrow(), expected);
        router.enforce_modal_input_boundary_with_text_engine(
            &mut tree,
            modal_root,
            &mut domain,
            Some(&mut text),
        );
        assert_eq!(*contexts.borrow(), expected);
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(pointer_owner)
        );
    }

    fn assert_pointer_focus_capture_context(target_capturable: bool) {
        use crate::core::event::{KeyboardAuthorityMode, PointerDevice, WindowId};
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::actions::ActionId;

        let old_contexts = Rc::new(RefCell::new(Vec::new()));
        let new_contexts = Rc::new(RefCell::new(Vec::new()));
        let clicks = Rc::new(std::cell::Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(CaptureContextProbe {
                id: WidgetInternalId::new(),
                redirect_to: None,
                preempt_pointer_down: false,
                release_on_down: false,
                contexts: old_contexts.clone(),
            }),
            None,
            0,
        );
        let mut text = FontManagerTextEngine::new();
        let engine_id = &mut text as *mut FontManagerTextEngine as usize;
        let target_widget: Box<dyn Widget> = if target_capturable {
            Box::new(CaptureContextProbe {
                id: WidgetInternalId::new(),
                redirect_to: None,
                preempt_pointer_down: false,
                release_on_down: false,
                contexts: new_contexts.clone(),
            })
        } else {
            let clicks = clicks.clone();
            let mut button = crate::widgets::button::Button::with_text("Target")
                .on_click(move || clicks.set(clicks.get() + 1));
            let theme = crate::core::theme::Theme::default();
            let mut cx = LayoutContext::new(&mut tree, &mut text, &theme);
            button.layout(&mut cx, Constraints::tight(Size::new(100.0, 50.0)), &[]);
            Box::new(button)
        };
        let target = tree.add_child(root, target_widget, None, 0);
        {
            let _guard = LayoutAuthorityGuard::enter();
            tree.set_rect(root, RectF::new(0.0, 0.0, 240.0, 50.0));
            tree.set_rect(target, RectF::new(120.0, 0.0, 100.0, 50.0));
            crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
            crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }
        let old_owner = tree.widget_id_for_node(root).unwrap();
        let target_owner = tree.widget_id_for_node(target).unwrap();
        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        router.request_focus_for_widget(&mut tree, old_owner, &mut domain);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            &mut domain,
            &mut text,
            None,
        ));
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);

        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos: Point::new(130.0, 10.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(domain.focus_manager.focused(), Some(target_owner));
        assert_eq!(
            *old_contexts.borrow(),
            [
                (KeyboardAuthorityMode::Capture, engine_id),
                (KeyboardAuthorityMode::Navigation, engine_id),
            ]
        );
        assert_eq!(clicks.get(), 0);
        if target_capturable {
            assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);
            assert_eq!(
                *new_contexts.borrow(),
                [(KeyboardAuthorityMode::Capture, engine_id)]
            );
        } else {
            assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
            assert_eq!(
                domain
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                Some(target_owner)
            );
        }

        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos: Point::new(130.0, 10.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(domain.focus_manager.focused(), Some(target_owner));
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(clicks.get(), usize::from(!target_capturable));
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        if target_capturable {
            assert_eq!(
                *new_contexts.borrow(),
                [
                    (KeyboardAuthorityMode::Capture, engine_id),
                    (KeyboardAuthorityMode::Navigation, engine_id),
                ]
            );
        }
    }

    #[test]
    fn capture_context_pointer_focus_to_capturable_keeps_current_engine() {
        assert_pointer_focus_capture_context(true);
    }

    #[test]
    fn capture_context_pointer_focus_to_button_keeps_current_engine_and_one_click() {
        assert_pointer_focus_capture_context(false);
    }

    fn assert_pointer_capture_entry(preempt_capture: bool) {
        use crate::core::event::{KeyCode, KeyboardAuthorityMode, PointerDevice, WindowId};
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::widgets::slider::Slider;

        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(CaptureContextProbe {
                id: WidgetInternalId::new(),
                redirect_to: None,
                preempt_pointer_down: preempt_capture,
                release_on_down: false,
                contexts: Rc::new(RefCell::new(Vec::new())),
            }),
            None,
            0,
        );
        let mut text = FontManagerTextEngine::new();
        let theme = crate::core::theme::Theme::default();
        let mut slider = Slider::new(0.0, 100.0).with_step(1.0);
        let mut cx = LayoutContext::new(&mut tree, &mut text, &theme);
        slider.layout(&mut cx, Constraints::tight(Size::new(200.0, 50.0)), &[]);
        let target = tree.add_child(root, Box::new(slider), None, 0);
        {
            let _guard = LayoutAuthorityGuard::enter();
            tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 50.0));
            tree.set_rect(target, RectF::new(0.0, 0.0, 200.0, 50.0));
            crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
            crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }
        let owner = tree.widget_id_for_node(target).unwrap();
        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos: Point::new(100.0, 25.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        if preempt_capture {
            assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
            assert_eq!(
                domain
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                None
            );
            assert_eq!(
                tree.node(target)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<Slider>()
                    .unwrap()
                    .value(),
                0.0
            );
            return;
        }
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos: Point::new(100.0, 25.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        let value = tree
            .node(target)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Slider>()
            .unwrap()
            .value();
        assert!(value < 100.0);
        router.route_input_event(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                key_code: KeyCode::Right,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        assert_eq!(
            tree.node(target)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<Slider>()
                .unwrap()
                .value(),
            value + 1.0
        );
    }

    #[test]
    fn pointer_capture_entry_slider_click_then_arrow_edits_value() {
        assert_pointer_capture_entry(false);
    }

    #[test]
    fn pointer_capture_entry_capture_preemption_keeps_slider_unentered() {
        assert_pointer_capture_entry(true);
    }

    /// Test widget that records received events for verification
    struct RecordingWidget {
        /// SPEC-HAL-087: Widget-internal ID (not used by tree)
        internal_id: WidgetInternalId,
        received_events: Rc<RefCell<Vec<(f32, f32)>>>,
    }

    impl RecordingWidget {
        fn new(_tree_id: u64, events: Rc<RefCell<Vec<(f32, f32)>>>) -> Self {
            Self {
                internal_id: WidgetInternalId::new(),
                received_events: events,
            }
        }
    }

    impl WidgetDowncast for RecordingWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for RecordingWidget {
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
            LayoutResult::fit(Size::new(100.0, 50.0))
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

        fn id(&self) -> WidgetInternalId {
            self.internal_id
        }

        fn handle_event(
            &mut self,
            _cx: &mut crate::core::widget::EventContext,
            event: &Event,
            _phase: EventPhase,
        ) -> bool {
            if let Event::Pointer(pe) = event {
                self.received_events
                    .borrow_mut()
                    .push((pe.local_pos.0, pe.local_pos.1));
            }
            false
        }
    }

    struct VisualMutationWidget {
        internal_id: WidgetInternalId,
        handled_events: Rc<RefCell<usize>>,
        hover_enters: Rc<RefCell<usize>>,
        hover_leaves: Rc<RefCell<usize>>,
    }

    impl VisualMutationWidget {
        fn new(
            handled_events: Rc<RefCell<usize>>,
            hover_enters: Rc<RefCell<usize>>,
            hover_leaves: Rc<RefCell<usize>>,
        ) -> Self {
            Self {
                internal_id: WidgetInternalId::new(),
                handled_events,
                hover_enters,
                hover_leaves,
            }
        }
    }

    impl WidgetDowncast for VisualMutationWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for VisualMutationWidget {
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(40.0, 24.0))
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

        fn id(&self) -> WidgetInternalId {
            self.internal_id
        }

        fn handle_event(
            &mut self,
            _cx: &mut crate::core::widget::EventContext,
            _event: &Event,
            _phase: EventPhase,
        ) -> bool {
            *self.handled_events.borrow_mut() += 1;
            true
        }

        fn on_hover_enter(&mut self, _cx: &mut crate::core::widget::EventContext) {
            *self.hover_enters.borrow_mut() += 1;
        }

        fn on_hover_leave(&mut self, _cx: &mut crate::core::widget::EventContext) {
            *self.hover_leaves.borrow_mut() += 1;
        }
    }

    fn clear_retained_dirty(tree: &mut WidgetTree, ids: &[NodeId]) {
        for &id in ids {
            let node = tree.node_mut(id).expect("test node");
            node.dirty_layout = false;
            node.dirty_paint = false;
            node.dirty_composition = false;
            node.dirty_flags = WidgetDirty::Unchanged;
        }
    }

    #[test]
    fn handled_dispatch_invalidates_retained_paint_path() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();
        let root_events = Rc::new(RefCell::new(Vec::new()));
        let root = tree.create_root(Box::new(RecordingWidget::new(1, root_events)), None, 0);
        let handled_events = Rc::new(RefCell::new(0));
        let hover_enters = Rc::new(RefCell::new(0));
        let hover_leaves = Rc::new(RefCell::new(0));
        let child = tree.add_child(
            root,
            Box::new(VisualMutationWidget::new(
                handled_events.clone(),
                hover_enters,
                hover_leaves,
            )),
            None,
            0,
        );
        let widget_id = tree.node(child).expect("child node").id;
        clear_retained_dirty(&mut tree, &[root, child]);

        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        router.enter_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "test");
        let handled = router.dispatch_to_widget(
            &mut tree,
            widget_id,
            &Event::Pointer(PointerEvent {
                id: PointerId::MOUSE,
                global_pos: (4.0, 4.0),
                local_pos: (4.0, 4.0),
                kind: PointerEventKind::Down,
                button: Some(PointerButton::Primary),
                modifiers: Modifiers::default(),
                pointer_type: PointerType::Mouse,
            }),
            EventPhase::Target,
            None,
            &mut domain_state,
            None,
        );
        router.exit_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "test");

        assert!(handled);
        assert_eq!(*handled_events.borrow(), 1);
        assert!(tree.node(child).expect("child node").dirty_paint);
        assert!(tree.node(root).expect("root node").dirty_composition);
    }

    #[test]
    fn hover_lifecycle_invalidates_retained_paint_path() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();
        let root_events = Rc::new(RefCell::new(Vec::new()));
        let root = tree.create_root(Box::new(RecordingWidget::new(1, root_events)), None, 0);
        let handled_events = Rc::new(RefCell::new(0));
        let hover_enters = Rc::new(RefCell::new(0));
        let hover_leaves = Rc::new(RefCell::new(0));
        let child = tree.add_child(
            root,
            Box::new(VisualMutationWidget::new(
                handled_events,
                hover_enters.clone(),
                hover_leaves.clone(),
            )),
            None,
            0,
        );
        let widget_id = tree.node(child).expect("child node").id;
        clear_retained_dirty(&mut tree, &[root, child]);

        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        router.update_hover(&mut tree, Some(widget_id), None, &mut domain_state);

        assert_eq!(*hover_enters.borrow(), 1);
        assert!(tree.node(child).expect("child node").dirty_paint);
        assert!(tree.node(root).expect("root node").dirty_composition);

        clear_retained_dirty(&mut tree, &[root, child]);
        router.update_hover(&mut tree, None, None, &mut domain_state);

        assert_eq!(*hover_leaves.borrow(), 1);
        assert!(tree.node(child).expect("child node").dirty_paint);
        assert!(tree.node(root).expect("root node").dirty_composition);
    }

    /// SPEC-HAL-024 Stage 6: Verify local_pos is computed from absolute_paint_rect
    #[test]
    fn test_local_pos_from_absolute_paint_rect() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();
        let events = Rc::new(RefCell::new(Vec::new()));

        // Root at (0, 0, 200, 100)
        let root = tree.create_root(Box::new(RecordingWidget::new(1, events.clone())), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

        // Child at (10, 20) relative to root, size (80, 60)
        let child = tree.add_child(
            root,
            Box::new(RecordingWidget::new(2, events.clone())),
            None,
            0,
        );
        tree.set_rect(child, RectF::new(10.0, 20.0, 80.0, 60.0));

        // Grandchild at (5, 5) relative to child, size (30, 20)
        let grandchild = tree.add_child(
            child,
            Box::new(RecordingWidget::new(3, events.clone())),
            None,
            0,
        );
        tree.set_rect(grandchild, RectF::new(5.0, 5.0, 30.0, 20.0));

        // Compute absolute positions and paint rects (Stage 3B)
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        // SPEC-HAL-088 Stage 3: Compute viewports for hit-testing
        tree.compute_viewports(root);

        // Verify absolute_paint_rects are correct:
        // Root: (0, 0)
        // Child: (0+10, 0+20) = (10, 20)
        // Grandchild: (10+5, 20+5) = (15, 25)
        assert_eq!(
            tree.get_absolute_paint_rect(grandchild).unwrap(),
            RectF::new(15.0, 25.0, 30.0, 20.0)
        );

        // Simulate a pointer event at absolute (20, 30) - inside grandchild
        // Expected local_pos for grandchild: (20-15, 30-25) = (5, 5)
        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        // SPEC-079: Enter Routing phase before dispatch (normally done by pipeline orchestrator)
        router.enter_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "test");
        #[allow(deprecated)]
        {
            let pointer_event = PointerEvent {
                id: PointerId::MOUSE,
                global_pos: (20.0, 30.0),
                local_pos: (20.0, 30.0), // Will be overwritten
                kind: PointerEventKind::Move,
                button: None,
                modifiers: Modifiers::default(),
                pointer_type: PointerType::Mouse, // SPEC-HAL-128 Stage 7
            };
            router.route_pointer_event(&mut tree, pointer_event, &mut domain_state);
        }

        // The grandchild should have received local_pos = (5, 5)
        let received = events.borrow();
        assert!(
            received
                .iter()
                .any(|(x, y)| (*x - 5.0).abs() < 0.01 && (*y - 5.0).abs() < 0.01),
            "Expected local_pos (5, 5) for grandchild, got: {:?}",
            *received
        );
    }

    /// SPEC-HAL-024 Stage 6: Verify hit_test result is used for routing
    #[test]
    fn test_hit_test_determines_target() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();
        let events = Rc::new(RefCell::new(Vec::new()));

        // Root at (0, 0, 200, 100)
        let root = tree.create_root(Box::new(RecordingWidget::new(1, events.clone())), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

        // Two children side by side
        let left_child = tree.add_child(
            root,
            Box::new(RecordingWidget::new(2, events.clone())),
            None,
            0,
        );
        tree.set_rect(left_child, RectF::new(0.0, 0.0, 100.0, 100.0));

        let right_child = tree.add_child(
            root,
            Box::new(RecordingWidget::new(3, events.clone())),
            None,
            0,
        );
        tree.set_rect(right_child, RectF::new(100.0, 0.0, 100.0, 100.0));

        // Compute absolute positions, paint rects, and viewports
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        // SPEC-HAL-088 Stage 3: Compute viewports for hit-testing
        tree.compute_viewports(root);

        // Hit-test at (50, 50) should hit left_child
        let hit = tree.hit_test(50.0, 50.0).expect("should hit");
        assert_eq!(hit.widget_id, TreeWidgetId(2));

        // Hit-test at (150, 50) should hit right_child
        let hit = tree.hit_test(150.0, 50.0).expect("should hit");
        assert_eq!(hit.widget_id, TreeWidgetId(3));
    }

    // ============================================================================
    // SPEC-HAL-079 Phase-2: Phase Violation Tests
    // ============================================================================

    /// Test that dispatch_to_widget panics when not in Routing phase (debug only)
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-079 VIOLATION")]
    fn test_phase_violation_dispatch_outside_routing() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();
        let events = Rc::new(RefCell::new(Vec::new()));

        let root = tree.create_root(Box::new(RecordingWidget::new(1, events.clone())), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

        // Compute absolute positions
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());

        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        let widget_id = TreeWidgetId(1);

        // Enter HitTest phase (NOT Routing)
        router.enter_phase(InputPipelinePhase::HitTest, PointerId::MOUSE, "test");

        // This should panic: dispatch_to_widget requires Routing phase
        let _ = router.dispatch_to_widget(
            &mut tree,
            widget_id,
            &Event::Pointer(PointerEvent {
                id: PointerId::MOUSE,
                global_pos: (50.0, 50.0),
                local_pos: (50.0, 50.0),
                kind: PointerEventKind::Move,
                button: None,
                modifiers: Modifiers::default(),
                pointer_type: PointerType::Mouse,
            }),
            EventPhase::Target,
            None, // text_engine_ptr - not needed for this test
            &mut domain_state,
            None, // animation
        );
    }

    /// Test that entering a phase while already in a phase panics (debug only)
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-079 VIOLATION")]
    fn test_phase_violation_double_enter() {
        let router = EventRouter::new();

        // Enter PreHitTest
        router.enter_phase(InputPipelinePhase::PreHitTest, PointerId::MOUSE, "test");

        // This should panic: already in a phase
        router.enter_phase(InputPipelinePhase::HitTest, PointerId::MOUSE, "test");
    }

    /// Test that exiting wrong phase panics (debug only)
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "SPEC-079 VIOLATION")]
    fn test_phase_violation_wrong_exit() {
        let router = EventRouter::new();

        // Enter PreHitTest
        router.enter_phase(InputPipelinePhase::PreHitTest, PointerId::MOUSE, "test");

        // This should panic: trying to exit HitTest but we're in PreHitTest
        router.exit_phase(InputPipelinePhase::HitTest, PointerId::MOUSE, "test");
    }

    /// Test that phase transitions work correctly
    #[test]
    fn test_phase_transitions_success() {
        let router = EventRouter::new();

        // Full pipeline should work
        router.enter_phase(InputPipelinePhase::PreHitTest, PointerId::MOUSE, "test");
        router.exit_phase(InputPipelinePhase::PreHitTest, PointerId::MOUSE, "test");

        router.enter_phase(InputPipelinePhase::HitTest, PointerId::MOUSE, "test");
        router.exit_phase(InputPipelinePhase::HitTest, PointerId::MOUSE, "test");

        router.enter_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "test");
        router.exit_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "test");

        router.enter_phase(InputPipelinePhase::PostProcess, PointerId::MOUSE, "test");
        router.exit_phase(InputPipelinePhase::PostProcess, PointerId::MOUSE, "test");
    }

    /// Test that complete_remaining_phases works correctly
    #[test]
    fn test_complete_remaining_phases() {
        let router = EventRouter::new();

        // Enter Routing phase
        router.enter_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "test");

        // Complete remaining phases (should exit Routing, then do PostProcess)
        router.complete_remaining_phases(InputPipelinePhase::Routing, PointerId::MOUSE, "test");

        // Should be able to start a new pipeline
        router.enter_phase(InputPipelinePhase::PreHitTest, PointerId::MOUSE, "test2");
        router.exit_phase(InputPipelinePhase::PreHitTest, PointerId::MOUSE, "test2");
    }
}

#[cfg(test)]
#[path = "event_router/availability_atomic_tests.rs"]
mod availability_atomic_tests;

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "ios")))]
#[path = "event_router/eventlog_selection_tests.rs"]
mod eventlog_selection_tests;
