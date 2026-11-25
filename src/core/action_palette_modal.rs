// ============================================================================
// SPEC-HAL-020 Stage 7-8: Action Palette Modal
// ============================================================================

use crate::core::app::{Screen, ScreenContext, ScreenFrameContext};
use crate::core::event::{FusedInputEvent, KeyCode};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::overlay_root::DismissPolicy;
use crate::core::paint::Color;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::ActionPalette;

// ============================================================================
// Stage 8: Dim Background Layer Widget
// ============================================================================

/// Dim layer widget: Fills entire window with semi-transparent black.
/// Does NOT render the palette. Does NOT contain the palette.
/// Only paints the dim overlay.
struct DimLayerWidget {
    id: WidgetInternalId,
}

impl DimLayerWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for DimLayerWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DimLayerWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Fill available space (entire window)
        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Paint semi-transparent black overlay
        cx.scene
            .push_rect(rect, Color::from_srgb(0.0, 0.0, 0.0, 0.67), 0, None);
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &crate::core::event::Event,
        _phase: crate::core::event::EventPhase,
    ) -> bool {
        // Receives no input
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

/// Action Palette Modal: Searchable action list overlay.
///
/// Stage 7: Modal screen that hosts the ActionPalette widget and handles:
/// - Keyboard input routing (text, arrows, Enter, Esc)
/// - Action execution on Enter
/// - Modal close on Esc or after execution
///
/// Stage 8: Extended with dim layer, mouse interaction, and scroll navigation
pub struct ActionPaletteModal {
    palette: ActionPalette,
    needs_rebuild: bool,
    /// Stage 8: Palette rect for hit-testing (set during build)
    palette_rect: RectF,
}

impl ActionPaletteModal {
    pub fn new() -> Self {
        Self {
            palette: ActionPalette::new(),
            needs_rebuild: true,
            palette_rect: RectF::zero(),
        }
    }
}

impl Default for ActionPaletteModal {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ActionPaletteModal {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Update search results from registry
        self.palette.update_search(&cx.app.registry);

        // Stage 8: Get real window size from parent node
        let (window_width, window_height) =
            if let Some(parent_rect) = cx.tree.get_rect(cx.parent_node) {
                (parent_rect.width, parent_rect.height)
            } else {
                // Fallback if parent rect not available
                (1000.0, 800.0)
            };

        // Stage 8: Mount dim layer first (fills entire window)
        let _dim_layer_id = cx.mount_child(Box::new(DimLayerWidget::new()));

        // Stage 8: Mount palette second (appears on top)
        let _palette_id = cx.mount_child(Box::new(ActionPalette::new()));

        // SPEC-HAL-141: Rect mutations are forbidden during build().
        // ScreenHost::layout will position children at (0,0) anyway.
        // We compute the centered rect here only for hit-testing purposes.
        // TODO: The actual palette renders at (0,0), creating a hit-test mismatch.
        //       Proper fix requires a centering layout container or overlay positioning.
        let palette_width = 600.0;
        let palette_height = 400.0;
        let x = (window_width - palette_width) / 2.0;
        let y = (window_height - palette_height) / 2.0;
        self.palette_rect = RectF::new(x, y, palette_width, palette_height);

        &mut self.palette
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        self.needs_rebuild = true;
        // Clear any previous search state
        self.palette.set_query(String::new());
    }

    fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
        match event {
            // Stage 8: Mouse click handling
            // S4.INV2: Modal observes pointer but does NOT decide dismissal.
            //          Outside-click dismissal is handled by OverlayRoot shielding path.
            FusedInputEvent::PointerDown { pos, .. } => {
                let point = Point::new(pos.x, pos.y);
                // Check if click is inside palette
                if self.palette_rect.contains(point) {
                    // Hit-test row
                    if let Some(row_idx) = self.palette.hit_row(point, self.palette_rect) {
                        self.palette.set_selected(row_idx);
                        self.needs_rebuild = true;
                    }
                }
                // S4.INV2: Do NOT close modal here. OverlayRoot handles outside-click dismissal.
                return true;
            }
            // Stage 8: Scroll wheel handling
            FusedInputEvent::Scroll { delta, .. } => {
                if delta.1 < 0.0 {
                    // Scroll up - move selection up
                    self.palette.on_move_up();
                    self.needs_rebuild = true;
                } else if delta.1 > 0.0 {
                    // Scroll down - move selection down
                    self.palette.on_move_down();
                    self.needs_rebuild = true;
                }
                return true;
            }
            FusedInputEvent::KeyDown { key_code, .. } => {
                match key_code {
                    // S4.INV2: Escape dismissal is handled by OverlayRoot, not here.
                    //          Modal must not call nav_close_modal() for Escape.
                    KeyCode::Escape => {
                        // Do NOT handle Escape here - OverlayRoot owns Escape dismissal.
                        // Return false to let the event propagate to centralized handling.
                        return false;
                    }
                    KeyCode::Enter => {
                        // Execute selected action and close
                        // Note: This is an intentional action result, not policy-based dismissal.
                        if let Some(action_id) = self.palette.selected_action_id() {
                            // Get handler (immutable borrow ends in this block)
                            let handler_opt = {
                                cx.app
                                    .registry
                                    .actions
                                    .get(&action_id)
                                    .map(|action| action.handler)
                            };
                            // Execute handler if found (mutable borrow, no conflict)
                            if let Some(handler) = handler_opt {
                                handler(&mut cx.app);
                            }
                        }
                        cx.nav_close_modal();
                        return true;
                    }
                    KeyCode::Up => {
                        self.palette.on_move_up();
                        self.needs_rebuild = true;
                        return true;
                    }
                    KeyCode::Down => {
                        self.palette.on_move_down();
                        self.needs_rebuild = true;
                        return true;
                    }
                    KeyCode::Left => {
                        self.palette.on_move_left();
                        self.needs_rebuild = true;
                        return true;
                    }
                    KeyCode::Right => {
                        self.palette.on_move_right();
                        self.needs_rebuild = true;
                        return true;
                    }
                    KeyCode::Backspace => {
                        self.palette.on_backspace();
                        self.needs_rebuild = true;
                        return true;
                    }
                    KeyCode::Delete => {
                        self.palette.on_delete();
                        self.needs_rebuild = true;
                        return true;
                    }
                    KeyCode::Character(ch) => {
                        // Handle text input
                        self.palette.on_text_input(*ch);
                        self.needs_rebuild = true;
                        return true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    /// SPEC-HAL-127 Stage 4: ActionPalette dismisses on outside click and Escape.
    ///
    /// S4.INV1: OverlayRoot uses this to decide when to dismiss.
    fn dismiss_policy(&self) -> DismissPolicy {
        DismissPolicy::popup_default()
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
