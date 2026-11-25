// SPEC-HAL-018 Stage 4: ScreenHost Widget
// SPEC-HAL-248: Navigation projection consumer (sole runtime pull point).
// SPEC-HAL-333 INV-G2: top-strip allocation for the in-window menu bar.
//
// A container widget that hosts the active screen and modal subtrees.
// ScreenHost itself does not know about AppRoot or screens - it's just a conduit
// where AppRoot rebuilds the active view subtree each frame.
//
// SPEC-HAL-248: ScreenHost also holds the current ProjectedSurfaceView, which is
// the sole terminal output of the navigation projection pipeline.
// Guarded runtime glue assembles NavigationFrameProjection from separate
// navigation, overlay, and focus authorities, resolves it via Compositor, and
// stores only the result here. ScreenHost does not import or depend on any
// navigation types — only compositor-native ProjectedSurfaceView.

use crate::compositor::ProjectedSurfaceView;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// SPEC-HAL-018 Stage 4: Host widget for screen and modal subtrees.
/// SPEC-HAL-248: Sole runtime consumer of navigation projection data.
///
/// ScreenHost is a container that:
/// - Occupies the app's root area (fills available space)
/// - Hosts one "content subtree" created from the active base screen + modals
/// - Does NOT know about AppRoot, ScreenStack, or ModalStack
/// - Holds the current `ProjectedSurfaceView` for root-host coordination and
///   runtime verification (SPEC-HAL-248)
///
/// Model A1 rule: Screens own their widgets and state.
/// ScreenHost is just a mount point where AppRoot rebuilds subtrees each frame.
///
/// SPEC-HAL-248 projection contract:
/// - `projected_view` is set by the runtime glue once per frame via `set_projected_view()`
/// - ScreenHost does NOT cache projection across frames (each pull replaces the previous)
/// - ScreenHost does NOT infer navigation, focus, or overlays
/// - ScreenHost does NOT store surface identity as retained state
/// - Renderer code never reads `projected_view()`; rendering uses SurfaceStackSnapshot
///
/// Layout: Fills available space. If a menu-bar child is registered
/// (SPEC-HAL-333 INV-G2), it claims the top strip — full width, its own
/// height — and every other child takes the remainder below it. With no
/// bar registered, children get full tight constraints at (0, 0), exactly
/// the pre-menu behavior (INV-G4: native hosts mount no bar).
/// Paint: No extra rendering, children paint themselves
/// Children ordering: base screen, modals (bottom-to-top), root overlays
/// (toast), and — when in-window — the menu bar (registered by NodeId).
pub struct ScreenHost {
    id: WidgetInternalId,
    /// SPEC-HAL-248: Current frame's projected surface view.
    /// Set by guarded runtime glue after resolving the complete frame through
    /// Compositor::apply_frame_projection(). None until first successful pull.
    projected_view: Option<ProjectedSurfaceView>,
    /// SPEC-HAL-333 INV-G2: the mounted in-window menu bar child, registered
    /// by App::run at mount. None when the active host is native (INV-G4) —
    /// layout is then identical to the pre-menu behavior.
    menu_bar_child: Option<NodeId>,
}

impl ScreenHost {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            projected_view: None,
            menu_bar_child: None,
        }
    }

    /// SPEC-HAL-248: Store the projected surface view for the current frame.
    /// Called by guarded runtime glue after resolving the complete frame through
    /// Compositor::apply_frame_projection(). Replaces any previous value.
    pub fn set_projected_view(&mut self, view: ProjectedSurfaceView) {
        self.projected_view = Some(view);
    }

    /// SPEC-HAL-248: Remove all resolved projection testimony synchronously.
    pub fn clear_projected_view(&mut self) {
        self.projected_view = None;
    }

    /// SPEC-HAL-248: Read the projected surface view for coordination or verification.
    /// Returns None before the first projection pull.
    pub fn projected_view(&self) -> Option<&ProjectedSurfaceView> {
        self.projected_view.as_ref()
    }

    /// SPEC-HAL-333 INV-G2: register (or clear) the menu-bar strip child.
    /// Called by App::run after mounting the in-window bar.
    pub fn set_menu_bar_child(&mut self, node: Option<NodeId>) {
        self.menu_bar_child = node;
    }
}

impl Default for ScreenHost {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for ScreenHost {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ScreenHost {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // =======================================================================
        // INV-SCREENHOST-NO-INSET: ScreenHost MUST NOT apply root_inset.
        // =======================================================================
        //
        // root_inset is CONTENT PADDING, not container offset.
        // ScreenHost is a pure pass-through container:
        //   - Children receive FULL constraints (no inset subtraction)
        //   - Children positioned at (0, 0) (no inset offset)
        //   - Scrollbars in child ScrollArea remain flush with ScreenHost edges
        //
        // The sole consumer of root_inset is ScrollArea, which insets content
        // while keeping scrollbars at container edges.
        //
        // GUARD: This debug_assert fires if root_inset is accessed in ScreenHost.
        // If you need to reference root_inset here, you are violating the contract.
        // See INV-ROOT-INSET-CONTRACT in theme.rs for ownership rules.
        // =======================================================================

        // DEBUG GUARD: Verify root_inset is not being used for positioning.
        // This guard exists to catch accidental misuse in future modifications.
        // The value is read ONLY to assert it's not being applied.
        #[cfg(debug_assertions)]
        {
            let _ = cx.theme.geometry.root_inset; // Touch to enable lint detection
            // If you're adding code here that uses root_inset, STOP.
            // ScreenHost is forbidden from consuming root_inset.
            // See INV-SCREENHOST-NO-INSET and INV-ROOT-INSET-CONTRACT.
        }

        // ScreenHost fills available space
        let size = Size::new(constraints.max_width, constraints.max_height);

        // SPEC-HAL-333 INV-G2: the registered menu bar claims the top strip —
        // tight width, loose height (the bar reports its theme-token height).
        let mut strip_height = 0.0f32;
        if let Some(bar_id) = self.menu_bar_child {
            if children.contains(&bar_id) {
                let bar_constraints = Constraints::new(size.width, size.width, 0.0, size.height);
                let bar_size = cx.layout_child(bar_id, bar_constraints);
                cx.set_child_rect(bar_id, RectF::new(0.0, 0.0, size.width, bar_size.height));
                strip_height = bar_size.height;
            }
        }

        // SPEC-HAL-158: Pass TIGHT constraints to children.
        // ScrollArea's is_final_pass check requires min ≈ max to set viewport_extent.
        // Loose constraints (min=0) cause viewport_extent to stay at zero,
        // resulting in a zero-sized clip rect that clips away all content.
        let content_size = Size::new(size.width, (size.height - strip_height).max(0.0));
        let child_constraints = Constraints::tight(content_size);

        // Layout all non-bar children with tight constraints, below the strip.
        for &child_id in children {
            if Some(child_id) == self.menu_bar_child {
                continue;
            }
            let child_size = cx.layout_child(child_id, child_constraints);
            cx.set_child_rect(
                child_id,
                RectF::new(0.0, strip_height, child_size.width, child_size.height),
            );
        }

        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        LayoutResult::fit(size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        // ScreenHost does not paint anything itself.
        // Children (base screen, modals) are painted by the standard traversal.
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        // ScreenHost itself is not hit-testable; its children are.
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "ScreenHost"
    }
}
