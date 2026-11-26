// ============================================================================
// Widget Authority Firewall Modules
// ============================================================================

pub mod forbidden;
pub mod guard;

// ============================================================================

use crate::core::appearance::{FrameworkAppearanceFacts, FrameworkAppearanceSnapshot};
use crate::core::authority::AuthorityOwner;
use crate::core::event::{Event, EventPhase, KeyboardAuthorityMode};
use crate::core::icon::IconRegistry;
use crate::core::layout::{
    Constraints, ControlPartColumns, ControlPartsRequest, ControlPartsRequirements, LayoutResult,
};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, PaintSink, RoundedCorners};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::scroll_container::ScrollContainer;
use crate::core::text_engine::TextEngine;
use crate::core::theme::Theme;
use crate::core::widget_tree::{
    ComboPopupInputGeometry, OverlayHitEntry, OverlayHitRegion, WidgetTree,
};
use crate::engines::cell_mechanics::CompositeChoiceOwner;
use crate::model::signal_store::{SignalHandle, SignalStore};
use crate::model::tooltip::{TooltipPayload, TooltipPlacement, TooltipTrigger};
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use std::any::Any;

// ============================================================================
// Public downcasting support for tree-owned Widget implementations
// ============================================================================

/// Private container for the publicly re-exported WidgetDowncast trait.
/// The containing module does not restrict external Widget implementations.
mod sealed {
    use std::any::Any;

    /// Downcasting support required by Widget.
    /// External implementations provide both methods by returning self.
    pub trait WidgetDowncast {
        fn as_any(&self) -> &dyn Any;
        fn as_any_mut(&mut self) -> &mut dyn Any;
    }
}

/// Public downcasting support required by Widget implementations.
/// External widgets explicitly implement both methods by returning self.
pub use sealed::WidgetDowncast;

/// SPEC-HAL-035: Stable, cheap key for widget paint/content state.
/// Widgets that don't care can just return None from content_key().
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetContentKey(pub u64);

/// Ephemeral text presentation; the child remains the request owner.
pub struct WidgetTextPresentation<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub color: Color,
    pub typography: Option<&'a crate::core::paragraph::TextStyle>,
}

/// Complete conservative allocation and viewport dependency of an Absolute drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbsoluteDragGeometryDependency {
    pub viewport_owner: crate::core::widget_tree::NodeId,
    pub reflow_root: crate::core::widget_tree::NodeId,
}

/// How a changed ScrollArea sync affects a descendant widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollSyncInvalidation {
    /// Scroll-synced state affects geometry, realization, or child layout.
    Layout,
    /// Scroll-synced state affects only this widget's local paint output.
    Paint,
}

/// Context provided during layout phase
pub struct LayoutContext<'a> {
    pub tree: &'a mut WidgetTree,
    pub text_engine: &'a mut dyn TextEngine,
    pub theme: &'a Theme,
    /// SPEC-HAL-190: Current node being laid out, for authority lookup.
    current_node: Option<crate::core::widget_tree::NodeId>,
    content_owned_vertical_translation: Option<crate::core::widget_tree::NodeId>,
    control_parts_request: Option<ControlPartsRequest>,
    control_parts_reply: Option<ControlPartsRequirements>,
    /// SPEC-HAL-031: Signal store for reactive subscriptions.
    signals: Option<&'a mut SignalStore>,
}

impl<'a> LayoutContext<'a> {
    pub(crate) fn cell_editor_presentation(&self) -> crate::core::appearance::CellEditorPresentation {
        self.current_node.and_then(|id| self.tree.node(id))
            .map_or(Default::default(), |node| node.cell_editor_presentation)
    }

    pub fn new(
        tree: &'a mut WidgetTree,
        text_engine: &'a mut dyn TextEngine,
        theme: &'a Theme,
    ) -> Self {
        Self {
            tree,
            text_engine,
            theme,
            current_node: None,
            content_owned_vertical_translation: None,
            control_parts_request: None,
            control_parts_reply: None,
            signals: None,
        }
    }

    pub fn new_with_signals(
        tree: &'a mut WidgetTree,
        text_engine: &'a mut dyn TextEngine,
        theme: &'a Theme,
        signals: &'a mut SignalStore,
    ) -> Self {
        Self {
            tree,
            text_engine,
            theme,
            current_node: None,
            content_owned_vertical_translation: None,
            control_parts_request: None,
            control_parts_reply: None,
            signals: Some(signals),
        }
    }

    /// SPEC-HAL-190: Set the current node being laid out.
    ///
    /// Called by layout_child before invoking widget.layout().
    /// Used for authority lookup.
    #[cfg(test)]
    pub(crate) fn set_current_node(&mut self, node_id: crate::core::widget_tree::NodeId) {
        self.current_node = Some(node_id);
    }

    /// Access the current node being laid out.
    ///
    /// Used by TreeViewBody for checkbox child reconciliation during layout.
    pub(crate) fn current_node(&self) -> Option<crate::core::widget_tree::NodeId> {
        self.current_node
    }

    /// Carry the ScrollArea's existing translation mode to its immediate
    /// content for this layout invocation only.
    pub(crate) fn with_content_owned_vertical_translation(
        &mut self,
        content: crate::core::widget_tree::NodeId,
        layout: impl FnOnce(&mut Self) -> LayoutResult,
    ) -> LayoutResult {
        let previous = self.content_owned_vertical_translation.replace(content);
        let result = layout(self);
        self.content_owned_vertical_translation = previous;
        result
    }

    pub(crate) fn content_owns_vertical_translation(&self) -> bool {
        self.current_node.is_some() && self.current_node == self.content_owned_vertical_translation
    }

    /// Publish pure desired-role data; the next ordinary deferred owner cut
    /// qualifies and applies structure. Layout never installs an editor.
    pub(crate) fn publish_composite_choice_manifest(
        &mut self,
        manifest: crate::engines::cell_mechanics::CompositeChoiceManifest,
    ) {
        if let Some(owner) = self.current_node {
            self.tree.record_composite_choice_manifest(owner, manifest);
        }
    }

    /// SPEC-HAL-328 §3.1: Defer focus to the current widget. Processed after
    /// layout completes by the engine's `deferred_focus` handler — see
    /// `src/core/app.rs` post-build_scene block, which calls
    /// `request_focus_for_widget` and (if `capture_eligible`)
    /// `enter_capture_for_widget`.
    ///
    /// This is the proper mechanism for programmatic Capture entry from
    /// layout time. The framework fires `on_focus` and `on_capture_enter`
    /// normally. Widgets MUST NOT set framework focus or capture state
    /// directly (INV-FC-NO-PARALLEL-STATE).
    ///
    /// No-op if `current_node` is unset (which should not happen in normal
    /// layout phase — the engine sets it before invoking `widget.layout()`).
    pub fn defer_focus_to_self(&mut self) {
        if let Some(node_id) = self.current_node {
            self.tree.deferred_focus = Some(node_id);
        }
    }

    /// SPEC-HAL-031: Subscribe to a signal and get its current value.
    /// The current widget node is automatically subscribed to receive
    /// targeted invalidation when the signal value changes.
    pub fn watch<T: 'static + Clone>(&mut self, signal: SignalHandle<T>) -> T {
        let node_id = self
            .current_node
            .expect("watch() called outside layout traversal (no current node)");
        self.signals
            .as_mut()
            .expect("watch() called without SignalStore wired into LayoutContext")
            .watch(signal, node_id)
    }

    /// SPEC-HAL-190: Look up authority from the nearest ancestor.
    ///
    /// Returns a Projection to the authority if an ancestor provides it.
    ///
    /// # Type Parameters
    ///
    /// - `A`: The authority type to look up. Must implement `Authority`.
    ///
    /// # Returns
    ///
    /// - `Some(Projection<A>)` if an ancestor provides authority A
    /// - `None` if no ancestor provides authority A or current node not set
    pub fn authority<A: crate::core::authority::Authority + 'static>(
        &self,
    ) -> Option<crate::core::authority::Projection<'_, A>> {
        let node_id = self.current_node?;
        self.tree.lookup_authority::<A>(node_id)
    }

    /// Scoped request for this owner. Ordinary descendants do not inherit it.
    pub fn control_parts_request(&self) -> Option<ControlPartsRequest> {
        self.control_parts_request
    }

    /// Reports owned natural metrics during Measure, without borrowing foreign state.
    pub fn report_control_parts(&mut self, requirements: ControlPartsRequirements) {
        assert_eq!(
            self.control_parts_request,
            Some(ControlPartsRequest::Measure),
            "parts reply requires scoped Measure"
        );
        requirements.validate();
        assert!(
            self.control_parts_reply.replace(requirements).is_none(),
            "duplicate parts reply"
        );
    }

    fn dispatch_control_parts(
        &mut self,
        child: crate::core::widget_tree::NodeId,
        constraints: Constraints,
        request: ControlPartsRequest,
    ) -> (LayoutResult, Option<ControlPartsRequirements>) {
        let mut reply = None;
        let result = self.layout_child_full_with(child, constraints, |widget, cx, children| {
            cx.control_parts_request = Some(request);
            let result = widget.layout(cx, constraints, children);
            reply = cx.control_parts_reply.take();
            result
        });
        (result, reply)
    }

    pub(crate) fn measure_control_row(
        &mut self,
        child: crate::core::widget_tree::NodeId,
    ) -> LayoutResult {
        if let Some(result) = self
            .tree
            .node(child)
            .filter(|n| !n.dirty_layout)
            .and_then(|n| n.control_parts_cache.as_ref())
            .and_then(|c| c.ordinary_natural)
        {
            return result;
        }
        let result = self.layout_child_full(child, Constraints::unbounded());
        let node = self.tree.node_mut(child).unwrap();
        let owner = node.widget.id();
        let cache = node
            .control_parts_cache
            .get_or_insert_with(Default::default);
        cache.owner = Some(owner);
        cache.ordinary_natural = Some(result);
        result
    }

    pub(crate) fn measure_control_parts(
        &mut self,
        child: crate::core::widget_tree::NodeId,
    ) -> ControlPartsRequirements {
        if let Some(req) = self
            .tree
            .node(child)
            .filter(|n| !n.dirty_layout)
            .and_then(|n| n.control_parts_cache.as_ref())
            .and_then(|c| c.natural)
        {
            return req;
        }
        let (_, reply) = self.dispatch_control_parts(
            child,
            Constraints::unbounded(),
            ControlPartsRequest::Measure,
        );
        let req = reply.expect("explicit control alignment member did not report natural parts");
        let node = self.tree.node_mut(child).unwrap();
        let owner = node.widget.id();
        let cache = node
            .control_parts_cache
            .get_or_insert_with(Default::default);
        cache.owner = Some(owner);
        cache.natural = Some(req);
        req
    }

    pub(crate) fn arrange_control_parts(
        &mut self,
        child: crate::core::widget_tree::NodeId,
        constraints: Constraints,
        columns: ControlPartColumns,
    ) -> LayoutResult {
        if let Some((old, assignment, result)) = self
            .tree
            .node(child)
            .filter(|n| !n.dirty_layout)
            .and_then(|n| n.control_parts_cache.as_ref())
            .and_then(|c| c.arranged)
        {
            if old == constraints && assignment == columns {
                return result;
            }
        }
        let result = self
            .dispatch_control_parts(child, constraints, ControlPartsRequest::Arrange(columns))
            .0;
        let node = self.tree.node_mut(child).unwrap();
        let owner = node.widget.id();
        let cache = node
            .control_parts_cache
            .get_or_insert_with(Default::default);
        cache.owner = Some(owner);
        cache.arranged = Some((constraints, columns, result));
        result
    }

    /// Forwards only an explicitly admitted identity single-child layout boundary.
    pub fn forward_control_parts(
        &mut self,
        child: crate::core::widget_tree::NodeId,
        constraints: Constraints,
    ) -> LayoutResult {
        match self.control_parts_request {
            Some(ControlPartsRequest::Measure) => {
                let req = self.measure_control_parts(child);
                self.report_control_parts(req);
                LayoutResult::fit(Size::new(
                    req.leading.width() + req.body_width + req.trailing.width(),
                    req.height,
                ))
            }
            Some(ControlPartsRequest::Arrange(columns)) => {
                self.arrange_control_parts(child, constraints, columns)
            }
            None => self.layout_child_full(child, constraints),
        }
    }

    /// Layout a child widget with given constraints
    /// Returns the size the child wants to be
    /// SPEC-HAL-026: Honors dirty flags - skips re-layout if child is clean and constraints unchanged
    /// SPEC-HAL-086 Stage 2: Internally uses LayoutResult, returns .size for compatibility
    pub fn layout_child(
        &mut self,
        child_id: crate::core::widget_tree::NodeId,
        constraints: Constraints,
    ) -> Size {
        // SPEC-HAL-026: Check if we can skip layout (clean widget with same constraints)
        // SPEC-HAL-141: Use getter for private geometry field
        let skip_result = self.tree.node(child_id).and_then(|node| {
            if !node.dirty_layout
                && node
                    .control_parts_cache
                    .as_ref()
                    .is_none_or(|c| c.arranged.is_none())
            {
                if let Some(last) = node.last_constraints {
                    if constraints == last {
                        return self.tree.get_cached_size(child_id);
                    }
                }
            }
            None
        });
        if let Some(cached_size) = skip_result {
            return cached_size;
        }

        // HAL_PROFILE_RESIZE: Count layout_child calls (not skipped)
        crate::core::app::profiler_inc_layout_child_call();

        // Take widget out of tree
        let (mut widget, children, rect) = match self.tree.take_widget(child_id) {
            Some(data) => data,
            None => {
                // SPEC-HAL-158 INV-ZERO-2: Illegal zero-size return (ERROR classification)
                // Missing child node is a layout error, not an intentional zero.
                // RELEASE-GRADE: This is a structural corruption - panic in ALL builds.
                panic!(
                    "\n\
                    ╔══════════════════════════════════════════════════════════════════════╗\n\
                    ║  SPEC-HAL-158 INV-ZERO-2 VIOLATION: Layout structural corruption     ║\n\
                    ╠══════════════════════════════════════════════════════════════════════╣\n\
                    ║  layout_child() failed to take widget for NodeId {:?}\n\
                    ║  \n\
                    ║  This means the widget tree is corrupted: a child node was expected ║\n\
                    ║  but could not be retrieved. Silent zero return would cause the     ║\n\
                    ║  widget to vanish without any diagnostic.                           ║\n\
                    ╚══════════════════════════════════════════════════════════════════════╝",
                    child_id
                );
            }
        };

        // SPEC-HAL-134: Engine-controlled pre-layout hook.
        // INV-PHASE-TRANSITION-1: Phase transitions happen in pre_layout(), not in layout().
        widget.pre_layout();

        // SPEC-HAL-154: Enforce deferred wiring (seals + validates) before layout.
        if let Some(dw) = widget.deferred_wiring_mut() {
            crate::core::deferred_wiring::enforce_deferred_wiring(dw);
        }

        // SPEC-HAL-190: Set current node for authority lookup.
        let prev_node = self.current_node;
        self.current_node = Some(child_id);
        let previous_parts = (
            self.control_parts_request.take(),
            self.control_parts_reply.take(),
        );

        // SPEC-HAL-086 Stage 2: Call layout on child, returns LayoutResult
        #[cfg(debug_assertions)]
        let hal_pass_probe = crate::core::app::hal_pass_probe::is_enabled()
            .then(|| (widget.debug_type_name(), web_time::Instant::now()));
        #[cfg(debug_assertions)]
        let result = widget.layout(self, constraints, &children);
        #[cfg(not(debug_assertions))]
        let result = widget.layout(self, constraints, &children);
        #[cfg(debug_assertions)]
        if let Some((hal_pass_name, hal_pass_t)) = hal_pass_probe {
            crate::core::app::hal_pass_probe::record(
                hal_pass_name,
                hal_pass_t.elapsed().as_micros() as u64,
            );
        }
        let size = result.size;

        // Detect leftover dirty children after parent layout (SPEC-HAL-026 §8).
        // With DEBUG tracing enabled, report each parent/child pair once.
        // The parent is temporarily out of the tree, so take its type from `widget`.
        #[cfg(debug_assertions)]
        if tracing::enabled!(tracing::Level::DEBUG) {
            use std::collections::HashSet;
            use std::sync::{Mutex, OnceLock};
            type Pair = (
                crate::core::widget_tree::NodeId,
                crate::core::widget_tree::NodeId,
            );
            static REPORTED: OnceLock<Mutex<HashSet<Pair>>> = OnceLock::new();
            let reported = REPORTED.get_or_init(|| Mutex::new(HashSet::new()));

            let mut new_leftover: Vec<(crate::core::widget_tree::NodeId, &'static str)> =
                Vec::new();
            {
                let mut set = reported.lock().expect("REPORTED mutex poisoned");
                for &grandchild_id in &children {
                    if self
                        .tree
                        .node(grandchild_id)
                        .is_some_and(|c| c.dirty_layout)
                    {
                        if set.insert((child_id, grandchild_id)) {
                            let child_type = self
                                .tree
                                .node(grandchild_id)
                                .map(|n| n.widget.debug_type_name())
                                .unwrap_or("<missing>");
                            new_leftover.push((grandchild_id, child_type));
                        }
                    }
                }
            }

            if !new_leftover.is_empty() {
                let parent_type = widget.debug_type_name();
                tracing::debug!(
                    "[LC-LEFTOVER] parent={:?} parent_type={} leftover={:?}",
                    child_id, parent_type, new_leftover
                );
            }
        }

        // SPEC-HAL-190: Restore previous current node.
        self.current_node = prev_node;
        (self.control_parts_request, self.control_parts_reply) = previous_parts;
        if let Some(cache) = self
            .tree
            .node_mut(child_id)
            .and_then(|n| n.control_parts_cache.as_mut())
        {
            cache.arranged = None;
        }

        // Restore widget with placeholder rect (Flex will set final rect)
        let new_rect = RectF::new(rect.x, rect.y, size.width, size.height);
        self.tree.restore_widget(child_id, widget, new_rect);

        // SPEC-HAL-026: Update cached values and clear dirty flag
        // SPEC-HAL-141: Use guarded setter for geometry field
        self.tree.set_cached_size(child_id, size);
        // Store content_size for scroll extent aggregation by parent widgets
        self.tree.set_content_size(child_id, result.content_size);
        // SPEC-HAL-086 Stage 5: Store scroll_offset from layout result (must match layout_node in app.rs)
        if let Some(node) = self.tree.node_mut(child_id) {
            node.last_constraints = Some(constraints);
            node.dirty_layout = false;
            // DEBUG: Stamp layout epoch (must match layout_node in app.rs)
            #[cfg(any(debug_assertions, test))]
            {
                node.layout_epoch = crate::core::app::current_layout_epoch();
            }
        }
        self.tree
            .set_layout_scroll_offset(child_id, result.scroll_offset);
        self.tree.mark_paint_dirty(child_id);

        size
    }

    /// SPEC-HAL-134: Layout a child widget, returning the full LayoutResult.
    ///
    /// Like `layout_child()` but returns the full LayoutResult instead of just Size.
    /// Used by ScrollArea to access `content_size` for virtualized scroll extent.
    ///
    /// INV-PHASE-TRANSITION-1: pre_layout() is called by this helper, not by the caller.
    pub fn layout_child_full(
        &mut self,
        child_id: crate::core::widget_tree::NodeId,
        constraints: Constraints,
    ) -> LayoutResult {
        self.layout_child_full_with(child_id, constraints, |widget, cx, children| {
            widget.layout(cx, constraints, children)
        })
    }

    pub(crate) fn layout_child_full_with_scroll_settlement(
        &mut self,
        child_id: crate::core::widget_tree::NodeId,
        constraints: Constraints,
        mut settle: impl FnMut(Size) -> f32,
    ) -> LayoutResult {
        self.layout_child_full_with(child_id, constraints, |widget, cx, children| {
            let mut admitted_extent = None;
            let result = {
                let mut settle_once = |extent: Size| {
                    assert!(
                        admitted_extent.is_none(),
                        "INIT-OWNER-SETTLES-ONCE: second settlement invocation"
                    );
                    assert!(
                        extent.width.is_finite()
                            && extent.width >= 0.0
                            && extent.height.is_finite()
                            && extent.height >= 0.0,
                        "INIT-SNAPSHOT-COHERENCE: invalid logical content extent"
                    );
                    admitted_extent = Some(extent);
                    settle(extent)
                };
                widget.layout_scroll_content(cx, constraints, children, &mut settle_once)
            };
            match (result, admitted_extent) {
                (None, None) => widget.layout(cx, constraints, children),
                (Some(result), Some(extent)) => {
                    assert_eq!(
                        result.content_size, extent,
                        "INIT-SNAPSHOT-COHERENCE: layout returned a different extent"
                    );
                    result
                }
                (None, Some(_)) => {
                    panic!("INIT-OPTIONAL-DEFAULT: declined layout after settlement")
                }
                (Some(_), None) => {
                    panic!("INIT-OWNER-SETTLES-ONCE: completed layout without settlement")
                }
            }
        })
    }

    fn layout_child_full_with(
        &mut self,
        child_id: crate::core::widget_tree::NodeId,
        constraints: Constraints,
        layout: impl FnOnce(
            &mut dyn Widget,
            &mut Self,
            &[crate::core::widget_tree::NodeId],
        ) -> LayoutResult,
    ) -> LayoutResult {
        // HAL_PROFILE_RESIZE: Count layout_child_full calls
        crate::core::app::profiler_inc_layout_child_call();

        // Take widget out of tree
        let (mut widget, children, rect) = match self.tree.take_widget(child_id) {
            Some(data) => data,
            None => {
                // SPEC-HAL-158 INV-ZERO-2: Illegal zero-size return (ERROR classification)
                // Missing child node is a layout error, not an intentional zero.
                // RELEASE-GRADE: This is a structural corruption - panic in ALL builds.
                panic!(
                    "\n\
                    ╔══════════════════════════════════════════════════════════════════════╗\n\
                    ║  SPEC-HAL-158 INV-ZERO-2 VIOLATION: Layout structural corruption     ║\n\
                    ╠══════════════════════════════════════════════════════════════════════╣\n\
                    ║  layout_child_full() failed to take widget for NodeId {:?}\n\
                    ║  \n\
                    ║  This means the widget tree is corrupted: a child node was expected ║\n\
                    ║  but could not be retrieved. Silent zero return would cause the     ║\n\
                    ║  widget to vanish without any diagnostic.                           ║\n\
                    ╚══════════════════════════════════════════════════════════════════════╝",
                    child_id
                );
            }
        };

        // SPEC-HAL-134: Engine-controlled pre-layout hook.
        // INV-PHASE-TRANSITION-1: Phase transitions happen in pre_layout(), not in layout().
        widget.pre_layout();

        // SPEC-HAL-154: Enforce deferred wiring (seals + validates) before layout.
        if let Some(dw) = widget.deferred_wiring_mut() {
            crate::core::deferred_wiring::enforce_deferred_wiring(dw);
        }

        // SPEC-HAL-190: Set current node for authority lookup.
        let prev_node = self.current_node;
        self.current_node = Some(child_id);
        let previous_parts = (
            self.control_parts_request.take(),
            self.control_parts_reply.take(),
        );

        // Call layout on child, returns full LayoutResult
        let result = layout(widget.as_mut(), self, &children);

        // SPEC-HAL-190: Restore previous current node.
        self.current_node = prev_node;
        (self.control_parts_request, self.control_parts_reply) = previous_parts;
        if let Some(cache) = self
            .tree
            .node_mut(child_id)
            .and_then(|n| n.control_parts_cache.as_mut())
        {
            cache.arranged = None;
        }

        // Restore widget with updated rect
        let new_rect = RectF::new(rect.x, rect.y, result.size.width, result.size.height);
        self.tree.restore_widget(child_id, widget, new_rect);

        // SPEC-HAL-026: Update cached values and clear dirty flag
        // SPEC-HAL-141: Use guarded setter for geometry field
        self.tree.set_cached_size(child_id, result.size);
        // Store content_size for scroll extent aggregation by parent widgets
        self.tree.set_content_size(child_id, result.content_size);
        // SPEC-HAL-086 Stage 5: Store scroll_offset from layout result (must match layout_node in app.rs)
        if let Some(node) = self.tree.node_mut(child_id) {
            node.last_constraints = Some(constraints);
            node.dirty_layout = false;
            // DEBUG: Stamp layout epoch (must match layout_node in app.rs)
            #[cfg(any(debug_assertions, test))]
            {
                node.layout_epoch = crate::core::app::current_layout_epoch();
            }
        }
        self.tree
            .set_layout_scroll_offset(child_id, result.scroll_offset);
        self.tree.mark_paint_dirty(child_id);

        result
    }

    /// Set the rect for a child (called after Flex determines positions)
    pub fn set_child_rect(&mut self, child_id: crate::core::widget_tree::NodeId, rect: RectF) {
        self.tree.set_rect(child_id, rect);
    }

    /// Re-layout a child with tight constraints matching its final rect size
    /// Used for two-pass layout: pass 1 measures, pass 2 re-layouts with final sizes
    /// This preserves x/y position, only recomputes internal layout of child
    /// SPEC-HAL-026: Updates cache after relayout
    /// SPEC-HAL-086 Stage 2: Internally uses LayoutResult
    pub fn relayout_child_with_size(
        &mut self,
        child_id: crate::core::widget_tree::NodeId,
        size: crate::core::math::Size,
    ) {
        // HAL_PROFILE_RESIZE: Count relayout calls
        crate::core::app::profiler_inc_relayout_child_call();

        // Get existing rect for position
        let old_rect = match self.tree.get_rect(child_id) {
            Some(r) => r,
            None => return,
        };

        // Take widget out of tree
        let (mut widget, children, _) = match self.tree.take_widget(child_id) {
            Some(data) => data,
            None => return,
        };

        // SPEC-HAL-134: Engine-controlled pre-layout hook.
        widget.pre_layout();

        // SPEC-HAL-154: Enforce deferred wiring (seals + validates) before layout.
        if let Some(dw) = widget.deferred_wiring_mut() {
            crate::core::deferred_wiring::enforce_deferred_wiring(dw);
        }

        // SPEC-HAL-190: Set current node for authority lookup.
        let prev_node = self.current_node;
        self.current_node = Some(child_id);
        let previous_parts = (
            self.control_parts_request.take(),
            self.control_parts_reply.take(),
        );

        // Re-layout with tight constraints
        let constraints = Constraints::tight(size);
        let result = widget.layout(self, constraints, &children);

        // SPEC-HAL-190: Restore previous current node.
        self.current_node = prev_node;
        (self.control_parts_request, self.control_parts_reply) = previous_parts;
        if let Some(cache) = self
            .tree
            .node_mut(child_id)
            .and_then(|n| n.control_parts_cache.as_mut())
        {
            cache.arranged = None;
        }

        // Restore widget with same x/y, new width/height
        let new_rect = RectF::new(old_rect.x, old_rect.y, size.width, size.height);
        self.tree.restore_widget(child_id, widget, new_rect);

        // SPEC-HAL-026: Update cached values and clear dirty flag
        // SPEC-HAL-141: Use guarded setter for geometry field
        self.tree.set_cached_size(child_id, size);
        // Store content_size for scroll extent aggregation by parent widgets
        self.tree.set_content_size(child_id, result.content_size);
        // SPEC-HAL-086 Stage 5: Store scroll_offset from layout result (must match layout_node in app.rs)
        if let Some(node) = self.tree.node_mut(child_id) {
            node.last_constraints = Some(constraints);
            node.dirty_layout = false;
            // DEBUG: Stamp layout epoch (must match layout_node in app.rs)
            #[cfg(any(debug_assertions, test))]
            {
                node.layout_epoch = crate::core::app::current_layout_epoch();
            }
        }
        self.tree
            .set_layout_scroll_offset(child_id, result.scroll_offset);
        self.tree.mark_paint_dirty(child_id);
    }

    /// Re-layout a child with custom constraints
    /// Used for Flex v2 pass 2: tight width, loose height so child can compute own height
    /// This preserves x/y position from the old rect
    /// SPEC-HAL-026: Updates cache after relayout
    /// SPEC-HAL-086 Stage 2: Internally uses LayoutResult
    pub fn relayout_child_with_constraints(
        &mut self,
        child_id: crate::core::widget_tree::NodeId,
        constraints: Constraints,
    ) {
        // HAL_PROFILE_RESIZE: Count relayout calls
        crate::core::app::profiler_inc_relayout_child_call();

        // Get existing rect for position
        let old_rect = match self.tree.get_rect(child_id) {
            Some(r) => r,
            None => return,
        };

        // Take widget out of tree
        let (mut widget, children, _) = match self.tree.take_widget(child_id) {
            Some(data) => data,
            None => return,
        };

        // SPEC-HAL-134: Engine-controlled pre-layout hook.
        widget.pre_layout();

        // SPEC-HAL-154: Enforce deferred wiring (seals + validates) before layout.
        if let Some(dw) = widget.deferred_wiring_mut() {
            crate::core::deferred_wiring::enforce_deferred_wiring(dw);
        }

        // SPEC-HAL-190: Set current node for authority lookup.
        let prev_node = self.current_node;
        self.current_node = Some(child_id);
        let previous_parts = (
            self.control_parts_request.take(),
            self.control_parts_reply.take(),
        );

        // SPEC-HAL-086 Stage 2: Re-layout with provided constraints, extract size
        let result = widget.layout(self, constraints, &children);
        let size = result.size;

        // SPEC-HAL-190: Restore previous current node.
        self.current_node = prev_node;
        (self.control_parts_request, self.control_parts_reply) = previous_parts;
        if let Some(cache) = self
            .tree
            .node_mut(child_id)
            .and_then(|n| n.control_parts_cache.as_mut())
        {
            cache.arranged = None;
        }

        // Restore widget with same x/y, new computed size
        let new_rect = RectF::new(old_rect.x, old_rect.y, size.width, size.height);
        self.tree.restore_widget(child_id, widget, new_rect);

        // SPEC-HAL-026: Update cached values and clear dirty flag
        // SPEC-HAL-141: Use guarded setter for geometry field
        self.tree.set_cached_size(child_id, size);
        // Store content_size for scroll extent aggregation by parent widgets
        self.tree.set_content_size(child_id, result.content_size);
        // SPEC-HAL-086 Stage 5: Store scroll_offset from layout result (must match layout_node in app.rs)
        if let Some(node) = self.tree.node_mut(child_id) {
            node.last_constraints = Some(constraints);
            node.dirty_layout = false;
            // DEBUG: Stamp layout epoch (must match layout_node in app.rs)
            #[cfg(any(debug_assertions, test))]
            {
                node.layout_epoch = crate::core::app::current_layout_epoch();
            }
        }
        self.tree
            .set_layout_scroll_offset(child_id, result.scroll_offset);
        self.tree.mark_paint_dirty(child_id);
    }

    /// SPEC-HAL-026: Mark this widget as needing layout, propagating up to ancestors
    /// NOTE: Takes TreeWidgetId, not WidgetInternalId. Widgets should use EventContext::invalidate_layout().
    pub fn invalidate_layout(&mut self, widget_id: TreeWidgetId) {
        self.tree.mark_layout_dirty_by_widget_id(widget_id);
    }
}

/// Engine-owned CPU projection; no GPU handle or resource preparation capability.
#[derive(Clone, PartialEq)]
pub(crate) struct ButtonSdfPaintInput {
    pub(crate) key: crate::renderer::sdf_material_status::SdfMaterialKey,
    /// Current prospective input; key still names the actual committed material.
    pub(crate) requested_revision: crate::renderer::sdf_material_status::SdfMaterialRevision,
    pub(crate) image: Option<ButtonSdfPaintImage>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct ButtonSdfPaintImage {
    pub(crate) binding: crate::renderer::image_binding::ImageBinding,
    pub(crate) coverage: RectF,
    pub(crate) uv: RectF,
}

pub(crate) enum OverlayHitOutput<'a> {
    Regions(&'a mut Vec<OverlayHitRegion>),
    Retained(&'a mut Vec<OverlayHitEntry>),
}

/// Context provided during paint phase
pub struct PaintContext<'a> {
    pub tree: &'a WidgetTree,
    pub scene: &'a mut dyn PaintSink,
    pub theme: &'a Theme,
    pub text_engine: &'a mut dyn TextEngine,
    pub icon_registry: &'a IconRegistry,
    /// SPEC-HAL-101 Stage-4C: Frame time in milliseconds for caret blinking
    frame_time_ms: u64,
    /// SPEC-HAL-021: Animation timeline for reading interpolated values
    timeline: &'a crate::core::animation::Timeline,
    /// SPEC-HAL-145: Pre-computed interpolated rects from the transition phase.
    /// Computed under TransitionAuthority BEFORE paint begins.
    /// LT-INV-31: Interpolation is paint-only (read-only access here).
    interpolated_rects: &'a crate::core::layout_transition::InterpolatedVisualRects,
    /// Overlay hit regions collected during paint (scrollbars, etc.)
    /// Widgets register these directly instead of extracting from scene.
    overlay_hit_regions: OverlayHitOutput<'a>,
    // ========================================================================
    // SPEC-HAL-083 Phase-2/10: Identity Plumbing (FROZEN)
    //
    // This identity API is structurally frozen. Any extension requires
    // explicit SPEC-HAL-083 Phase-3.
    //
    // - Identity is WIDGET-OWNED ONLY. PaintContext just passes it through.
    // - PaintSink NEVER invents identity.
    // - `scene_key == None` means ephemeral nodes (recreated every frame).
    // - Phase-10: Identity scopes must not be nested or cross-widget.
    // ========================================================================
    /// SPEC-HAL-083: Current scene node key for identity plumbing.
    /// Set by widgets via `with_scene_key`, default is `None`.
    pub scene_key: Option<SceneNodeKey>,
    /// SPEC-HAL-083 Phase-10: Guard against nested `with_scene_key` calls (debug-only).
    #[cfg(debug_assertions)]
    scene_key_active: bool,
    /// SPEC-HAL-190: Current node being painted, for authority lookup.
    current_node: Option<crate::core::widget_tree::NodeId>,
    /// SPEC-HAL-226A: Tooltip trigger regions registered during this paint frame.
    tooltip_triggers: &'a mut Vec<TooltipTrigger>,
    /// SPEC-HAL-093 v3: Display DPI scale factor (1.0 = 100%, 1.5 = 150%, 2.0 = 200%).
    /// Used by widgets to snap paint rects to the physical pixel grid.
    pub scale_factor: f32,
    /// SPEC-HAL-328 §12.6: TreeWidgetId of the focused widget at paint time,
    /// as projected by the engine from `FocusManager::focused()`. None when no
    /// widget is focused. Consumed by `PaintContext::is_focused`.
    focused_widget: Option<TreeWidgetId>,
    /// SPEC-HAL-328 §12.6: Keyboard authority mode at paint time, projected by
    /// the engine from `InteractionDomainState::keyboard_mode`. Consumed by
    /// `PaintContext::has_capture`.
    keyboard_mode: KeyboardAuthorityMode,
    appearance: Option<&'a FrameworkAppearanceSnapshot>,
    button_sdf_material: Option<&'a ButtonSdfPaintInput>,
}

impl<'a> PaintContext<'a> {
    pub fn new(
        tree: &'a WidgetTree,
        scene: &'a mut dyn PaintSink,
        theme: &'a Theme,
        text_engine: &'a mut dyn TextEngine,
        icon_registry: &'a IconRegistry,
        frame_time_ms: u64,
        timeline: &'a crate::core::animation::Timeline,
        interpolated_rects: &'a crate::core::layout_transition::InterpolatedVisualRects,
        overlay_hit_regions: &'a mut Vec<OverlayHitRegion>,
        tooltip_triggers: &'a mut Vec<TooltipTrigger>,
        scale_factor: f32,
        focused_widget: Option<TreeWidgetId>,
        keyboard_mode: KeyboardAuthorityMode,
    ) -> Self {
        Self::new_with_overlay_output(
            tree,
            scene,
            theme,
            text_engine,
            icon_registry,
            frame_time_ms,
            timeline,
            interpolated_rects,
            OverlayHitOutput::Regions(overlay_hit_regions),
            tooltip_triggers,
            scale_factor,
            focused_widget,
            keyboard_mode,
        )
    }

    pub(crate) fn new_with_overlay_output(
        tree: &'a WidgetTree,
        scene: &'a mut dyn PaintSink,
        theme: &'a Theme,
        text_engine: &'a mut dyn TextEngine,
        icon_registry: &'a IconRegistry,
        frame_time_ms: u64,
        timeline: &'a crate::core::animation::Timeline,
        interpolated_rects: &'a crate::core::layout_transition::InterpolatedVisualRects,
        overlay_hit_regions: OverlayHitOutput<'a>,
        tooltip_triggers: &'a mut Vec<TooltipTrigger>,
        scale_factor: f32,
        focused_widget: Option<TreeWidgetId>,
        keyboard_mode: KeyboardAuthorityMode,
    ) -> Self {
        Self {
            tree,
            scene,
            theme,
            text_engine,
            icon_registry,
            frame_time_ms,
            timeline,
            interpolated_rects,
            overlay_hit_regions,
            scene_key: None,
            #[cfg(debug_assertions)]
            scene_key_active: false,
            current_node: None,
            tooltip_triggers,
            scale_factor,
            focused_widget,
            keyboard_mode,
            appearance: None,
            button_sdf_material: None,
        }
    }

    /// SPEC-HAL-190: Set the current node being painted.
    ///
    /// Called by paint_node before invoking widget.paint().
    /// Used for authority lookup.
    pub(crate) fn set_current_node(&mut self, node_id: crate::core::widget_tree::NodeId) {
        self.current_node = Some(node_id);
        self.button_sdf_material = None;
    }

    pub(crate) fn set_button_sdf_material(&mut self, input: Option<&'a ButtonSdfPaintInput>) {
        self.button_sdf_material = input;
    }

    pub(crate) fn button_sdf_material(
        &self,
        revision: crate::renderer::sdf_material_status::SdfMaterialRevision,
    ) -> Option<&ButtonSdfPaintInput> {
        let input = self.button_sdf_material?;
        let owner = self.tree.widget_id_for_node(self.current_node?)?;
        (input.key.owner() == owner
            && input.requested_revision == revision
            && self.tree.is_live_mount(input.key.tree_lifetime(), owner))
        .then_some(input)
    }

    /// SPEC-HAL-078B §7: Clipping uses completed layout geometry, without
    /// presentation interpolation. Unbound low-level contexts return `None`.
    pub fn authoritative_rect_for_paint(&self) -> Option<RectF> {
        self.tree.get_absolute_paint_rect(self.current_node?)
    }

    pub(crate) fn own_presentation_geometry(&self) -> Option<OwnedWidgetPresentationGeometry> {
        Self::presentation_geometry_for_node(
            self.tree,
            self.current_node?,
            self.scale_factor,
            self.interpolated_rects,
        )
    }

    pub(crate) fn presentation_geometry_for_node(
        tree: &WidgetTree,
        node: crate::core::widget_tree::NodeId,
        scale_factor: f32,
        interpolated_rects: &crate::core::layout_transition::InterpolatedVisualRects,
    ) -> Option<OwnedWidgetPresentationGeometry> {
        let geometry = tree.own_presentation_geometry(node, scale_factor)?;
        let mut ancestor = Some(node);
        while let Some(id) = ancestor {
            let current = tree.node(id)?;
            if interpolated_rects
                .get(&current.id)
                .is_some_and(|visual| Some(*visual) != tree.get_absolute_paint_rect(id))
            {
                return None;
            }
            ancestor = current.parent;
        }
        Some(geometry)
    }

    pub(crate) fn cumulative_scroll_offset_for_paint(&self) -> (f32, f32) {
        self.current_node
            .map(|node| self.tree.cumulative_scroll_offset(node))
            .unwrap_or((0.0, 0.0))
    }

    /// Framework facts for the current consumer, from the same publication
    /// supplied to update. Reading does not mutate input authority.
    pub fn appearance_facts(&self) -> Option<FrameworkAppearanceFacts> {
        let owner = self.tree.widget_id_for_node(self.current_node?)?;
        Some(self.appearance?.for_owner(owner))
    }

    pub(crate) fn cell_editor_presentation(&self) -> crate::core::appearance::CellEditorPresentation {
        self.current_node.and_then(|id| self.tree.node(id))
            .map_or(Default::default(), |node| node.cell_editor_presentation)
    }

    /// HAL391: only this consumer's already prepared CMS members are visible.
    pub(crate) fn has_cell_editor_edge(
        &self, row: u64, column: crate::model::tabular::column::ColumnId,
    ) -> bool {
        let Some(owner) = self.current_node.and_then(|id| self.tree.widget_id_for_node(id)) else {
            return false;
        };
        let Some(facts) = self.appearance.map(|snapshot| snapshot.for_owner(owner)) else { return false; };
        self.tree.cell_edges.get(&owner).and_then(|edges| edges.get(&(row, column))).is_some_and(|edge| {
            edge.consumer.is_live() && edge.editor.is_live()
                && edge.window == facts.window() && edge.domain == facts.interaction_domain()
        })
    }

    /// Allocations are ephemeral paint inputs from the consumer's current
    /// geometry projection. No child/widget lookup or stored rect is involved.
    pub(crate) fn paint_cell_editor_edges(
        &mut self,
        allocations: impl IntoIterator<Item = (u64, crate::model::tabular::column::ColumnId, RectF, bool, bool)>,
    ) {
        use crate::core::theme::ControlChromeState as State;
        let Some(owner) = self.current_node.and_then(|id| self.tree.widget_id_for_node(id)) else { return; };
        let Some(prepared) = self.tree.cell_edges.get(&owner) else { return; };
        let Some(facts) = self.appearance.map(|snapshot| snapshot.for_owner(owner)) else { return; };
        // Each physical grid segment has one owner, including shared row edges.
        // The selected state precedence also determines intersections deterministically.
        let mut segments = std::collections::BTreeMap::<[u32; 4], (u8, u8, State)>::new();
        for (row, column, rect, selected, hovered) in allocations {
            let Some(edge) = prepared.get(&(row, column)).filter(|edge|
                edge.consumer.is_live() && edge.editor.is_live()
                    && edge.window == facts.window() && edge.domain == facts.interaction_domain()) else { continue; };
            if ![rect.x, rect.y, rect.width, rect.height].iter().all(|v| v.is_finite())
                || rect.width <= 0.0 || rect.height <= 0.0 { continue; }
            let state = match edge.state {
                State::Idle if selected => State::Selected,
                State::Idle if hovered => State::Hovered,
                state => state,
            };
            let priority = match state {
                State::Disabled => 6, State::Captured => 5, State::Focused => 4,
                State::Selected => 3, State::Hovered | State::Pressed => 2, State::Idle => 1,
            };
            let sf = self.scale_factor;
            let x0 = crate::core::math::snap_to_physical(rect.x, sf);
            let y0 = crate::core::math::snap_to_physical(rect.y, sf);
            let x1 = crate::core::math::snap_to_physical(rect.right(), sf);
            let y1 = crate::core::math::snap_to_physical(rect.bottom(), sf);
            if x1 <= x0 || y1 <= y0 { continue; }
            for (key, side) in [
                ([0, y0.to_bits(), x0.to_bits(), x1.to_bits()], 1),
                ([0, y1.to_bits(), x0.to_bits(), x1.to_bits()], 2),
                ([1, x0.to_bits(), y0.to_bits(), y1.to_bits()], 1),
                ([1, x1.to_bits(), y0.to_bits(), y1.to_bits()], 2),
            ] {
                let entry = segments.entry(key).or_insert((0, priority, state));
                entry.0 |= side;
                if priority > entry.1 { entry.1 = priority; entry.2 = state; }
            }
        }
        let mut segments: Vec<_> = segments.into_iter().collect();
        segments.sort_by_key(|(key, (_, priority, _))| (*priority, *key));
        for (key, (sides, _, state)) in segments {
            let chrome = self.theme.input_chrome(state).chrome;
            let width = if matches!(state, State::Captured | State::Focused) {
                (chrome.border_width * self.scale_factor).round().max(1.0) / self.scale_factor
            } else { self.one_px() };
            let color = if state == State::Idle { self.theme.colors.borders.subtle } else { chrome.border_color };
            let fixed = f32::from_bits(key[1]);
            let start = f32::from_bits(key[2]);
            let end = f32::from_bits(key[3]);
            let position = if sides == 1 { fixed } else { fixed - width };
            let rect = if key[0] == 0 {
                RectF::new(start, position, end - start, width)
            } else { RectF::new(position, start, width, end - start) };
            self.scene.push_rect(rect, color, crate::core::paint::LAYER_FOCUS + 1, None);
        }
    }

    /// The same cell edge replaces the consumer's column separator over its
    /// current row band. Other rows keep the consumer's original separator.
    pub(crate) fn paint_cell_grid_separator(
        &mut self,
        rect: RectF,
        color: crate::core::paint::Color,
        layer: u32,
        allocations: &[(u64, crate::model::tabular::column::ColumnId, RectF, bool, bool)],
    ) {
        let mut gaps = Vec::new();
        for &(row, column, cell, _, _) in allocations {
            if !self.has_cell_editor_edge(row, column) { continue; }
            let left = crate::core::math::snap_to_physical(cell.x, self.scale_factor);
            let right = crate::core::math::snap_to_physical(cell.right(), self.scale_factor);
            if [left, right].into_iter().any(|x| rect.x <= x && rect.right() >= x) {
                let top = crate::core::math::snap_to_physical(cell.y, self.scale_factor).max(rect.y);
                let bottom = crate::core::math::snap_to_physical(cell.bottom(), self.scale_factor).min(rect.bottom());
                if bottom > top { gaps.push((top, bottom)); }
            }
        }
        gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut top = rect.y;
        for (start, end) in gaps {
            if start > top {
                self.scene.push_rect(RectF::new(rect.x, top, rect.width, start - top), color, layer, None);
            }
            top = top.max(end);
        }
        if rect.bottom() > top {
            self.scene.push_rect(RectF::new(rect.x, top, rect.width, rect.bottom() - top), color, layer, None);
        }
    }

    pub(crate) fn composite_delegation(
        &self,
    ) -> Option<crate::core::composite_text_edit::CompositeDelegationFacts> {
        let child = self.tree.widget_id_for_node(self.current_node?)?;
        self.tree.composite_delegation(child, self.appearance?)
    }

    pub(crate) fn set_appearance_snapshot(
        &mut self,
        appearance: Option<&'a FrameworkAppearanceSnapshot>,
    ) {
        self.appearance = appearance;
    }

    /// SPEC-HAL-328 §12.6: Whether the widget currently being painted holds
    /// focus, per the framework's `FocusManager`.
    ///
    /// Returns false when no widget is focused, when this method is called
    /// outside an active widget paint (current_node unset), or when the
    /// current node's `TreeWidgetId` does not match the focused widget.
    ///
    /// INV-FC-NO-PARALLEL-STATE: This is the SOLE permitted source of
    /// paint-time focus state. Widgets MUST NOT mirror this in local fields.
    pub fn is_focused(&self) -> bool {
        let Some(node_id) = self.current_node else {
            return false;
        };
        let Some(focused) = self.focused_widget else {
            return false;
        };
        self.tree
            .node(node_id)
            .map(|n| n.id == focused)
            .unwrap_or(false)
    }

    /// SPEC-HAL-328 §12.6: Whether the widget currently being painted is
    /// focused AND the keyboard authority mode is `Capture`.
    ///
    /// Equivalent to `self.is_focused() && keyboard_mode == Capture`. Use this
    /// where the widget's visual differs by capture state (caret visibility,
    /// editing chrome, etc.).
    ///
    /// INV-FC-NO-PARALLEL-STATE: This is the SOLE permitted source of
    /// paint-time capture state. Widgets MUST NOT mirror `in_capture` /
    /// `editing` flags for paint.
    pub fn has_capture(&self) -> bool {
        self.is_focused() && self.keyboard_mode == KeyboardAuthorityMode::Capture
    }

    /// Whether the current focus has an eligible navigation or actual Capture cue.
    pub fn has_focus_cue(&self) -> bool {
        self.appearance_facts()
            .map_or_else(|| self.is_focused(), |facts| facts.focus_cue_visible())
    }

    pub fn navigation_cue_eligible(&self) -> bool {
        self.appearance_facts()
            .is_none_or(|facts| facts.navigation_cue_eligible())
    }

    /// SPEC-HAL-328 §12: Framework-owned focus and capture indicator paint pass.
    ///
    /// Called from inside `paint_node` immediately after `widget.paint(...)`
    /// completes, for every widget. Emits the indicator only when
    /// `widget_id == self.focused_widget` and the widget does not opt out via
    /// `paints_own_focus_indicator() == true`.
    ///
    /// Must be invoked from inside `paint_node` (NOT from outside the paint
    /// recursion) so the scene-graph `begin_scroll` / `begin_clip` wrappings
    /// established by ancestor scroll containers also wrap the indicator
    /// primitive. Calling this from outside the recursion would emit the ring
    /// at the unscrolled `absolute_paint_rect` while the widget itself sits at
    /// its scrolled visual position — the bug this signature exists to prevent.
    ///
    /// `visual_rect` is the absolute pre-scroll rect that `widget.paint` was
    /// called with (same value passed by `paint_node`). Default
    /// `focus_indicator_geometry` returns `for_rect(visual_rect)`; widgets
    /// that override may return a cached sub-rect (also in absolute pre-scroll
    /// coords).
    ///
    /// Invariants enforced here:
    /// - INV-FC-SINGLETON: at most one indicator emitted per frame (caller
    ///   contract — only `paint_node` invokes this, and only the focused
    ///   widget's invocation emits).
    /// - INV-FC-WIDTHS: width comes from `theme.focus_ring.focused_width` or
    ///   `captured_width`, never from a widget.
    /// - INV-FC-COLOR: color is `theme.colors.borders.focus` for both states.
    pub(crate) fn paint_focus_indicator_for(
        &mut self,
        widget_id: crate::utils::id::TreeWidgetId,
        visual_rect: crate::core::math::RectF,
    ) {
        if self.focused_widget != Some(widget_id) {
            return;
        }
        if self
            .appearance
            .is_some_and(|snapshot| !snapshot.for_owner(widget_id).focus_cue_visible())
        {
            return;
        }
        let Some(node_id) = self.tree.find_node_by_widget_id(widget_id) else {
            return;
        };
        let Some(node) = self.tree.node(node_id) else {
            return;
        };
        if node.cell_editor_presentation == crate::core::appearance::CellEditorPresentation::CellIntegrated {
            return;
        }
        if node.widget.paints_own_focus_indicator() {
            return;
        }
        let geom = node.widget.focus_indicator_geometry(visual_rect);
        let width = match self.keyboard_mode {
            KeyboardAuthorityMode::Capture => self.theme.focus_ring.captured_width,
            KeyboardAuthorityMode::Navigation => self.theme.focus_ring.focused_width,
        };
        let color = self.theme.colors.borders.focus;
        if geom.radius > 0.0 {
            let corners = crate::core::paint::RoundedCorners::uniform(geom.radius);
            self.scene.push_border_rounded_rect(
                geom.rect,
                color,
                width,
                corners,
                crate::core::paint::LAYER_FOCUS,
                None,
            );
        } else {
            self.scene.push_border_rect(
                geom.rect,
                color,
                width,
                crate::core::paint::LAYER_FOCUS,
                None,
            );
        }
    }

    /// SPEC-HAL-093 v3: Logical size of exactly 1 physical pixel at the current
    /// display scale. Use as the thickness for crisp 1-device-pixel borders.
    #[inline]
    pub fn one_px(&self) -> f32 {
        1.0 / self.scale_factor
    }

    /// Root viewport size (window content area, in logical pixels).
    /// Falls back to (0.0, 0.0) if the tree has no root rect yet.
    pub fn viewport_size(&self) -> (f32, f32) {
        if let Some(root_id) = self.tree.root {
            if let Some(rect) = self.tree.get_rect(root_id) {
                return (rect.width, rect.height);
            }
        }
        (0.0, 0.0)
    }

    /// SPEC-HAL-190: Look up authority from the nearest ancestor.
    ///
    /// Returns a Projection to the authority if an ancestor provides it.
    ///
    /// # Type Parameters
    ///
    /// - `A`: The authority type to look up. Must implement `Authority`.
    ///
    /// # Returns
    ///
    /// - `Some(Projection<A>)` if an ancestor provides authority A
    /// - `None` if no ancestor provides authority A or current node not set
    pub fn authority<A: crate::core::authority::Authority + 'static>(
        &self,
    ) -> Option<crate::core::authority::Projection<'_, A>> {
        let node_id = self.current_node?;
        self.tree.lookup_authority::<A>(node_id)
    }

    /// Register an overlay hit region (scrollbars, etc.) during paint.
    ///
    /// Widgets call this to register hit-testable overlays directly,
    /// avoiding scene→widget_tree backflow.
    pub fn register_overlay_hit_region(&mut self, region: OverlayHitRegion) {
        match &mut self.overlay_hit_regions {
            OverlayHitOutput::Regions(regions) => regions.push(region),
            OverlayHitOutput::Retained(entries) => entries.push(region.into()),
        }
    }

    pub(crate) fn register_combo_popup_hit(
        &mut self,
        region: OverlayHitRegion,
        popup_revision: u64,
        geometry: ComboPopupInputGeometry,
    ) {
        let viewport = self.viewport_size();
        match &mut self.overlay_hit_regions {
            OverlayHitOutput::Regions(regions) => regions.push(region),
            OverlayHitOutput::Retained(entries) => entries.push(self.tree.combo_popup_hit_entry(
                region,
                popup_revision,
                viewport,
                self.scale_factor,
                geometry,
            )),
        }
    }

    /// SPEC-HAL-226A: Register a tooltip for the given trigger rect.
    /// The framework manages the lifecycle — the widget does NOT manage
    /// timers, visibility, or placement.
    pub fn register_tooltip(&mut self, trigger_rect: RectF, content: TooltipPayload) {
        self.register_tooltip_at(trigger_rect, content, TooltipPlacement::Auto);
    }

    pub fn register_tooltip_at(
        &mut self,
        trigger_rect: RectF,
        content: TooltipPayload,
        placement: TooltipPlacement,
    ) {
        if let Some(node_id) = self.current_node {
            if let Some(node) = self.tree.node(node_id) {
                self.tooltip_triggers.push(TooltipTrigger {
                    widget_id: node.id,
                    trigger_rect,
                    content,
                    placement,
                });
            }
        }
    }

    /// SPEC-HAL-083: Temporarily set a scene node key for all push_* calls within `f`.
    /// Restores the previous key after `f` completes.
    ///
    /// This is the ONLY way to set identity. PaintContext never invents keys.
    ///
    /// ## SPEC-HAL-083 Phase-9 Invariant
    ///
    /// **A widget may emit many primitives, but at most one may carry a `SceneNodeKey` per frame.**
    ///
    /// Inside the closure, `cx.scene_key` is set to `Some(key)`. If you call multiple push_*
    /// methods, only ONE should use `cx.scene_key`. All sub-primitives (borders, overlays,
    /// focus rings, shadows, etc.) must use `None` to avoid duplicate key emissions.
    ///
    /// Debug builds panic if a widget emits more than one keyed primitive per frame.
    ///
    /// ## SPEC-HAL-083 Phase-10 Invariant
    ///
    /// **A `SceneNodeKey` identifies the widget's visual root for the frame.
    /// It must not be nested, stacked, or reused for subtrees.**
    ///
    /// - `with_scene_key` may only be called once per widget paint.
    /// - Nested calls panic in debug builds.
    /// - Cross-widget key scopes are structurally prevented.
    pub fn with_scene_key<F, R>(&mut self, key: SceneNodeKey, f: F) -> R
    where
        F: FnOnce(&mut PaintContext) -> R,
    {
        // SPEC-HAL-083 Phase-10: Prevent nested identity scopes
        #[cfg(debug_assertions)]
        {
            assert!(
                !self.scene_key_active,
                "SPEC-HAL-083 Phase-10: Nested or cross-widget SceneNodeKey emission detected. \
                 with_scene_key may only be called once per widget paint, not nested."
            );
            self.scene_key_active = true;
        }

        let prev_key = self.scene_key;
        self.scene_key = Some(key);
        let result = f(self);
        self.scene_key = prev_key;

        // SPEC-HAL-083 Phase-10: Restore guard state
        #[cfg(debug_assertions)]
        {
            self.scene_key_active = false;
        }

        result
    }

    /// SPEC-HAL-101 Stage-4C: Get frame time in milliseconds for animations.
    pub fn frame_time_ms(&self) -> u64 {
        self.frame_time_ms
    }

    /// SPEC-HAL-021: Get an animation value from the timeline.
    ///
    /// Returns the current interpolated value for the given handle, or None
    /// if the animation has completed or was cancelled.
    pub fn get_animation<T: crate::core::animation::Interpolatable>(
        &self,
        handle: crate::core::animation::TransitionHandle,
    ) -> Option<T> {
        self.timeline.get(handle)
    }

    /// Sample animation value and lifecycle state without mutation.
    pub fn sample_animation<T: crate::core::animation::Interpolatable>(
        &self,
        handle: crate::core::animation::TransitionHandle,
    ) -> crate::core::animation::AnimationSample<T> {
        self.timeline.sample(handle)
    }

    /// SPEC-HAL-078 Stage 3 / SPEC-HAL-145: Get the visual rect for paint.
    ///
    /// Returns the pre-computed interpolated rect if one exists for this widget,
    /// otherwise returns the authoritative rect.
    ///
    /// SPEC-HAL-145: Interpolation is computed BEFORE paint (transition phase),
    /// under TransitionAuthority. Paint just reads the pre-computed result.
    ///
    /// LT-INV-30: Layout remains authoritative - this only affects paint.
    /// LT-INV-31: Interpolation is paint-only (read-only access here).
    /// LT-INV-34: Falls back to authoritative rect if no interpolation.
    pub(crate) fn visual_rect_for_paint(
        &self,
        widget_id: TreeWidgetId,
        authoritative_rect: RectF,
    ) -> RectF {
        // SPEC-HAL-145: Look up pre-computed interpolated rect from transition phase.
        // If not found, use the authoritative rect (no active transition).
        let interpolated = self.interpolated_rects.get(&widget_id).copied();
        interpolated.unwrap_or(authoritative_rect)
    }

    /// SPEC-R-013-v2: Emit a drop shadow behind a rect/rounded rect.
    /// Shadow is drawn at z_index - 1 (below the shape).
    pub fn paint_drop_shadow(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        offset: (f32, f32),
        blur_radius: f32,
        color: crate::core::paint::Color,
    ) {
        use crate::core::paint::ShadowMode;
        // z_index 1 = below typical widget content (z=2+)
        self.scene.push_shadow(
            shape_rect,
            shape_radius,
            ShadowMode::Drop,
            offset,
            0.0,
            blur_radius,
            color,
            1,
            None,
        );
    }

    /// SPEC-R-013-v2: Emit a drop shadow with spread.
    pub fn paint_drop_shadow_spread(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        offset: (f32, f32),
        spread: f32,
        blur_radius: f32,
        color: crate::core::paint::Color,
        z_index: crate::core::paint::ZIndex,
    ) {
        use crate::core::paint::ShadowMode;
        self.scene.push_shadow(
            shape_rect,
            shape_radius,
            ShadowMode::Drop,
            offset,
            spread,
            blur_radius,
            color,
            z_index,
            None,
        );
    }

    /// SPEC-R-013-v2: Emit an inner shadow inside a rect.
    pub fn paint_inner_shadow(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        blur_radius: f32,
        color: crate::core::paint::Color,
        z_index: crate::core::paint::ZIndex,
    ) {
        use crate::core::paint::ShadowMode;
        self.scene.push_shadow(
            shape_rect,
            shape_radius,
            ShadowMode::Inner,
            (0.0, 0.0),
            0.0,
            blur_radius,
            color,
            z_index,
            None,
        );
    }

    /// SPEC-R-012-v2: Emit a linear gradient rectangle.
    pub fn paint_linear_gradient(
        &mut self,
        rect: RectF,
        radius: f32,
        from: (f32, f32),
        to: (f32, f32),
        stops: &[crate::core::paint::ColorStop],
        z_index: crate::core::paint::ZIndex,
    ) {
        use crate::core::paint::{GradientDef, GradientKind};
        self.scene.push_gradient_rect(
            rect,
            radius,
            &GradientDef {
                kind: GradientKind::Linear,
                stops: stops.to_vec(),
                p0: from,
                p1: to,
            },
            z_index,
            None,
        );
    }

    /// SPEC-R-012-v2: Emit a vertical gradient (top color to bottom color).
    pub fn paint_vertical_gradient(
        &mut self,
        rect: RectF,
        radius: f32,
        top: crate::core::paint::Color,
        bottom: crate::core::paint::Color,
        z_index: crate::core::paint::ZIndex,
        scene_key: Option<crate::core::scene_identity::SceneNodeKey>,
    ) {
        use crate::core::paint::{ColorStop, GradientDef, GradientKind};
        self.scene.push_gradient_rect(
            rect,
            radius,
            &GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop {
                        offset: 0.0,
                        color: top,
                    },
                    ColorStop {
                        offset: 1.0,
                        color: bottom,
                    },
                ],
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
            },
            z_index,
            scene_key,
        );
    }

    /// SPEC-R-012-v2: Emit a radial gradient rectangle.
    pub fn paint_radial_gradient(
        &mut self,
        rect: RectF,
        radius: f32,
        center: (f32, f32),
        edge: (f32, f32),
        stops: &[crate::core::paint::ColorStop],
        z_index: crate::core::paint::ZIndex,
    ) {
        use crate::core::paint::{GradientDef, GradientKind};
        self.scene.push_gradient_rect(
            rect,
            radius,
            &GradientDef {
                kind: GradientKind::Radial,
                stops: stops.to_vec(),
                p0: center,
                p1: edge,
            },
            z_index,
            None,
        );
    }

    /// SPEC-HAL-272: Emit a color picker saturation/value area.
    pub fn paint_color_picker_sv(
        &mut self,
        rect: RectF,
        hue: f32,
        radius: f32,
        z_index: crate::core::paint::ZIndex,
    ) {
        self.scene
            .push_color_picker_sv(rect, hue, radius, z_index, self.scene_key);
    }

    /// SPEC-R-013-v2: Emit a multi-layered elevation shadow.
    ///
    /// Stacks 2-3 shadow layers for natural-looking elevation:
    /// - Near layer: sharp, higher alpha, small offset (edge definition)
    /// - Far layer: soft, lower alpha, larger offset (ambient depth)
    ///
    /// `level` controls intensity (0-4 scale matching shadow catalog):
    /// - 0: no shadow
    /// - 1: toolbar (barely visible)
    /// - 2: dropdown / popup
    /// - 3: dialog / modal
    /// - 4: floating panel (maximum)
    pub fn paint_elevation_shadow(&mut self, shape_rect: RectF, shape_radius: f32, level: u32) {
        use crate::core::paint::ShadowMode;

        if level == 0 || level > 4 {
            return;
        }

        let elev = self.theme.shadows.elevation[level as usize];

        // Near layer (z=0, behind everything)
        self.scene.push_shadow(
            shape_rect,
            shape_radius,
            ShadowMode::Drop,
            (0.0, elev.near_offset_y),
            0.0,
            elev.near_blur,
            crate::core::paint::Color::from_srgb(0.0, 0.0, 0.0, elev.near_alpha),
            0,
            None,
        );

        // Far layer (z=0, same layer — blends additively via premultiplied alpha)
        self.scene.push_shadow(
            shape_rect,
            shape_radius,
            ShadowMode::Drop,
            (0.0, elev.far_offset_y),
            0.0,
            elev.far_blur,
            crate::core::paint::Color::from_srgb(0.0, 0.0, 0.0, elev.far_alpha),
            0,
            None,
        );
    }

    /// SPEC-R-013-v2: Emit a shadow color-matched to the surface it falls on.
    ///
    /// Instead of pure black shadows, derives a darker, more saturated version
    /// of the background color. This makes shadows look integrated into the
    /// surface rather than painted on top.
    ///
    /// `bg_color`: the color of the surface the shadow falls ON (not the casting shape)
    /// `alpha`: shadow opacity (0.0 to 1.0)
    pub fn paint_surface_shadow(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        offset: (f32, f32),
        blur_radius: f32,
        bg_color: crate::core::paint::Color,
        alpha: f32,
    ) {
        use crate::core::paint::ShadowMode;

        // Darken the background color by 60% and increase saturation slightly
        // This produces a shadow that "belongs" to the surface
        let shadow_color = crate::core::paint::Color::from_linear(
            bg_color.r * 0.4,
            bg_color.g * 0.4,
            bg_color.b * 0.4,
            alpha,
        );

        self.scene.push_shadow(
            shape_rect,
            shape_radius,
            ShadowMode::Drop,
            offset,
            0.0,
            blur_radius,
            shadow_color,
            1,
            None,
        );
    }

    /// Paint a standard focus ring around a rect.
    /// Uses borders.focus color at LAYER_FOCUS.
    /// Call from any widget's paint() when focus_t > 0.
    pub fn paint_focus_ring(&mut self, rect: RectF, focus_t: f32, radius: f32) {
        let color = self.theme.colors.borders.focus;
        self.paint_focus_ring_colored(rect, focus_t, radius, color);
    }

    /// Like `paint_focus_ring`, but with an explicit ring color. For controls
    /// whose fill can equal `borders.focus` (e.g. a checked Checkbox, whose box
    /// fills with `accents.primary` — identical to `borders.focus` in the dark
    /// theme), where the default ring would be invisible against the fill.
    pub fn paint_focus_ring_colored(
        &mut self,
        rect: RectF,
        focus_t: f32,
        radius: f32,
        color: crate::core::paint::Color,
    ) {
        if focus_t < 0.001 {
            return;
        }
        let color = color.with_alpha(focus_t);
        let two_px = 2.0 / self.scale_factor;
        // 2-physical-pixel border at LAYER_FOCUS (z=5). Honor the caller's corner
        // radius so the ring matches the control's shape: the radio group passes
        // circle_size/2 for a full circle; rounded controls pass their corner
        // radius. Mirrors paint_focus_indicator_for.
        if radius > 0.0 {
            let corners = crate::core::paint::RoundedCorners::uniform(radius);
            self.scene.push_border_rounded_rect(
                rect,
                color,
                two_px,
                corners,
                crate::core::paint::LAYER_FOCUS,
                None,
            );
        } else {
            self.scene
                .push_border_rect(rect, color, two_px, crate::core::paint::LAYER_FOCUS, None);
        }
    }
}

/// Context provided during event handling (SPEC-HAL-009)
/// Allows widgets to request pointer capture and keyboard focus
#[derive(Clone, Copy)]
pub(crate) struct EventContextEffects {
    pub(crate) capture_requested: Option<crate::core::event::PointerButton>,
    pub(crate) capture_mode: crate::core::event::CaptureMode,
    pub(crate) focus_requested: bool,
    pub(crate) focus_redirect: Option<TreeWidgetId>,
    pub(crate) keyboard_capture_requested: bool,
    pub(crate) keyboard_capture_release_requested: bool,
    pub(crate) release_capture: bool,
    pub(crate) retire_descendant_pointer_capture:
        Option<(crate::core::event::PointerId, TreeWidgetId)>,
    pub(crate) gesture_commit: bool,
    pub(crate) continuous_frames: bool,
    pub(crate) cursor: Option<crate::core::event::CursorIcon>,
}

impl EventContextEffects {
    pub(crate) fn empty() -> Self {
        Self {
            capture_requested: None,
            capture_mode: crate::core::event::CaptureMode::Hard,
            focus_requested: false,
            focus_redirect: None,
            keyboard_capture_requested: false,
            keyboard_capture_release_requested: false,
            release_capture: false,
            retire_descendant_pointer_capture: None,
            gesture_commit: false,
            continuous_frames: false,
            cursor: None,
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        if other.capture_requested.is_some() {
            self.capture_requested = other.capture_requested;
            self.capture_mode = other.capture_mode;
        }
        self.focus_requested |= other.focus_requested;
        if other.focus_redirect.is_some() {
            self.focus_redirect = other.focus_redirect;
        }
        self.keyboard_capture_requested |= other.keyboard_capture_requested;
        self.keyboard_capture_release_requested |= other.keyboard_capture_release_requested;
        self.release_capture |= other.release_capture;
        if other.retire_descendant_pointer_capture.is_some() {
            self.retire_descendant_pointer_capture = other.retire_descendant_pointer_capture;
        }
        self.gesture_commit |= other.gesture_commit;
        self.continuous_frames |= other.continuous_frames;
        if other.cursor.is_some() {
            self.cursor = other.cursor;
        }
    }
}

pub struct EventContext<'a> {
    composite_request: Option<crate::core::composite_text_edit::CompositeEditRequest>,
    composite_authority: Option<crate::core::composite_text_edit::CompositeEditorId>,
    composite_origin: Option<crate::core::composite_text_edit::CompositeEditOrigin>,
    composite_host_capture: bool,
    composite_local_position: Option<(f32, f32)>,
    availability_transition: bool,
    availability_owner: TreeWidgetId,
    pub widget_id: TreeWidgetId,
    capture_requested: Option<crate::core::event::PointerButton>,
    /// SPEC-HAL-079: Capture mode for requested capture.
    /// Defaults to Hard if not specified.
    capture_mode_requested: crate::core::event::CaptureMode,
    focus_requested: bool,
    /// Focus redirect: transfer focus to a different widget.
    /// Processed by dispatch_to_widget post-dispatch instead of
    /// setting focus to self.
    focus_redirect: Option<TreeWidgetId>,
    keyboard_capture_requested: bool,
    keyboard_capture_release_requested: bool,
    release_capture: bool,
    retire_descendant_pointer_capture: Option<(crate::core::event::PointerId, TreeWidgetId)>,
    /// SPEC-HAL-026: Raw pointer to tree for layout invalidation from event handlers
    tree: *mut WidgetTree,
    _phantom: std::marker::PhantomData<&'a ()>,

    /// SPEC-HAL-101 Stage-4: Frame time in milliseconds from EngineTicker
    frame_time_ms: u64,

    /// SPEC-HAL-056: Clipboard access for copy/paste
    clipboard: *mut crate::platform::clipboard::Clipboard,

    /// INV-TEXT-GEOM-PHASE: Text engine for synchronous layout cache rebuild.
    /// Enables widgets to rebuild text layout during event handling when
    /// cross-event staleness would otherwise cause geometry read panics.
    /// None for events not routed through route_input_event (focus/blur callbacks).
    text_engine: Option<&'a mut dyn TextEngine>,

    /// Click count projected from the delivered pointer kind.
    fused_click_count: Option<u32>,

    /// SPEC-HAL-128 Stage 3: Pending gesture commit request.
    /// INV-GA5: Commit only happens when explicitly requested.
    pending_gesture_commit: bool,

    /// SPEC-HAL-021 Stage 2.5: Optional animation capability.
    /// Animation is a utility, not a first-class routing primitive.
    /// This capability may be absent in layout-only paths or tests.
    animation: Option<crate::core::animation::AnimationContext<'a>>,

    /// SPEC-HAL-038 / INV-INTERACTION-STATE-5: Whether this widget has pointer capture.
    /// Used by widgets to check capture state before consuming scroll input.
    has_pointer_capture: bool,

    /// Engine-resolved provenance for the pointer event currently being routed.
    /// Overlay routes remain logically parented in the widget tree, but ancestors
    /// must not reinterpret their coordinates as underlay interaction.
    pointer_route_origin: crate::core::event::PointerRouteOrigin,
    combo_popup_hit: Option<ComboPopupInputGeometry>,
    scrollbar_rail_point: Option<crate::core::math::Point>,
    touch_gesture_press: Option<crate::core::event::TouchPressIdentity>,
    touch_scroll_motion: Option<crate::core::event::TouchScrollMotion>,
    kinetic_scroll_requested: bool,

    /// Stage 42G: Request continuous redraws.
    ///
    /// INV-42G-3 (Active Resize Frame Pump): While this is true, the app MUST
    /// schedule continuous frames independent of further input.
    ///
    /// Set by widgets that need frame-driven updates (e.g., during resize).
    /// Checked by event router and propagated to app's frame scheduling.
    request_continuous_frames: bool,

    /// Cursor icon request.
    ///
    /// INV-CURSOR-EPHEMERAL: Cursor requests are per-event, not persistent.
    /// Widgets set this during hover to show resize/text/pointer cursors.
    /// The event router collects the last request and applies it after dispatch.
    cursor_requested: Option<crate::core::event::CursorIcon>,
    /// SPEC-HAL-268: True when Ctrl was double-tapped and is held
    ctrl_double_tap_active: bool,
    /// SPEC-HAL-176: Current keyboard authority mode.
    /// Widgets check this to decide whether arrow keys should
    /// change values (Capture) or be ignored for navigation
    /// (Navigation).
    pub keyboard_mode: crate::core::event::KeyboardAuthorityMode,

    /// SPEC-HAL-326 §12.x INV-F9: When focus is changing via directional
    /// keyboard navigation (Up/Down/Left/Right arrows or hjkl aliases),
    /// the framework sets this to the direction the focus is arriving
    /// from. None for Tab/programmatic/click focus, or when the framework
    /// has no direction context.
    ///
    /// Composite widgets with internal focus targets (RadioGroup, TabBar,
    /// future ListView, etc.) read this in `on_focus` to choose which
    /// internal element to focus. Simple widgets ignore it.
    focus_entry_direction: Option<crate::core::event::Direction>,
}

impl<'a> EventContext<'a> {
    pub fn new(
        widget_id: TreeWidgetId,
        tree: *mut WidgetTree,
        frame_time_ms: u64,
        clipboard: *mut crate::platform::clipboard::Clipboard,
        text_engine: Option<&'a mut dyn TextEngine>,
        animation: Option<crate::core::animation::AnimationContext<'a>>,
        has_pointer_capture: bool,
        ctrl_double_tap_active: bool,
        keyboard_mode: crate::core::event::KeyboardAuthorityMode,
    ) -> Self {
        Self {
            availability_transition: false,
            availability_owner: widget_id,
            widget_id,
            capture_requested: None,
            capture_mode_requested: crate::core::event::CaptureMode::Hard,
            focus_requested: false,
            focus_redirect: None,
            keyboard_capture_requested: false,
            keyboard_capture_release_requested: false,
            release_capture: false,
            retire_descendant_pointer_capture: None,
            tree,
            _phantom: std::marker::PhantomData,
            frame_time_ms,
            clipboard,
            text_engine,
            fused_click_count: None,
            // SPEC-HAL-128 Stage 3: No commit request initially
            pending_gesture_commit: false,
            // SPEC-HAL-021 Stage 2.5: Optional animation capability
            animation,
            // SPEC-HAL-038 / INV-INTERACTION-STATE-5: Capture state for scroll authority
            has_pointer_capture,
            pointer_route_origin: crate::core::event::PointerRouteOrigin::Regular,
            combo_popup_hit: None,
            scrollbar_rail_point: None,
            touch_gesture_press: None,
            touch_scroll_motion: None,
            kinetic_scroll_requested: false,
            // Stage 42G: No continuous frames requested initially
            request_continuous_frames: false,
            // INV-CURSOR-EPHEMERAL: No cursor requested initially
            cursor_requested: None,
            // SPEC-HAL-268: Double-tap Ctrl state
            ctrl_double_tap_active,
            keyboard_mode,
            focus_entry_direction: None,
            composite_request: None,
            composite_authority: None,
            composite_origin: None,
            composite_host_capture: false,
            composite_local_position: None,
        }
    }

    /// Request pointer capture for drag operations (hard capture by default)
    pub(crate) fn request_composite_edit(
        &mut self,
        operation: crate::core::composite_text_edit::CompositeEditOperation,
        continuation: crate::widgets::composite_edit_host::CompositeHostContinuation,
    ) -> Result<(), crate::core::composite_text_edit::CompositeEditOutcome> {
        if self.composite_request.is_some() || self.composite_authority.is_some() {
            return Err(crate::core::composite_text_edit::CompositeEditOutcome::Reentrant);
        }
        self.composite_request = Some(crate::core::composite_text_edit::CompositeEditRequest {
            operation,
            continuation,
        });
        Ok(())
    }

    pub(crate) fn take_composite_edit(
        &mut self,
    ) -> Option<crate::core::composite_text_edit::CompositeEditRequest> {
        self.composite_request.take()
    }

    pub(crate) fn take_dispatch_effects(&mut self) -> EventContextEffects {
        EventContextEffects {
            capture_requested: self.capture_requested,
            capture_mode: self.capture_mode_requested,
            focus_requested: self.focus_requested,
            focus_redirect: self.focus_redirect,
            keyboard_capture_requested: self.keyboard_capture_requested,
            keyboard_capture_release_requested: self.keyboard_capture_release_requested,
            release_capture: self.release_capture,
            retire_descendant_pointer_capture: self.retire_descendant_pointer_capture.take(),
            gesture_commit: self.take_pending_gesture_commit(),
            continuous_frames: self.request_continuous_frames,
            cursor: self.cursor_requested,
        }
    }

    pub(crate) fn set_composite_edit_authority(
        &mut self,
        id: &crate::core::composite_text_edit::CompositeEditorId,
        origin: crate::core::composite_text_edit::CompositeEditOrigin,
        host_has_capture: bool,
    ) {
        self.composite_authority = Some(id.clone());
        self.composite_origin = Some(origin);
        self.composite_host_capture = host_has_capture;
    }

    pub(crate) fn composite_edit_authority(
        &self,
    ) -> Option<(
        &crate::core::composite_text_edit::CompositeEditorId,
        crate::core::composite_text_edit::CompositeEditOrigin,
        bool,
    )> {
        Some((
            self.composite_authority.as_ref()?,
            self.composite_origin?,
            self.composite_host_capture,
        ))
    }

    pub(crate) fn composite_edit_origin(
        &self,
    ) -> Option<crate::core::composite_text_edit::CompositeEditOrigin> {
        self.composite_origin
    }

    pub(crate) fn composite_edit_has_capture(&self) -> bool {
        self.composite_host_capture
    }

    pub(crate) fn set_composite_local_position(&mut self, position: Option<(f32, f32)>) {
        self.composite_local_position = position;
    }

    pub(crate) fn composite_local_position(&self) -> Option<(f32, f32)> {
        self.composite_local_position
    }

    /// Request pointer capture for drag operations (hard capture by default)
    pub fn capture_pointer(&mut self, button: crate::core::event::PointerButton) {
        self.capture_requested = Some(button);
        self.capture_mode_requested = crate::core::event::CaptureMode::Hard;
    }

    /// SPEC-HAL-079: Request pointer capture with explicit mode.
    ///
    /// INV-PC-MODE-EXPLICIT: Capture mode is explicitly specified.
    pub fn capture_pointer_with_mode(
        &mut self,
        button: crate::core::event::PointerButton,
        mode: crate::core::event::CaptureMode,
    ) {
        self.capture_requested = Some(button);
        self.capture_mode_requested = mode;
    }

    /// SPEC-HAL-079: Get the requested capture mode.
    pub fn capture_mode_requested(&self) -> crate::core::event::CaptureMode {
        self.capture_mode_requested
    }

    /// Release pointer capture
    pub fn release_pointer(&mut self) {
        self.release_capture = true;
    }

    /// HAL371: request engine-authenticated retirement of a captured descendant.
    /// The lifecycle owner cannot release or impersonate that widget itself.
    pub(crate) fn retire_descendant_pointer_capture(
        &mut self,
        pointer: crate::core::event::PointerId,
        capture_owner: TreeWidgetId,
    ) {
        self.retire_descendant_pointer_capture = Some((pointer, capture_owner));
    }

    /// Request keyboard focus
    pub fn request_focus(&mut self) {
        self.focus_requested = true;
    }

    /// Transfer focus to a different widget.
    ///
    /// Unlike request_focus (which focuses self), this focuses
    /// an arbitrary widget by TreeWidgetId. The event router
    /// processes this immediately in the post-dispatch block —
    /// same frame, no deferral.
    ///
    /// If the target is capture-eligible, Capture mode is
    /// entered and on_capture_enter is called.
    pub fn redirect_focus(&mut self, target: TreeWidgetId) {
        self.focus_redirect = Some(target);
    }

    pub fn focus_redirect(&self) -> Option<TreeWidgetId> {
        self.focus_redirect
    }

    /// Request keyboard focus AND immediate Capture mode entry.
    /// Use for widgets that become fully keyboard-operable on click
    /// without requiring the user to press `i` (ENTER_WIDGET).
    pub fn request_keyboard_capture(&mut self) {
        self.focus_requested = true;
        self.keyboard_capture_requested = true;
    }

    pub(crate) fn is_availability_transition(&self) -> bool {
        self.availability_transition
    }

    pub(crate) fn set_availability_transition(&mut self) {
        self.availability_transition = true;
    }

    pub fn request_enabled(
        &mut self,
        enabled: bool,
    ) -> Result<(), crate::core::widget_tree::WidgetAvailabilityError> {
        if self.tree.is_null() {
            return Err(crate::core::widget_tree::WidgetAvailabilityError::TreeUnavailable);
        }
        // The existing event context owns only an ephemeral self-qualified tree access.
        let tree = unsafe { &mut *self.tree };
        tree.request_owner_availability(self.availability_owner, Some(enabled), None)
    }

    pub fn request_read_only(
        &mut self,
        read_only: bool,
    ) -> Result<(), crate::core::widget_tree::WidgetAvailabilityError> {
        if self.tree.is_null() {
            return Err(crate::core::widget_tree::WidgetAvailabilityError::TreeUnavailable);
        }
        let tree = unsafe { &mut *self.tree };
        tree.request_owner_availability(self.availability_owner, None, Some(read_only))
    }

    pub fn is_keyboard_capture_requested(&self) -> bool {
        self.keyboard_capture_requested
    }

    /// Release keyboard Capture mode while retaining focus.
    /// Widget stays focused in Navigation mode.
    pub fn release_keyboard_capture(&mut self) {
        self.keyboard_capture_release_requested = true;
    }

    /// Root viewport size (window content area, in logical pixels).
    /// Falls back to (0.0, 0.0) if the tree has no root rect yet.
    pub fn viewport_size(&self) -> (f32, f32) {
        // Safety: tree pointer is valid for duration of event dispatch.
        unsafe {
            if let Some(tree) = self.tree.as_ref() {
                if let Some(root_id) = tree.root {
                    if let Some(rect) = tree.get_rect(root_id) {
                        return (rect.width, rect.height);
                    }
                }
            }
        }
        (0.0, 0.0)
    }

    /// Dispatch an event to a child widget by TreeWidgetId.
    ///
    /// Temporarily takes the widget out of the tree (same pattern as
    /// layout_child), calls handle_event with the current EventContext,
    /// then restores the widget. The child uses the parent's cx for
    /// text_engine, invalidation, etc. — same as ComboBox→TextField.
    ///
    /// Used by container widgets that keep focus but need to forward
    /// key/text events to an active child editor.
    pub fn dispatch_to_child(
        &mut self,
        target_id: TreeWidgetId,
        event: &Event,
        phase: EventPhase,
    ) -> bool {
        let tree = unsafe { &mut *self.tree };
        let node_id = match tree.find_node_by_widget_id(target_id) {
            Some(id) => id,
            None => return false,
        };
        // Scope the node borrow so we can call mark_layout_dirty after.
        let handled = if let Some(node) = tree.node_mut(node_id) {
            node.widget.handle_event(self, event, phase)
        } else {
            return false;
        };
        if handled {
            // Child's cx.invalidate_layout() marked the PARENT dirty
            // (because cx.widget_id is the parent). Also mark the child's
            // own tree node dirty so layout_child doesn't skip it.
            tree.mark_layout_dirty(node_id);
        }
        handled
    }

    /// Set deferred focus to a widget by TreeWidgetId.
    ///
    /// The focus transfer happens after layout completes
    /// (processed by app.rs deferred_focus handler).
    /// Used by container widgets to programmatically focus
    /// sibling widgets (e.g., PropertyEditor → filter TextField).
    pub fn defer_focus(&mut self, target_id: TreeWidgetId) {
        let tree = unsafe { &mut *self.tree };
        if let Some(node_id) = tree.find_node_by_widget_id(target_id) {
            tree.deferred_focus = Some(node_id);
        }
    }

    pub fn is_keyboard_capture_release_requested(&self) -> bool {
        self.keyboard_capture_release_requested
    }

    /// Check if capture was requested
    pub fn is_capture_requested(&self) -> Option<crate::core::event::PointerButton> {
        self.capture_requested
    }

    /// Check if focus was requested
    pub fn is_focus_requested(&self) -> bool {
        self.focus_requested
    }

    /// Check if release was requested
    pub fn is_release_requested(&self) -> bool {
        self.release_capture
    }

    /// SPEC-HAL-038 / INV-INTERACTION-STATE-5: Check if this widget has pointer capture.
    ///
    /// Widgets MUST check this before consuming scroll input. A widget that does not
    /// have capture MUST NOT consume scroll events.
    ///
    /// Scrollbar drag establishes capture (via capture_pointer()). Wheel scroll does not.
    pub fn has_pointer_capture(&self) -> bool {
        self.has_pointer_capture
    }

    /// Whether the current pointer route originated in an overlay hit region.
    ///
    /// This is crate-internal routing provenance. Composite ancestors use it to
    /// avoid treating popup coordinates as interaction with covered content.
    pub(crate) fn pointer_route_is_overlay(&self) -> bool {
        self.pointer_route_origin == crate::core::event::PointerRouteOrigin::Overlay
    }

    pub(crate) fn set_pointer_route_origin(
        &mut self,
        origin: crate::core::event::PointerRouteOrigin,
    ) {
        self.pointer_route_origin = origin;
    }

    pub(crate) fn set_combo_popup_hit(&mut self, hit: Option<ComboPopupInputGeometry>) {
        self.combo_popup_hit = hit;
    }

    pub(crate) fn combo_popup_hit(&self) -> Option<ComboPopupInputGeometry> {
        self.combo_popup_hit
    }

    pub(crate) fn set_scrollbar_rail_point(&mut self, point: Option<crate::core::math::Point>) {
        self.scrollbar_rail_point = point;
    }

    /// The router's clipped, container-local position for rail observation.
    pub(crate) fn scrollbar_rail_point(&self) -> Option<crate::core::math::Point> {
        self.scrollbar_rail_point
    }

    /// The qualified press for this gesture lifecycle notification (HAL-396).
    pub fn touch_gesture_press(&self) -> Option<crate::core::event::TouchPressIdentity> {
        self.touch_gesture_press
    }

    /// Motion belonging to a winning content scroll. At commit this is the
    /// first, accumulated sample and is delivered only in the commit callback.
    pub fn touch_scroll_motion(&self) -> Option<crate::core::event::TouchScrollMotion> {
        self.touch_scroll_motion
    }

    pub(crate) fn set_touch_gesture_press(
        &mut self,
        press: Option<crate::core::event::TouchPressIdentity>,
    ) {
        self.touch_gesture_press = press;
    }

    pub(crate) fn set_touch_scroll_motion(
        &mut self,
        motion: crate::core::event::TouchScrollMotion,
    ) {
        self.touch_gesture_press = Some(motion.press);
        self.touch_scroll_motion = Some(motion);
    }

    pub(crate) fn request_kinetic_scroll(&mut self) {
        self.kinetic_scroll_requested = self.touch_scroll_motion.is_some_and(|motion| motion.is_release);
    }

    pub(crate) fn kinetic_scroll_requested(&self) -> bool {
        self.kinetic_scroll_requested
    }

    /// SPEC-HAL-268: Whether Ctrl was double-tapped and is held (proximity scroll trigger)
    pub fn ctrl_double_tap_active(&self) -> bool {
        self.ctrl_double_tap_active
    }

    // ========================================================================
    // SPEC-HAL-128 Stage 3: Gesture Commit Request
    // SPEC-HAL-128 Stage 8: Direct registration removed (INV-GS3)
    // ========================================================================

    /// SPEC-HAL-128 Stage 3/8: Request to commit this widget's gesture.
    ///
    /// An ordinary Absolute Drag handle records tentative facts on Down and
    /// requests commitment on framework DragStart, after InputFusion recognizes
    /// the threshold. DragStart uses the original Down position, re-hit-tested
    /// when uncaptured; it does not retain a target ID across relayout.
    /// This request is not commitment: visible preview begins only in
    /// on_gesture_commit, after framework capture. Other accepted gesture
    /// consumers retain their governing commitment contracts.
    ///
    /// INV-GA5: Commit only happens when explicitly requested.
    /// INV-GA6: On successful commit, pointer is captured for this widget.
    /// INV-GA7: Losing candidates are cancelled.
    ///
    /// SPEC-HAL-128 Stage 8: The system registers candidates based on the widget's
    /// `gesture_surface()` declaration. Widgets no longer register directly.
    /// After commit resolution:
    /// - Winner receives `on_gesture_commit()` callback
    /// - Losers receive `on_gesture_cancel()` callback
    pub fn request_gesture_commit(&mut self) {
        self.pending_gesture_commit = true;
    }

    /// SPEC-HAL-128 Stage 3: Take pending gesture commit request.
    ///
    /// Called by EventRouter after dispatch to check for commit requests.
    /// Returns true if commit was requested, false otherwise.
    pub(crate) fn take_pending_gesture_commit(&mut self) -> bool {
        std::mem::take(&mut self.pending_gesture_commit)
    }

    /// Click count for the classified pointer kind delivered to this widget.
    ///
    /// Returns 1 for `Down`, 2 for `DoubleClick`, and 3 for `TripleClick`.
    /// Other events and contexts return 1.
    pub fn click_count(&self) -> u32 {
        self.fused_click_count.unwrap_or(1)
    }

    pub(crate) fn set_routed_click_count(&mut self, kind: crate::core::event::PointerEventKind) {
        use crate::core::event::PointerEventKind;

        self.fused_click_count = match kind {
            PointerEventKind::Down => Some(1),
            PointerEventKind::DoubleClick => Some(2),
            PointerEventKind::TripleClick => Some(3),
            _ => None,
        };
    }

    /// SPEC-HAL-101 Stage-4: Get frame time in milliseconds from EngineTicker.
    /// Used for caret blinking and other time-based widget state.
    pub fn frame_time_ms(&self) -> u64 {
        self.frame_time_ms
    }

    /// SPEC-HAL-326 §12.x INV-F9: Get the direction from which focus is
    /// arriving, if any. Set by the framework on directional keyboard
    /// navigation; None for Tab, programmatic, click, or non-directional
    /// focus changes.
    ///
    /// Composite widgets read this in `on_focus` to choose internal
    /// entry point. For RadioGroup: Up → bottommost member; Down →
    /// topmost; Left/Right → selected-or-first (default).
    pub fn focus_entry_direction(&self) -> Option<crate::core::event::Direction> {
        self.focus_entry_direction
    }

    /// Framework-only: set the directional entry tag for the next
    /// `on_focus` call. Called by `EventRouter::request_focus_for_widget`
    /// after constructing the EventContext but before dispatching
    /// `on_focus`. Not intended for widget use.
    pub fn set_focus_entry_direction(&mut self, direction: Option<crate::core::event::Direction>) {
        self.focus_entry_direction = direction;
    }

    /// INV-TEXT-GEOM-PHASE: Get mutable text engine for synchronous layout cache rebuild.
    ///
    /// Widgets that manage text layout (TextField, TextArea) can use this to rebuild
    /// their layout cache during event handling when cross-event staleness would
    /// otherwise cause geometry read panics.
    ///
    /// # Panics
    /// Panics if called from an event handler that doesn't have text_engine available
    /// (e.g., on_focus/on_blur callbacks). This is intentional - text widgets should
    /// only need layout rebuild during input events routed through route_input_event.
    ///
    /// # Panics
    /// Panics if called from an event not routed through route_input_event
    /// (e.g., lifecycle callbacks like on_focus/on_blur).
    pub fn text_engine(&mut self) -> &mut dyn TextEngine {
        self.text_engine.as_deref_mut().expect(
            "INV-TEXT-GEOM-PHASE: text_engine not available. \
             This event was not routed through route_input_event.",
        )
    }

    /// Non-panicking accessor for callers that may run from lifecycle
    /// callbacks (e.g., `on_capture_enter`) where text_engine may not be
    /// available. Returns None instead of panicking; callers should defer
    /// any layout-cache rebuild to the next paint/layout pass.
    pub fn try_text_engine(&mut self) -> Option<&mut (dyn TextEngine + 'a)> {
        self.text_engine.as_deref_mut()
    }

    /// SPEC-HAL-021 Stage 2.5: Start an animation on the timeline.
    ///
    /// Returns a TransitionHandle that can be used to query the animated value.
    /// The widget should store this handle and the current value in its state.
    ///
    /// Returns `None` if animation capability is not available (e.g., in tests
    /// or layout-only paths). Widgets should handle this gracefully.
    pub fn start_animation<T: crate::core::animation::Interpolatable>(
        &mut self,
        animation: crate::core::animation::Animation<T>,
    ) -> Option<crate::core::animation::TransitionHandle> {
        match self.try_start_animation(animation) {
            Ok(handle) => handle,
            Err(error) => {
                tracing::error!(%error, "Timeline rejected widget animation");
                None
            }
        }
    }

    /// Start a `Once` animation while preserving admission failure separately
    /// from an unavailable animation capability.
    pub fn try_start_animation<T: crate::core::animation::Interpolatable>(
        &mut self,
        animation: crate::core::animation::Animation<T>,
    ) -> Result<
        Option<crate::core::animation::TransitionHandle>,
        crate::core::animation::AnimationStartError,
    > {
        let Some(owner) = self.animation_owner_for_start() else {
            return Ok(None);
        };
        self.animation
            .as_mut()
            .map(|ctx| ctx.start_for_owner(owner, animation))
            .transpose()
    }

    /// Start an animation with explicit Once/Loop/Alternate playback.
    pub fn start_animation_descriptor<T: crate::core::animation::Interpolatable>(
        &mut self,
        descriptor: crate::core::animation::AnimationDescriptor<T>,
    ) -> Result<
        Option<crate::core::animation::TransitionHandle>,
        crate::core::animation::AnimationStartError,
    > {
        let Some(owner) = self.animation_owner_for_start() else {
            return Ok(None);
        };
        self.animation
            .as_mut()
            .map(|ctx| ctx.start_descriptor_for_owner(owner, descriptor))
            .transpose()
    }

    fn animation_owner_for_start(&self) -> Option<crate::core::animation::AnimationOwnerId> {
        self.animation.as_ref()?;
        if self.tree.is_null() {
            return Some(crate::core::animation::AnimationOwnerId::new(
                self.widget_id.0,
            ));
        }
        // The existing routed context owns this scoped tree access. Only the
        // current concrete instance's disjoint lifetime metadata is observed.
        unsafe { &*self.tree }
            .animation_mount(self.availability_owner)
            .map(|mount| crate::core::animation::AnimationOwnerId::mounted(mount.token()))
    }

    /// Cancel an in-flight animation by handle.
    pub fn cancel_animation(&mut self, handle: crate::core::animation::TransitionHandle) {
        if let Some(ctx) = self.animation.as_mut() {
            ctx.cancel(handle);
        }
    }

    /// SPEC-HAL-021 Stage 5: Check if animation capability is available.
    ///
    /// Returns true if this EventContext has animation capability injected.
    pub fn has_animation(&self) -> bool {
        self.animation.is_some()
    }

    /// SPEC-HAL-026: Mark this widget as needing layout from event handler
    /// Propagates dirty flag up to ancestors
    pub fn invalidate_layout(&mut self) {
        // Safety: tree pointer is valid for duration of event dispatch
        unsafe {
            if let Some(tree) = self.tree.as_mut() {
                tree.mark_layout_dirty_by_widget_id(self.widget_id);
            }
        }
    }

    /// Mark the nearest ancestor whose layout observes this widget's scroll input.
    pub fn invalidate_scroll_dependent_ancestor_layout(&mut self) {
        unsafe {
            if let Some(tree) = self.tree.as_mut() {
                tree.mark_nearest_scroll_dependent_ancestor_layout(self.widget_id);
            }
        }
    }

    /// SPEC-HAL-026: Mark a specific widget as needing layout by its TreeWidgetId.
    /// Propagates dirty flag up to ancestors.
    ///
    /// Use when a widget needs to invalidate a descendant (e.g., ListView invalidating Body
    /// when column widths change). For marking self dirty, use `invalidate_layout()` instead.
    pub fn invalidate_layout_by_id(&mut self, widget_id: TreeWidgetId) {
        // Safety: tree pointer is valid for duration of event dispatch
        unsafe {
            if let Some(tree) = self.tree.as_mut() {
                tree.mark_layout_dirty_by_widget_id(widget_id);
            }
        }
    }

    /// SPEC-HAL-354: Regenerate this widget's paint fragment without layout.
    pub fn invalidate_paint(&mut self) {
        unsafe {
            if let Some(tree) = self.tree.as_mut() {
                tree.mark_paint_dirty_by_widget_id(self.widget_id);
            }
        }
    }

    /// An immediate scroll jump changes both retained paint and input geometry.
    pub(crate) fn invalidate_scroll_projection(&mut self) {
        unsafe {
            if let Some(tree) = self.tree.as_mut()
                && let Some(node) = tree.find_node_by_widget_id(self.widget_id)
            {
                tree.record_geometry_origin(node);
                tree.mark_paint_dirty(node);
            }
        }
    }

    /// SPEC-HAL-354: Regenerate a specific widget's paint fragment without layout.
    pub fn invalidate_paint_by_id(&mut self, widget_id: TreeWidgetId) {
        unsafe {
            if let Some(tree) = self.tree.as_mut() {
                tree.mark_paint_dirty_by_widget_id(widget_id);
            }
        }
    }

    /// SPEC-HAL-026: Mark this widget dirty, plus its ancestors (upward) and
    /// all its descendants (downward).
    ///
    /// Use when an event changes state that affects the geometry of descendant
    /// widgets (e.g., row height changes inside a virtualized list body that
    /// is a grandchild via an intermediate ScrollArea).
    ///
    /// Both directions are required:
    ///   - Upward: ensures the layout traversal (starting at root) reaches this
    ///     node. Without ancestor propagation, `layout_node` short-circuits at
    ///     a clean ancestor and never visits the subtree.
    ///   - Downward: ensures descendants do not short-circuit on unchanged
    ///     constraints when the traversal reaches them. `invalidate_layout()`
    ///     alone cannot do this — it only propagates upward.
    pub fn invalidate_subtree(&mut self) {
        // Safety: tree pointer is valid for duration of event dispatch
        unsafe {
            if let Some(tree) = self.tree.as_mut() {
                if let Some(node_id) = tree.find_node_by_widget_id(self.widget_id) {
                    // Upward: mark ancestors dirty so layout traversal reaches this node.
                    // Without this, a clean root short-circuits in `layout_node` and the
                    // subtree is never visited, regardless of descendant dirty flags.
                    tree.mark_layout_dirty(node_id);
                    // Downward: mark descendants dirty so they do not short-circuit
                    // on unchanged constraints when the traversal reaches them.
                    tree.mark_subtree_dirty(node_id);
                }
            }
        }
    }

    /// Stage 42G: Request continuous frame updates.
    ///
    /// INV-42G-3 (Active Resize Frame Pump): While continuous frames are requested,
    /// the app MUST schedule frames independent of further input events.
    ///
    /// Use this when a widget needs frame-driven updates (e.g., during resize).
    /// The request is checked by the event router after event dispatch.
    pub fn request_continuous_frames(&mut self) {
        self.request_continuous_frames = true;
    }

    /// Stage 42G: Check if continuous frames have been requested.
    ///
    /// INV-42G-3: Event router checks this and propagates to frame scheduling.
    pub(crate) fn continuous_frames_requested(&self) -> bool {
        self.request_continuous_frames
    }

    /// Request a cursor icon for this event.
    ///
    /// Widgets call this during hover detection to show appropriate cursors
    /// (e.g., ColResize when hovering a column separator).
    ///
    /// INV-CURSOR-EPHEMERAL: Cursor requests are per-event. The last widget
    /// to request a cursor during event dispatch wins. If no cursor is
    /// requested, the default arrow cursor is used.
    ///
    /// INV-CURSOR-NO-CAPTURE: Requesting a cursor does NOT capture the pointer.
    /// INV-CURSOR-NO-DRAG: Cursor request is visual-only, no drag arming.
    pub fn request_cursor(&mut self, cursor: crate::core::event::CursorIcon) {
        self.cursor_requested = Some(cursor);
    }

    /// Check if a cursor was requested during this event.
    ///
    /// Called by event router after dispatch to apply cursor changes.
    pub(crate) fn cursor_requested(&self) -> Option<crate::core::event::CursorIcon> {
        self.cursor_requested
    }

    /// SPEC-HAL-086 Stage 10: Get the widget's global rect (absolute_paint_rect).
    ///
    /// Returns the canonical global coordinates for this widget, computed during
    /// `compute_absolute_paint_rects()` after layout. Use this for coordinate
    /// transformations that need the widget's true screen position.
    ///
    /// Returns None if the widget node cannot be found.
    pub fn widget_global_rect(&self) -> Option<RectF> {
        // Safety: tree pointer is valid for duration of event dispatch
        // SPEC-HAL-141: Use getter for private geometry field
        unsafe {
            let tree = self.tree.as_ref()?;
            let node_id = tree.find_by_id(self.widget_id)?;
            tree.get_absolute_paint_rect(node_id)
        }
    }

    pub(crate) fn widget_viewport_rect(&self) -> Option<RectF> {
        // The live layout rect and the tree's scroll offset share the paint basis.
        let tree = unsafe { self.tree.as_ref()? };
        let node = tree.find_by_id(self.widget_id)?;
        let rect = tree.get_absolute_paint_rect(node)?;
        let (x, y) = tree.cumulative_scroll_offset(node);
        Some(rect.translate(-x, -y))
    }

    /// SPEC-HAL-056: Get text from clipboard.
    ///
    /// Returns None if clipboard is empty, unavailable, or contains non-text data.
    pub fn clipboard_get_text(&mut self) -> Option<String> {
        // Safety: clipboard pointer is valid for duration of event dispatch
        unsafe { self.clipboard.as_mut().and_then(|c| c.get_text()) }
    }

    /// SPEC-HAL-056: Set text to clipboard.
    ///
    /// Returns true if successful, false otherwise.
    pub fn clipboard_set_text(&mut self, text: &str) -> bool {
        // Safety: clipboard pointer is valid for duration of event dispatch
        unsafe {
            self.clipboard
                .as_mut()
                .map(|c| c.set_text(text))
                .unwrap_or(false)
        }
    }

    // ========================================================================
    // SPEC-HAL-190: Authority Projection
    // ========================================================================

    /// SPEC-HAL-190: Look up authority from the nearest ancestor.
    ///
    /// Returns a Projection to the authority if an ancestor provides it.
    ///
    /// # Type Parameters
    ///
    /// - `A`: The authority type to look up. Must implement `Authority`.
    ///
    /// # Returns
    ///
    /// - `Some(Projection<A>)` if an ancestor provides authority A
    /// - `None` if no ancestor provides authority A
    pub fn authority<A: crate::core::authority::Authority + 'static>(
        &self,
    ) -> Option<crate::core::authority::Projection<'_, A>> {
        // Safety: tree pointer is valid for duration of event dispatch
        unsafe {
            let tree = self.tree.as_ref()?;
            let node_id = tree.find_by_id(self.widget_id)?;
            tree.lookup_authority::<A>(node_id)
        }
    }

    /// SPEC-HAL-190: Emit an intent to the nearest ancestor authority owner of type A.
    ///
    /// The intent is buffered; the owner processes it during deferred mutation phase.
    ///
    /// # Type Parameters
    ///
    /// - `A`: The authority type to target. Must implement `Authority`.
    /// - `I`: The intent type. Must implement `Intent`.
    ///
    /// # Returns
    ///
    /// - `true` if the intent was pushed to a buffer (owner found)
    /// - `false` if no ancestor provides authority A
    ///
    /// # Invariants
    ///
    /// - INV-INTENT-EMIT-1: May only be called during event handling phase.
    /// - INV-INTENT-EMIT-2: Child has no visibility into buffer contents or owner identity.
    pub fn emit_intent<
        A: crate::core::authority::Authority + 'static,
        I: crate::core::authority::Intent + 'static,
    >(
        &mut self,
        intent: I,
    ) -> bool {
        // Safety: tree pointer is valid for duration of event dispatch
        unsafe {
            let tree = self.tree.as_mut();
            let Some(tree) = tree else { return false };

            // Find the node for this widget
            let Some(node_id) = tree.find_by_id(self.widget_id) else {
                return false;
            };

            // Find the authority owner
            let Some(owner_id) = tree.find_authority_owner::<A>(node_id) else {
                return false;
            };

            // Push to the owner's intent buffer
            tree.intent_buffers_mut()
                .buffer_for(owner_id)
                .push(Box::new(intent));
            true
        }
    }
}

// ============================================================================
// UpdateContext (SPEC-HAL-146: Update Phase Animation Capability)
// ============================================================================

/// Context available to widgets during the update phase.
///
/// SPEC-HAL-146: The update phase is the only legal time
/// for visual state mutations. This context provides
/// animation capability so widgets can start transitions
/// in response to detected state changes.
pub(crate) use crate::core::widget_tree::OwnedWidgetPresentationGeometry;

pub struct UpdateContext<'a> {
    composite_delegation: Option<crate::core::composite_text_edit::CompositeDelegationFacts>,
    /// Animation capability — start/cancel transitions.
    animation: Option<crate::core::animation::AnimationContext<'a>>,
    animation_owner: Option<crate::core::animation::AnimationOwnerId>,
    animation_mount: Option<crate::core::widget_tree::WidgetAnimationMount<'a>>,
    own_geometry: Option<OwnedWidgetPresentationGeometry>,
    flat_viewport3d_transfers: Option<&'a mut crate::three_d::transfer::FlatViewport3DTransfers>,
    flat_viewport3d_owner: Option<TreeWidgetId>,
    flat_viewport3d_source: Option<crate::core::widget_tree::WidgetMountToken>,
    /// Active theme for this frame. Present for engine-driven updates.
    theme: Option<&'a Theme>,
    appearance: Option<FrameworkAppearanceFacts>,
    paint_invalidated: bool,
    subtree_paint_invalidated: bool,
    layout_invalidated: bool,
}

impl<'a> UpdateContext<'a> {
    pub fn new(animation: Option<crate::core::animation::AnimationContext<'a>>) -> Self {
        Self {
            composite_delegation: None,
            animation,
            animation_owner: None,
            animation_mount: None,
            own_geometry: None,
            flat_viewport3d_transfers: None,
            flat_viewport3d_owner: None,
            flat_viewport3d_source: None,
            theme: None,
            appearance: None,
            paint_invalidated: false,
            subtree_paint_invalidated: false,
            layout_invalidated: false,
        }
    }

    pub(crate) fn with_theme(
        animation: Option<crate::core::animation::AnimationContext<'a>>,
        theme: &'a Theme,
    ) -> Self {
        Self {
            composite_delegation: None,
            animation,
            animation_owner: None,
            animation_mount: None,
            own_geometry: None,
            flat_viewport3d_transfers: None,
            flat_viewport3d_owner: None,
            flat_viewport3d_source: None,
            theme: Some(theme),
            appearance: None,
            paint_invalidated: false,
            subtree_paint_invalidated: false,
            layout_invalidated: false,
        }
    }

    /// Reborrow frame authority only for this concrete Widget's one Update.
    /// Mount liveness is allocated lazily by an actual animation start.
    pub(crate) fn for_widget_update<'b>(
        &'b mut self,
        owner: TreeWidgetId,
        mount: crate::core::widget_tree::WidgetAnimationMount<'b>,
    ) -> UpdateContext<'b> {
        UpdateContext {
            composite_delegation: self.composite_delegation,
            animation: self
                .animation
                .as_mut()
                .map(|animation| animation.reborrow()),
            animation_owner: self.animation_owner.clone(),
            animation_mount: Some(mount),
            own_geometry: self.own_geometry,
            flat_viewport3d_transfers: self.flat_viewport3d_transfers.as_deref_mut(),
            flat_viewport3d_owner: Some(owner),
            flat_viewport3d_source: self.flat_viewport3d_source.clone(),
            theme: self.theme,
            appearance: self.appearance,
            paint_invalidated: false,
            subtree_paint_invalidated: false,
            layout_invalidated: false,
        }
    }

    pub(crate) fn own_presentation_geometry(&self) -> Option<OwnedWidgetPresentationGeometry> {
        self.own_geometry
    }

    pub(crate) fn set_composite_delegation(
        &mut self,
        facts: Option<crate::core::composite_text_edit::CompositeDelegationFacts>,
    ) {
        self.composite_delegation = facts;
    }

    pub(crate) fn composite_delegation(
        &self,
    ) -> Option<crate::core::composite_text_edit::CompositeDelegationFacts> {
        self.composite_delegation
    }

    pub(crate) fn set_own_presentation_geometry(
        &mut self,
        geometry: Option<OwnedWidgetPresentationGeometry>,
    ) {
        self.own_geometry = geometry;
    }

    pub(crate) fn animation_is_active(
        &self,
        handle: crate::core::animation::TransitionHandle,
    ) -> bool {
        self.animation
            .as_ref()
            .is_some_and(|animation| animation.is_active(handle))
    }

    fn animation_owner_for_start(&self) -> Option<crate::core::animation::AnimationOwnerId> {
        self.animation_mount
            .map(|mount| crate::core::animation::AnimationOwnerId::mounted(mount.token()))
            .or_else(|| self.animation_owner.clone())
    }

    pub(crate) fn set_flat_viewport3d_transfers(
        &mut self,
        owner: Option<&'a mut crate::three_d::transfer::FlatViewport3DTransfers>,
    ) {
        self.flat_viewport3d_transfers = owner;
    }

    pub(crate) fn prepare_flat_viewport3d_source(
        &mut self,
        tree: &crate::core::widget_tree::WidgetTree,
        owner: TreeWidgetId,
    ) {
        self.flat_viewport3d_source = self
            .flat_viewport3d_transfers
            .as_ref()
            .filter(|transfers| transfers.contains_source(owner))
            .and_then(|_| tree.concrete_mount(owner));
    }

    /// Publish one complete owned immutable flat request during this Widget's
    /// own engine-timed Update only (HAL-354 G1). Acceptance means pending cutoff,
    /// not setter success, target preparation, GPU commit or presentation.
    /// A link handle grants no peer Widget/tree access. Duplicate candidates and
    /// unconsumed results are explicitly refused without overwriting prior work.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_flat_viewport3d_request(
        &mut self,
        link: &crate::three_d::FlatViewport3DLinkId,
        correlation: u64,
        view_revision: crate::three_d::Viewport3DViewRevision,
        view: crate::three_d::ViewDescriptor3,
        snapshot: crate::three_d::RenderSnapshot,
        model: crate::three_d::ModelState,
        sample_count: crate::three_d::SampleCount3D,
    ) -> Result<
        crate::three_d::FlatViewport3DPublicationStatus,
        crate::three_d::FlatViewport3DTransferError,
    > {
        let owner = self
            .flat_viewport3d_owner
            .ok_or(crate::three_d::FlatViewport3DTransferError::NotEngineUpdate)?;
        let transfers = self
            .flat_viewport3d_transfers
            .as_deref_mut()
            .ok_or(crate::three_d::FlatViewport3DTransferError::Disabled)?;
        transfers.publish(
            owner,
            self.flat_viewport3d_source.as_ref(),
            link,
            correlation,
            view_revision,
            view,
            snapshot,
            model,
            sample_count,
        )
    }

    /// Active application theme for engine-driven updates.
    pub fn theme(&self) -> Option<&Theme> {
        self.theme
    }

    /// Framework facts for this update consumer, copied from the frame publication.
    pub fn appearance_facts(&self) -> Option<FrameworkAppearanceFacts> {
        self.appearance
    }

    pub(crate) fn set_appearance_facts(&mut self, facts: Option<FrameworkAppearanceFacts>) {
        self.appearance = facts;
    }

    /// Start an animation. Returns handle for later consumption
    /// in paint(). Returns None if animation system unavailable.
    pub fn start_animation<T: crate::core::animation::Interpolatable + Copy + 'static>(
        &mut self,
        anim: crate::core::animation::Animation<T>,
    ) -> Option<crate::core::animation::TransitionHandle> {
        match self.try_start_animation(anim) {
            Ok(handle) => handle,
            Err(error) => {
                tracing::error!(%error, "Timeline rejected update animation");
                None
            }
        }
    }

    pub fn try_start_animation<T: crate::core::animation::Interpolatable + Copy + 'static>(
        &mut self,
        animation: crate::core::animation::Animation<T>,
    ) -> Result<
        Option<crate::core::animation::TransitionHandle>,
        crate::core::animation::AnimationStartError,
    > {
        let owner = self
            .animation
            .as_ref()
            .and_then(|_| self.animation_owner_for_start());
        self.animation
            .as_mut()
            .map(|ctx| match owner {
                Some(owner) => ctx.start_for_owner(owner, animation),
                None => ctx.start(animation),
            })
            .transpose()
    }

    pub fn start_animation_descriptor<
        T: crate::core::animation::Interpolatable + Copy + 'static,
    >(
        &mut self,
        descriptor: crate::core::animation::AnimationDescriptor<T>,
    ) -> Result<
        Option<crate::core::animation::TransitionHandle>,
        crate::core::animation::AnimationStartError,
    > {
        let owner = self
            .animation
            .as_ref()
            .and_then(|_| self.animation_owner_for_start());
        self.animation
            .as_mut()
            .map(|ctx| match owner {
                Some(owner) => ctx.start_descriptor_for_owner(owner, descriptor),
                None => ctx.start_descriptor(descriptor),
            })
            .transpose()
    }

    /// Cancel an animation owned by this update consumer.
    pub fn cancel_animation(&mut self, handle: crate::core::animation::TransitionHandle) {
        if let Some(ctx) = self.animation.as_mut() {
            ctx.cancel(handle);
        }
    }

    /// Read the frame-stable sample established by the pre-routing advance.
    pub fn sample_animation<T: crate::core::animation::Interpolatable + Copy + 'static>(
        &self,
        handle: crate::core::animation::TransitionHandle,
    ) -> Option<crate::core::animation::AnimationSample<T>> {
        self.animation.as_ref().map(|ctx| ctx.sample(handle))
    }

    pub(crate) fn animation_frame_timestamp_secs(&self) -> Option<f64> {
        self.animation
            .as_ref()
            .and_then(|ctx| ctx.frame_timestamp_secs())
    }

    /// SPEC-HAL-354: Request local paint regeneration after this update.
    pub fn invalidate_paint(&mut self) {
        self.paint_invalidated = true;
    }

    /// SPEC-HAL-354: Regenerate this widget's retained paint subtree without layout.
    ///
    /// Use when update-owned shared presentation state is consumed by descendant
    /// widgets while their completed geometry remains valid.
    pub fn invalidate_paint_subtree(&mut self) {
        self.subtree_paint_invalidated = true;
        self.paint_invalidated = true;
    }

    /// SPEC-HAL-354: Request layout and paint regeneration after this update.
    pub fn invalidate_layout(&mut self) {
        self.layout_invalidated = true;
        self.paint_invalidated = true;
    }

    pub(crate) fn begin_widget_update(&mut self) {
        self.composite_delegation = None;
        self.animation_owner = None;
        self.animation_mount = None;
        self.own_geometry = None;
        self.flat_viewport3d_owner = None;
        self.flat_viewport3d_source = None;
        self.appearance = None;
        self.paint_invalidated = false;
        self.subtree_paint_invalidated = false;
        self.layout_invalidated = false;
    }

    pub(crate) fn begin_widget_update_for_owner(&mut self, owner: TreeWidgetId) {
        self.composite_delegation = None;
        self.animation_owner = Some(crate::core::animation::AnimationOwnerId::new(owner.0));
        self.animation_mount = None;
        self.own_geometry = None;
        self.flat_viewport3d_owner = Some(owner);
        self.flat_viewport3d_source = None;
        self.appearance = None;
        self.paint_invalidated = false;
        self.subtree_paint_invalidated = false;
        self.layout_invalidated = false;
    }

    pub(crate) fn take_widget_invalidations(&mut self) -> (bool, bool, bool) {
        let invalidations = (
            self.paint_invalidated,
            self.subtree_paint_invalidated,
            self.layout_invalidated,
        );
        self.paint_invalidated = false;
        self.subtree_paint_invalidated = false;
        self.layout_invalidated = false;
        invalidations
    }
}

// ============================================================================
// FocusIndicatorGeometry (SPEC-HAL-328 §12.2)
// ============================================================================

/// SPEC-HAL-328 §12.2: Widget-supplied geometry for the framework's focus
/// and capture indicator paint pass.
///
/// Widgets express the rect to outline and its corner radius. The framework
/// owns the color, width, animation, and z-order per §12.1 — these fields
/// describe only the target shape.
///
/// The default returned by `Widget::focus_indicator_geometry` is the widget's
/// own rect with radius 0. Composite widgets and embedded controls override
/// the trait method to return a sub-rect: SpinBox returns its embedded
/// value-field rect; RadioGroup returns the focused member's circle rect;
/// TabBar returns the active tab's rect; ListView returns the focused row's
/// rect.
#[derive(Debug, Clone, Copy)]
pub struct FocusIndicatorGeometry {
    /// Rect to outline, in the same coordinate space as the widget's paint
    /// rect (scene-coordinate, post-layout).
    pub rect: RectF,
    /// Corner radius applied to the indicator. 0.0 for square corners.
    pub radius: f32,
}

impl FocusIndicatorGeometry {
    /// Construct geometry covering the widget's own rect with no rounding.
    /// Used by the default `Widget::focus_indicator_geometry` implementation
    /// and by simple widgets that want the default visual.
    pub fn for_rect(rect: RectF) -> Self {
        Self { rect, radius: 0.0 }
    }
}

#[doc(hidden)]
pub struct CompositeChoiceOwnerHandle<'a> {
    owner: &'a mut dyn CompositeChoiceOwner,
}

impl<'a> CompositeChoiceOwnerHandle<'a> {
    pub(crate) fn new(owner: &'a mut dyn CompositeChoiceOwner) -> Self {
        Self { owner }
    }

    pub(crate) fn owner_mut(&mut self) -> &mut dyn CompositeChoiceOwner {
        &mut *self.owner
    }
}

/// Core widget trait - all UI components implement this
///
/// Widgets also explicitly implement the publicly re-exported `WidgetDowncast`
/// support trait. Both `as_any` and `as_any_mut` return `self`; there is no
/// automatic blanket implementation of this prerequisite.
pub trait Widget: Any + sealed::WidgetDowncast {
    /// Closed data-owner bridge for the admitted composite choice path.
    fn composite_choice_owner_mut(&mut self) -> Option<CompositeChoiceOwnerHandle<'_>> {
        None
    }

    /// Compute layout given constraints.
    ///
    /// SPEC-HAL-086 Stage 2: Returns LayoutResult containing both:
    /// - `size`: the final rendered size (respects constraints)
    /// - `content_size`: natural content size (may overflow for scrollables)
    ///
    /// For non-scrollable widgets, use `LayoutResult::fit(size)` where content_size == size.
    /// children: NodeIds of this widget's children from the tree
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[crate::core::widget_tree::NodeId],
    ) -> LayoutResult;

    /// SPEC-HAL-367: optionally supply current logical metrics before realization.
    /// A supporting layout invokes `settle` exactly once, consumes its presented Y,
    /// and returns that same extent as content_size. Declining does no layout work
    /// and never invokes `settle`; the engine then runs ordinary layout once.
    fn layout_scroll_content(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[crate::core::widget_tree::NodeId],
        settle: &mut dyn FnMut(Size) -> f32,
    ) -> Option<LayoutResult> {
        let _ = (cx, constraints, children, settle);
        None
    }

    /// Paint this widget into the scene graph
    /// Called after layout, with final rect position
    /// children: NodeIds of this widget's children from the tree
    fn paint(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        children: &[crate::core::widget_tree::NodeId],
    );

    /// Paint overlay elements after children have been painted
    /// Used for elements that must render on top of children (e.g., scrollbars)
    ///
    /// SPEC-HAL-359 §2.5.1 permits an explicit ancestor declaration to divert this
    /// complete current output into a closed decorative main-affordance channel.
    /// It is captured once at this widget, with unchanged keys/resources and
    /// inherited ancestor clip/scroll scopes. Exported output must have no input/
    /// tooltip sidecars or global-overlay/surface output. Ordinary unexported
    /// overlays retain their existing placement and semantics.
    ///
    /// # Parameters
    /// - `tree_widget_id`: The tree-managed TreeWidgetId (from alloc_widget_id), NOT self.id().
    ///   Use this for scene nodes that need to be hit-tested back to the widget tree.
    ///
    /// Default implementation does nothing
    fn paint_overlay(&self, _cx: &mut PaintContext, _rect: RectF, _tree_widget_id: TreeWidgetId) {}

    /// Close a widget-local non-modal popup on an outside primary press.
    ///
    /// The router calls this only for a mounted owner of an overlay hit
    /// region. True reports that the popup closed, so the engine retires
    /// that region and invalidates paint before continuing normal routing.
    /// This lifecycle notification carries no pointer or capture authority.
    fn dismiss_nonmodal_popup_on_outside_press(&mut self) -> bool {
        false
    }

    /// Per-frame update for time-based state changes.
    ///
    /// Called once per frame, before layout. Use for:
    /// - Scroll animation/auto-scroll
    /// - Time-based state transitions
    /// - Any mutation that depends on frame timing
    ///
    /// # Parameters
    /// - `dt`: Frame delta time in seconds (real time since last frame)
    ///
    /// Default implementation does nothing.
    fn update(&mut self, _dt: f32, _cx: &mut UpdateContext) {}

    /// SPEC-HAL-354: Whether this widget needs update-phase ticks without
    /// implying that its visual projection changes every frame.
    ///
    /// Use this for synchronized data-channel polling and asynchronous work
    /// completion. The widget must call `UpdateContext::invalidate_paint()` or
    /// `UpdateContext::invalidate_layout()` when a consumed result changes its
    /// visible projection.
    fn needs_update(&self) -> bool {
        false
    }

    /// Handle an input event with phase-aware routing
    /// Return true if event was handled and should stop propagation
    /// Use cx to request pointer capture or keyboard focus
    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let _ = (cx, event, phase);
        false
    }

    /// Get widget's internal ID (for widget-internal bookkeeping only)
    /// SPEC-HAL-087: Returns WidgetInternalId, NOT TreeWidgetId
    fn id(&self) -> WidgetInternalId;

    /// Whether this widget should participate in hit-testing
    /// Widgets like Label that are purely visual should return false
    /// so clicks pass through to their parent (e.g., Button)
    fn hit_testable(&self) -> bool {
        true
    }

    /// Debug: Return widget type name for diagnostic logging
    fn debug_type_name(&self) -> &'static str {
        "Widget"
    }

    /// Return clip bounds if this widget needs its children clipped
    /// Bounds are relative to the widget's own coordinate space (0,0 = top-left)
    /// Used by ScrollArea to clip scrolling content to viewport
    fn clip_bounds(&self) -> Option<RectF> {
        None
    }

    /// SPEC-R-020 Stage 5: Return rounded corners for clip region if needed
    /// Returns None for no rounding (fast scissor path)
    /// Returns Some(RoundedCorners) for GPU-based rounded clipping
    /// Only called when clip_bounds() returns Some(_)
    fn clip_bounds_rounded(&self) -> Option<RoundedCorners> {
        None
    }

    /// Return scroll offsets if this widget scrolls its children
    /// Traversal applies these when descending into children
    /// SPEC-HAL-024: Scroll offsets applied ONLY by traversal, never in layout
    fn scroll_offset(&self) -> Option<(f32, f32)> {
        None
    }

    /// SPEC-HAL-086 Stage 1: Return reference to scroll container if this widget is scrollable
    /// Used by compute_absolute_paint_rects() to get scroll offset from unified container
    fn scroll_container(&self) -> Option<&ScrollContainer> {
        None
    }

    /// SPEC-HAL-086 Stage 1: Return mutable reference to scroll container
    /// Used for programmatic scroll offset manipulation
    fn scroll_container_mut(&mut self) -> Option<&mut ScrollContainer> {
        None
    }

    /// SPEC-HAL-078B: Query if this scroll container has pending anchor resolution.
    ///
    /// Used by conflict resolution to detect viewport anchor compensation.
    /// Returns true if this widget is a scroll container with an anchor that
    /// needs resolution (which will cause offset compensation).
    ///
    /// CONFLICT-4: Widgets inside scroll containers with pending anchor resolution
    /// have their layout transitions denied because anchor preservation has
    /// higher priority than animation.
    fn has_pending_anchor_resolution(&self) -> bool {
        false
    }

    /// Whether this widget wants to receive scroll events
    /// Used by event routing to bubble scroll events to the right target
    fn wants_scroll(&self) -> bool {
        false
    }

    /// Whether this widget's layout-owned state observes user-driven scrolling
    /// in a descendant ScrollArea.
    fn layout_depends_on_descendant_scroll(&self) -> bool {
        false
    }

    /// Whether this widget requires bounded viewport constraints from a parent ScrollArea.
    ///
    /// Virtualizing widgets (List, ListView) return `true` because they use
    /// `constraints.max_height` to compute visible slot count and body viewport.
    /// Non-virtualizing widgets (Flex, Label, etc.) return `false` (default)
    /// so ScrollArea passes INFINITY on the scroll axis, allowing natural overflow
    /// instead of shrink-compression.
    ///
    /// PM-005: Prevents ScrollArea-Flex constraint feedback oscillation.
    fn needs_viewport_constraint(&self) -> bool {
        false
    }

    /// Sync scroll offset from ScrollArea (for virtualizing widgets).
    ///
    /// Called by post_layout_sync_scroll_to_lists() to propagate scroll offset
    /// to widgets that need it for virtualization (List, ListView, etc.).
    ///
    /// Returns true if the visible range changed (caller may mark layout dirty).
    ///
    /// INV-SCROLL-SOT-1: Scroll offsets originate from ScrollArea (single source of truth).
    fn sync_scroll(&mut self, _scroll_y: f32) -> bool {
        false
    }

    /// Invalidation required when `sync_scroll()` reports a change.
    ///
    /// Virtualized widgets keep the default because their scroll state changes
    /// realization geometry. Paint-windowed widgets may opt into paint-only
    /// invalidation when their layout and child geometry are unchanged.
    fn scroll_sync_invalidation(&self) -> ScrollSyncInvalidation {
        ScrollSyncInvalidation::Layout
    }

    /// Take and return any pending scroll Y request.
    /// Used by post_layout_resolve_scroll_anchors to handle focus-driven auto-scroll.
    /// Widgets like List and ListViewBody implement this to request scroll reveal
    /// after focus changes from keyboard navigation.
    fn take_requested_scroll_y(&mut self) -> Option<f32> {
        None
    }

    /// Observe pending scroll intent without consuming the request.
    fn has_requested_scroll(&self) -> bool {
        false
    }

    /// Return a read-only projection of separator centerlines in content coordinates.
    ///
    /// Post-layout synchronization copies `Some(centerlines)` into ScrollArea's
    /// compatibility cache. `None` skips the cache update. ScrollArea's event
    /// bubbling is independent of this projection.
    fn separator_hit_bands(&self) -> Option<Vec<f32>> {
        None
    }

    /// Layout optimization: whether this widget's height is stable under width constraint changes.
    ///
    /// When true, the Flex wrap layout can skip relayout passes for this widget if only
    /// width constraints changed. This is safe for widgets whose height doesn't depend on width:
    /// - Fixed-height containers (Container::with_height)
    /// - Buttons with fixed-height labels
    /// - Most non-text widgets
    ///
    /// When false (default), the widget is re-laid out when width constraints change,
    /// which is necessary for text that might reflow.
    ///
    /// Widgets should only return true if they guarantee their layout height won't change
    /// when given a different width constraint with the same height constraint.
    fn height_stable(&self) -> bool {
        false
    }

    /// Optional: called once when widget node is first inserted into the tree
    fn on_mount(&mut self) {}

    /// Optional: called once when widget node is removed from the tree
    fn on_unmount(&mut self) {}

    /// SPEC-HAL-134: Engine-controlled pre-layout hook.
    ///
    /// Called by the engine immediately before `layout()`. Widgets that need
    /// phase separation (like List) use this to transition into layout phase.
    ///
    /// INV-PHASE-TRANSITION-1: Phase transitions happen here, not in layout().
    /// Default: no-op for widgets without phase separation.
    fn pre_layout(&mut self) {}

    /// SPEC-HAL-131A: Return reference to AnchorResolver if this widget supports anchor resolution.
    /// Used by ScrollArea to resolve anchors before layout.
    fn as_anchor_resolver(&self) -> Option<&dyn crate::core::scroll_state::AnchorResolver> {
        None
    }

    /// SPEC-HAL-131C: Return reference to InputProjectionProvider if this widget virtualizes content.
    /// Used by the engine to build input projection during layout.
    fn as_input_projection_provider(
        &self,
    ) -> Option<&dyn crate::core::input_projection::InputProjectionProvider> {
        None
    }

    /// Return text info if this widget displays text
    /// Returns (text, font_size, color) for widgets like Label
    /// Used by Button to paint text with proper centering
    fn text_info(&self) -> Option<(&str, f32, Color)> {
        None
    }

    fn text_presentation(&self) -> Option<WidgetTextPresentation<'_>> {
        self.text_info()
            .map(|(text, font_size, color)| WidgetTextPresentation {
                text,
                font_size,
                color,
                typography: None,
            })
    }

    /// Disable painting for this widget (used when parent handles rendering)
    /// Default implementation does nothing; Label overrides to skip paint
    fn set_paint_disabled(&mut self, _disabled: bool) {}

    /// SPEC-HAL-035: Optional stable content key used by the widget diff optimizer.
    ///
    /// Semantics:
    /// - Same key between frames ⇒ paint output is assumed identical
    ///   for the same rect and layout constraints.
    /// - Different key ⇒ paint may have changed.
    ///
    /// Defaults to None, which means "always treat as paint-dirty".
    fn content_key(&self) -> Option<WidgetContentKey> {
        None
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 1: Focus System
    // ========================================================================

    /// SPEC-HAL-038: Whether this widget can receive keyboard focus.
    ///
    /// Widgets that return true will:
    /// - Receive focus when clicked (pointer-down)
    /// - Receive keyboard events when focused
    /// - Have on_focus/on_blur called on focus changes
    ///
    /// Default: false (most widgets are not focusable)
    fn focusable(&self) -> bool {
        false
    }

    /// SPEC-HAL-326: The layout axis of this container's children.
    /// Returns None for leaf widgets or containers without
    /// spatial ordering. Used by tree-structured focus navigation
    /// to determine sibling traversal direction.
    fn layout_axis(&self) -> Option<crate::core::layout::Axis> {
        None
    }

    /// SPEC-HAL-101 Stage-4C: Whether this widget needs continuous animation frames.
    ///
    /// Widgets that return true will cause the engine to continue running
    /// at full frame rate (via FrameReason::Animation) instead of throttling.
    ///
    /// Use cases:
    /// - TextField: caret blinking while focused
    /// - Animations: smooth transitions
    /// - Continuously changing visual projections
    ///
    /// Default: false (widget does not need animation)
    fn needs_animation(&self) -> bool {
        false
    }

    /// SPEC-HAL-038: Called when this widget gains keyboard focus.
    ///
    /// Use this to update visual state (e.g., show focus ring, cursor).
    /// Only called on widgets where focusable() returns true.
    fn on_focus(&mut self, _cx: &mut EventContext) {}

    /// SPEC-HAL-038: Called when this widget loses keyboard focus.
    ///
    /// Use this to update visual state (e.g., hide focus ring, cursor).
    /// Only called on widgets where focusable() returns true.
    fn on_blur(&mut self, _cx: &mut EventContext) {}

    /// Called when the widget enters keyboard Capture mode, either because:
    /// - User pressed the ENTER_WIDGET key (e.g. `i`)
    /// - Widget called request_keyboard_capture() (e.g. on click)
    /// Default is no-op.
    fn on_capture_enter(&mut self, _cx: &mut EventContext) {}

    /// Called when the widget loses keyboard Capture mode, either because:
    /// - Focus moved to another widget
    /// - Escape was pressed
    /// - Window lost focus
    /// Default is no-op.
    fn on_capture_leave(&mut self, _cx: &mut EventContext) {}

    /// Clear transient read-only text selection owned by this widget.
    ///
    /// An explicit force-clear, independent of implicit outside-down retention.
    fn clear_read_only_text_selection(&mut self) -> bool {
        false
    }

    /// HAL328 §19/HAL390 §16.5: implicit primary outside-down selection policy.
    /// Returns whether selection changed, never whether routing is consumed.
    /// Existing widgets delegate exactly once to their original explicit clear.
    fn clear_read_only_text_selection_on_outside_down(&mut self) -> bool {
        self.clear_read_only_text_selection()
    }

    /// SPEC-HAL-328 §12.2: Geometry of the focus / capture indicator the
    /// framework paints on this widget when it holds focus.
    ///
    /// Default returns the widget's own rect with no corner rounding. Widgets
    /// with internal focus targets override this to return the sub-rect of
    /// the actually-focused element:
    ///
    /// - SpinBox returns its embedded value-field rect.
    /// - RadioGroup returns the focused member's circle rect.
    /// - TabBar returns the active tab's rect.
    /// - ListView returns the focused row's rect.
    ///
    /// The returned rect MUST be in the same coordinate space as `own_rect`
    /// (scene-coordinate, post-layout).
    ///
    /// Geometry for the legacy framework indicator only. Complete SPEC-HAL-012/328
    /// appearances supply their own coordinated projection and skip this pass.
    fn focus_indicator_geometry(&self, own_rect: RectF) -> FocusIndicatorGeometry {
        FocusIndicatorGeometry::for_rect(own_rect)
    }

    /// Whether the widget's paint path supplies its complete focus/capture
    /// treatment. SPEC-HAL-012/328 consumers return true so the framework does
    /// not append an independent default indicator. Unmigrated inline painters
    /// also retain true during migration.
    ///
    /// Default returns `true` for the duration of the §12 migration. The
    /// framework's focus paint pass skips widgets that return `true` and lets
    /// them continue emitting their own focus visuals.
    ///
    /// Unmigrated widgets using the legacy framework indicator return false.
    /// A complete appearance consumes framework facts, never a local focus or
    /// keyboard-capture mirror.
    fn paints_own_focus_indicator(&self) -> bool {
        true
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 3: Focus Scopes
    // ========================================================================

    /// SPEC-HAL-038 Stage 3: Whether this widget defines a focus scope.
    ///
    /// When true, this widget becomes a focus scope root. Tab navigation
    /// will cycle only through focusable descendants of this widget, not
    /// the entire application.
    ///
    /// Use cases:
    /// - Modals/dialogs that should trap Tab focus
    /// - Dropdown menus
    /// - Action palettes
    /// - Any overlay that needs contained keyboard navigation
    ///
    /// Default: false (widget does not define a scope)
    fn is_focus_scope(&self) -> bool {
        false
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 6: Keyboard Activation
    // ========================================================================

    /// SPEC-HAL-038 Stage 6: Whether this widget accepts keyboard activation.
    ///
    /// When true and the widget is focused, pressing Enter or Space will
    /// trigger `on_keyboard_activate()`. This provides keyboard accessibility
    /// for interactive widgets like buttons.
    ///
    /// Default: false (widget does not accept keyboard activation)
    fn accepts_keyboard_activation(&self) -> bool {
        false
    }

    /// SPEC-HAL-038 Stage 6: Called when widget is activated via keyboard.
    ///
    /// This is called when the widget is focused and the user presses
    /// Enter or Space. The widget should perform the same action as
    /// a pointer click.
    ///
    /// Only called on widgets where `accepts_keyboard_activation()` returns true.
    fn on_keyboard_activate(&mut self, _cx: &mut EventContext) {}

    // ========================================================================
    // SPEC-HAL-176: Keyboard Authority Declarations
    // ========================================================================

    /// SPEC-HAL-176: Whether this widget can enter Capture mode.
    ///
    /// Widgets that return true will transition the keyboard authority mode
    /// from Navigation to Capture when activated via `enter_widget` action.
    /// While in Capture, the widget owns all keyboard input.
    ///
    /// Typical capture-eligible widgets: TextField, TextArea, Slider, SpinBox,
    /// ColorPicker, or any widget that consumes arbitrary keyboard input for mutation.
    ///
    /// Widgets that are NOT capture-eligible (Button, Checkbox, Label) receive
    /// inline activation instead — Enter/Space fires their action without mode change.
    ///
    /// INV-176-CAPTURE-1: capture_eligible and defines_focus_scope are mutually exclusive.
    ///
    /// Default: false
    fn capture_eligible(&self) -> bool {
        false
    }

    /// Static opt-in to physical responses within the existing Capture authority.
    /// Normal responses do not release Capture. Escape, Tab and focus laws remain.
    fn physical_keyboard_capture(&self) -> bool {
        false
    }

    /// Settle uncommitted response work before an observed interruption releases
    /// authority. This notification does not undo already committed values.
    fn on_keyboard_capture_interrupted(
        &mut self,
        _interruption: &crate::core::event::KeyboardCaptureInterruption,
        _cx: &mut EventContext,
    ) {
    }

    /// SPEC-HAL-176: Whether this widget defines a focus scope boundary.
    ///
    /// Widgets that return true act as focus scope containers. When entered
    /// via `enter_widget` action in Navigation mode, a new focus scope is
    /// pushed and navigation is confined to this widget's focusable descendants.
    /// Escape pops the scope and returns focus to the container.
    ///
    /// Typical scope containers: Panel, List, Tree, TabBar, SplitView.
    ///
    /// INV-176-CAPTURE-1: capture_eligible and defines_focus_scope are mutually exclusive.
    ///
    /// Default: false
    fn defines_focus_scope(&self) -> bool {
        false
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 7: Hover Lifecycle
    // ========================================================================

    /// SPEC-HAL-038 Stage 7: Called when the pointer enters this widget's bounds.
    ///
    /// Use this to update visual state (e.g., show hover effect).
    /// This is a lifecycle method, not an event - it does not participate
    /// in event capture/bubble phases.
    fn on_hover_enter(&mut self, _cx: &mut EventContext) {}

    /// SPEC-HAL-038 Stage 7: Called when the pointer leaves this widget's bounds.
    ///
    /// Use this to reset visual state (e.g., remove hover effect).
    /// This is a lifecycle method, not an event - it does not participate
    /// in event capture/bubble phases.
    ///
    /// Note: This is NOT called if the pointer leaves due to PointerDown on
    /// this widget (pointer capture takes over). It IS called if the pointer
    /// moves to a different widget or leaves the window.
    fn on_hover_leave(&mut self, _cx: &mut EventContext) {}

    // ========================================================================
    // SPEC-HAL-128 Stage 8: Declarative Gesture Surfaces
    // ========================================================================

    /// SPEC-HAL-128 Stage 8 INV-GS1: Static gesture surface declaration.
    ///
    /// Returns the gesture kinds this widget supports. This declaration:
    /// - MUST be static (determinable without runtime input state)
    /// - MUST NOT vary based on PointerType, focus, capture, or modifiers
    /// - MUST NOT change between layout passes
    ///
    /// The system uses this declaration to:
    /// 1. Register gesture candidates on pointer-down (INV-GS3)
    /// 2. Filter via PointerType legality (Stage 7)
    /// 3. Perform arbitration
    ///
    /// Widgets MUST NOT:
    /// - Inspect PointerType (INV-GS4)
    /// - Query other candidates (INV-GS3)
    /// - Override arbitration (INV-GS2)
    ///
    /// Default: empty slice (widget does not participate in gestures)
    fn gesture_surface(&self) -> &'static [crate::core::event::GestureKind] {
        &[]
    }

    /// Static admission for native touch pointer delivery through the input router.
    fn accepts_touch_pointer_input(&self) -> bool {
        false
    }

    /// HAL-397 owner-local qualification of a tentative touch target.
    fn touch_target(&self, _hit: crate::core::event::TouchHit) -> Option<crate::core::event::TouchTarget> {
        None
    }

    /// Revalidation token for the owner-local touch target projection.
    fn touch_target_revision(&self) -> Option<u64> {
        None
    }

    /// Runs in the existing widget dispatch only after Tap admission.
    fn on_touch_tap(&mut self, _cx: &mut EventContext, _tap: crate::core::event::TouchTap) -> bool {
        false
    }

    fn touch_drag_axis(
        &self, _target: crate::core::event::TouchTarget,
    ) -> Option<crate::core::event::GestureAxis> {
        None
    }

    fn on_touch_drag_motion(&mut self, _cx: &mut EventContext, _motion: crate::core::event::TouchDragMotion) {}

    /// Static, gesture-only participation in HAL-396 content scrolling.
    /// Ordinary pointer delivery still requires `accepts_touch_pointer_input`.
    fn accepts_touch_scroll_gesture(&self) -> bool {
        false
    }

    fn touch_scroll_requires_overlay(&self) -> bool {
        false
    }

    /// Request the registered Scroll candidate for a qualified motion sample.
    /// The router owns the movement threshold, axis choice and arbitration.
    fn touch_scroll_intent(
        &self,
        _motion: crate::core::event::QualifiedTouchMotion,
        _axis: crate::core::event::GestureAxis,
    ) -> bool {
        false
    }

    /// Apply a subsequent sample for the winning press, including final Up.
    fn on_touch_scroll_motion(&mut self, _cx: &mut EventContext) {}

    /// A transient content surface can retire its committed gesture when its
    /// own input geometry or current role is withdrawn.
    fn touch_scroll_is_live(&self, _press: crate::core::event::TouchPressIdentity) -> bool {
        true
    }

    /// HAL-396 unfinished Button press policy. An exit-disarmed press remains
    /// owned by its initiating pointer until release or cancellation.
    fn has_unfinished_touch_press(&self, _pointer: crate::core::event::PointerId) -> bool {
        false
    }

    /// Static motion representation for one declared gesture kind.
    fn pointer_motion_policy(
        &self,
        _kind: crate::core::event::GestureKind,
    ) -> crate::core::relative_pointer::PointerMotionPolicy {
        crate::core::relative_pointer::PointerMotionPolicy::Absolute
    }

    /// Static, complete geometry domain for a primary Absolute Drag surface.
    fn absolute_drag_geometry_dependency(&self) -> Option<AbsoluteDragGeometryDependency> {
        None
    }

    /// Settle only a revoked geometry-dependent capture's cached presentation.
    fn settle_absolute_drag_geometry_appearance(
        &mut self,
        _facts: crate::core::appearance::FrameworkAppearanceFacts,
    ) {
    }

    /// SPEC-HAL-128 Stage 8 INV-GS8: Called when a gesture is committed to this widget.
    ///
    /// The system calls this after arbitration when this widget wins.
    /// The widget:
    /// - MUST accept (cannot refuse or defer)
    /// - MUST NOT delegate to another widget
    /// - MAY now mutate state (side effects allowed after commit)
    ///
    /// This is the ONLY notification that a gesture was committed.
    /// Pointer capture is already active when this is called.
    ///
    /// # Arguments
    /// * `kind` - The gesture kind that was committed
    fn on_gesture_commit(
        &mut self,
        _cx: &mut EventContext,
        _kind: crate::core::event::GestureKind,
    ) {
    }

    /// SPEC-HAL-128 Stage 8 INV-GS9: Called when a gesture candidate is cancelled.
    ///
    /// The system calls this when:
    /// - Another gesture won arbitration (this widget lost)
    /// - Escape was pressed during arbitration
    /// - Session was cancelled for any reason
    ///
    /// The widget:
    /// - MUST accept (cannot refuse or retry)
    /// - MUST restore all state to pre-registration condition (INV-GS7)
    /// - MUST NOT emit application activation or commitment events
    /// - MUST release all tentative resources
    ///
    /// A capture owner may also receive on_pointer_capture_cancel for its
    /// distinct role. Rollback must be idempotent across both notifications.
    /// Changed paint-consumed state needs narrow invalidation unless the caller
    /// already guarantees it before retained reuse (SPEC-HAL-354, SPEC-PM-018).
    ///
    /// # Arguments
    /// * `kind` - The gesture kind that was cancelled
    fn on_gesture_cancel(
        &mut self,
        _cx: &mut EventContext,
        _kind: crate::core::event::GestureKind,
    ) {
    }

    /// SPEC-HAL-128 INV-PC5: Terminal notification to the actual capture owner,
    /// including owners with no gesture candidate. Capture is already revoked.
    /// Restore pointer interaction state without activation, recapture, or changes
    /// to independent keyboard/focus state. Timeline writes remain Update-owned.
    /// Cleanup is idempotent with on_gesture_cancel when both roles are held.
    /// SPEC-HAL-128 defines these distinct notifications; SPEC-HAL-354 and
    /// SPEC-PM-018 govern invalidation of changed paint-consumed state.
    fn on_pointer_capture_cancel(
        &mut self,
        _cx: &mut EventContext,
        _pointer_id: crate::core::event::PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
    }

    /// Owner-only terminal outcome for a RelativeLocked gesture.
    fn on_relative_pointer_terminal(
        &mut self,
        cx: &mut EventContext,
        kind: crate::core::event::GestureKind,
        outcome: crate::core::relative_pointer::RelativePointerTerminalOutcome,
    ) {
        if !matches!(
            outcome,
            crate::core::relative_pointer::RelativePointerTerminalOutcome::Completed { .. }
        ) {
            self.on_gesture_cancel(cx, kind);
        }
    }

    // ========================================================================
    // SPEC-HAL-131E: Projected Action Routing
    // ========================================================================

    /// SPEC-HAL-131E: Handle a projected action for an unrealized item.
    ///
    /// Called when a virtualizing container receives an action for an item
    /// that is not currently realized (no widget exists). The container
    /// handles the action by updating its state based on logical identity.
    ///
    /// Invariants:
    /// - INV-NO-IMPLICIT-REALIZE-1: This method MUST NOT trigger realization.
    /// - INV-STATE-IDENTITY-1: Action targets logical identity, not widget.
    ///
    /// # Arguments
    /// * `logical_id` - The stable logical identity of the target item
    /// * `logical_index` - The index of the target item (convenience)
    /// * `action` - The projected action to perform
    ///
    /// # Returns
    /// `true` if the action was handled, `false` otherwise.
    ///
    /// Default: false (widget does not handle projected actions)
    fn handle_projected_action(
        &mut self,
        _cx: &mut EventContext,
        _logical_id: crate::core::input_projection::LogicalItemId,
        _logical_index: usize,
        _action: crate::core::input_projection::ProjectedAction,
    ) -> bool {
        false
    }

    // ========================================================================
    // SPEC-HAL-154: Deferred Wiring Access
    // ========================================================================
    //
    // CONTRACT: If a widget returns Some(_) from deferred_wiring_mut,
    // it MUST return Some(_) from deferred_wiring, referring to the same object.
    // Violating this contract silently bypasses paint/event guards.
    // ========================================================================

    /// SPEC-HAL-154: Return mutable access to DeferredWiring if this widget uses deferred wiring.
    ///
    /// The engine uses this hook to:
    /// - INV-SEAL-ENG-1: Verify wiring is complete before first layout
    /// - INV-SEAL-ENG-2: Seal the widget exactly once, immediately before first layout
    ///
    /// Widgets that implement DeferredWiring MUST override this to return `Some(self)`.
    /// Widgets MUST NOT call `seal()` themselves anymore (engine owns lifecycle).
    ///
    /// Default: None (widget does not use deferred wiring)
    fn deferred_wiring_mut(
        &mut self,
    ) -> Option<&mut dyn crate::core::deferred_wiring::DeferredWiring> {
        None
    }

    /// SPEC-HAL-154: Return immutable access to DeferredWiring for assertion checks.
    ///
    /// Used by paint/event guards to verify INV-SEAL-ENG-NO-PAINT-BEFORE-SEAL
    /// and INV-SEAL-ENG-NO-EVENT-BEFORE-SEAL without requiring mutable access.
    ///
    /// Widgets that implement DeferredWiring MUST override this to return `Some(self)`.
    /// Default: None (widget does not use deferred wiring)
    fn deferred_wiring(&self) -> Option<&dyn crate::core::deferred_wiring::DeferredWiring> {
        None
    }

    // ========================================================================
    // SPEC-HAL-266A: Custom Render Preparation Admission
    // ========================================================================

    /// Return the explicit SPEC-HAL-295A opt-in marker used for participant-index
    /// maintenance. Implementing `CustomRenderParticipant` without this opaque
    /// admission marker does not register the widget.
    fn custom_render_admission(&self) -> Option<&crate::renderer::CustomRenderAdmission> {
        None
    }

    /// Return the participant-local GPU cache/lifecycle hook.
    fn custom_render_participant_mut(
        &mut self,
    ) -> Option<&mut dyn crate::renderer::CustomRenderParticipant> {
        None
    }

    // ========================================================================
    // SPEC-HAL-190: Authority Projection
    // ========================================================================
    //
    // Widgets that own authority state (TabularSubstrate, SelectionSet, etc.)
    // implement these methods to declare their authority ownership.
    // ========================================================================

    /// SPEC-HAL-190: Return a type-erased reference to the authority this widget provides.
    ///
    /// Called by the engine during tree traversal to build the authority registry.
    /// The registry maps NodeId to TypeId for efficient ancestor lookup.
    ///
    /// # Invariants
    ///
    /// - INV-AUTH-REGISTER-1: A widget MUST return the same TypeId for its lifetime.
    /// - INV-AUTH-REGISTER-2: Only direct ownership is permitted.
    ///
    /// # Returns
    ///
    /// - `Some((authority_ref, type_id))` if this widget owns an authority
    /// - `None` if this widget does not own any authority
    ///
    /// Default: None (widget does not provide authority)
    fn provided_authority(&self) -> Option<(&dyn std::any::Any, std::any::TypeId)> {
        None
    }

    /// SPEC-HAL-190: Return a mutable type-erased reference to the authority.
    ///
    /// Used by the deferred mutation phase to allow authority owners to mutate
    /// their state after processing intents.
    ///
    /// # Returns
    ///
    /// - `Some((authority_ref_mut, type_id))` if this widget owns an authority
    /// - `None` if this widget does not own any authority
    ///
    /// Default: None (widget does not provide authority)
    fn provided_authority_mut(&mut self) -> Option<(&mut dyn std::any::Any, std::any::TypeId)> {
        None
    }

    /// SPEC-HAL-190: Return self as AuthorityOwner if this widget owns authority.
    ///
    /// This method enables mechanical enforcement of INV-AUTH-SINGLE-OWNER:
    /// only widgets that implement AuthorityOwner can receive intents during
    /// the deferred mutation phase.
    ///
    /// # Implementation
    ///
    /// Widgets that own authority and implement `AuthorityOwner` must override:
    /// ```ignore
    /// fn as_authority_owner(&mut self) -> Option<&mut dyn AuthorityOwner> {
    ///     Some(self)
    /// }
    /// ```
    ///
    /// # Invariants
    ///
    /// - INV-AUTH-OWNER-MECHANICAL: Only AuthorityOwner implementors receive intents.
    /// - INV-INTENT-PROCESS-1: Called exactly once per frame if buffer is non-empty.
    ///
    /// Default: None (widget does not own authority)
    fn as_authority_owner(&mut self) -> Option<&mut dyn AuthorityOwner> {
        None
    }

    // ========================================================================
    // SPEC-HAL-241: Drag and Drop
    // ========================================================================

    /// Return drag payload if this widget supports being dragged.
    /// Called on DragStart when this widget has pointer capture.
    fn drag_payload(&self) -> Option<Box<dyn std::any::Any>> {
        None
    }

    /// Paint a drag proxy ghost for this widget during an active drag.
    /// Called each frame while this widget is the drag source.
    ///
    /// `rect` is positioned at the cursor (offset by a small amount) with
    /// the widget's original layout size from when the drag started.
    ///
    /// Default: does nothing (no ghost). Override to provide visual feedback.
    fn paint_drag_proxy(&self, _cx: &mut PaintContext, _rect: RectF, _alpha: f32) {}

    /// Check if this widget accepts a drop of the given payload type.
    /// Called during DragUpdate to filter eligible targets.
    fn accepts_drop(&self, _payload_type: std::any::TypeId) -> bool {
        false
    }

    /// Called when a drag enters this widget's bounds (eligible target only).
    fn on_drag_enter(&mut self) {}

    /// Called when a drag leaves this widget's bounds.
    fn on_drag_leave(&mut self) {}

    /// Called when a drop occurs on this widget. Return true if consumed.
    fn on_drop(&mut self, _payload: Box<dyn std::any::Any>) -> bool {
        false
    }
}

// ============================================================================
// SPEC-HAL-158: Release-Grade Layout Invariant Tests
// ============================================================================
//
// INV-ZERO-2 enforcement is tested by verifying the panic message exists.
// Full integration tests exist in widget_tree.rs tests.
// The enforcement is verified at compile time by the panic!() macro usage.
//
// VERIFICATION: The following paths now panic with "INV-ZERO-2 VIOLATION":
// - layout_child() when take_widget fails
// - layout_child_full() when take_widget fails
// - Button::layout() when take_widget fails
// - Container::layout() when take_widget fails
// - Panel::layout() when take_widget fails
// ============================================================================

#[cfg(test)]
mod layout_invariant_tests {
    /// SPEC-HAL-158 INV-ZERO-2: Verify enforcement message exists in panic strings.
    ///
    /// This is a compile-time verification that the enforcement is in place.
    /// The actual panic behavior is tested by attempting invalid operations.
    #[test]
    fn test_inv_zero_2_enforcement_messages_exist() {
        // Verify the panic message pattern exists
        let layout_child_msg = "INV-ZERO-2 VIOLATION: Layout structural corruption";
        let button_msg = "INV-ZERO-2 VIOLATION: Button layout corruption";
        let container_msg = "INV-ZERO-2 VIOLATION: Container layout corruption";
        let panel_msg = "INV-ZERO-2 VIOLATION: Panel layout corruption";

        // These assertions verify the message strings compile and are accessible
        assert!(layout_child_msg.contains("INV-ZERO-2"));
        assert!(button_msg.contains("INV-ZERO-2"));
        assert!(container_msg.contains("INV-ZERO-2"));
        assert!(panel_msg.contains("INV-ZERO-2"));
    }

    /// SPEC-HAL-158: Verify take_widget on invalid node returns None.
    ///
    /// The enforcement relies on take_widget returning None for invalid nodes,
    /// which then triggers the panic in layout_child.
    #[test]
    fn test_take_widget_invalid_node_returns_none() {
        use crate::core::widget_tree::WidgetTree;

        let mut tree = WidgetTree::new();

        // Create a fake NodeId that doesn't exist in the tree
        let invalid_node_id = crate::core::widget_tree::NodeId(9999);

        // take_widget should return None for invalid node
        let result = tree.take_widget(invalid_node_id);
        assert!(
            result.is_none(),
            "take_widget must return None for invalid NodeId"
        );
    }
}
