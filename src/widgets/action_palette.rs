// ============================================================================
// SPEC-HAL-020 Stage 6: Action Palette Widget
// ============================================================================

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::runtime::actions::{Action, ActionId, ActionRegistry};
use crate::utils::id::WidgetInternalId;

/// Action palette widget for searching and selecting actions.
///
/// Stage 6: Self-contained widget with search input and results list.
/// Stage 7: Extended to store action IDs for execution.
pub struct ActionPalette {
    id: WidgetInternalId,
    /// Search query text
    query: String,
    /// Cursor position within query (for text editing)
    cursor: usize,
    /// Current search results (action names for display)
    result_names: Vec<String>,
    /// Action IDs corresponding to results (Stage 7)
    result_ids: Vec<ActionId>,
    /// Selected result index (for keyboard navigation)
    selected: usize,
    /// Dirty flag: true if search needs to run
    needs_search: bool,
}

impl ActionPalette {
    /// Create a new action palette widget.
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            query: String::new(),
            cursor: 0,
            result_names: Vec::new(),
            result_ids: Vec::new(),
            selected: 0,
            needs_search: true,
        }
    }

    /// Update search results from the action registry.
    ///
    /// Only runs if needs_search flag is set. Clamps selection to valid range.
    /// Stage 7: Also stores action IDs for execution.
    pub fn update_search(&mut self, registry: &ActionRegistry) {
        if !self.needs_search {
            return;
        }

        // Get search results
        let mut buf: Vec<&Action> = Vec::new();
        registry.search(&self.query, &mut buf);

        // Store action names and IDs
        self.result_names.clear();
        self.result_ids.clear();
        for action in buf {
            let display = if let Some(shortcut) = action.shortcut {
                format!("{} ({:?})", action.name, shortcut.key)
            } else {
                action.name.to_string()
            };
            self.result_names.push(display);
            self.result_ids.push(action.id);
        }

        // Clamp selected index to valid range
        if !self.result_names.is_empty() {
            self.selected = self.selected.min(self.result_names.len() - 1);
        } else {
            self.selected = 0;
        }

        self.needs_search = false;
    }

    /// Set the search query and mark for re-search.
    pub fn set_query(&mut self, s: String) {
        self.cursor = s.len();
        self.query = s;
        self.needs_search = true;
    }

    /// Handle text input character.
    pub fn on_text_input(&mut self, ch: char) {
        self.query.insert(self.cursor, ch);
        self.cursor += 1;
        self.needs_search = true;
    }

    /// Handle backspace key (delete character before cursor).
    pub fn on_backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.query.remove(self.cursor);
            self.needs_search = true;
        }
    }

    /// Handle delete key (delete character at cursor).
    pub fn on_delete(&mut self) {
        if self.cursor < self.query.len() {
            self.query.remove(self.cursor);
            self.needs_search = true;
        }
    }

    /// Move cursor left.
    pub fn on_move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right.
    pub fn on_move_right(&mut self) {
        if self.cursor < self.query.len() {
            self.cursor += 1;
        }
    }

    /// Move selection up (previous result).
    pub fn on_move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down (next result).
    pub fn on_move_down(&mut self) {
        if self.selected + 1 < self.result_names.len() {
            self.selected += 1;
        }
    }

    /// Get the currently selected result index, if any.
    pub fn selected_index(&self) -> Option<usize> {
        if self.selected < self.result_names.len() {
            Some(self.selected)
        } else {
            None
        }
    }

    /// Stage 7: Get the action ID of the currently selected action.
    pub fn selected_action_id(&self) -> Option<ActionId> {
        self.result_ids.get(self.selected).copied()
    }

    /// Stage 8: Set the selected index directly.
    pub fn set_selected(&mut self, idx: usize) {
        if idx < self.result_names.len() {
            self.selected = idx;
        }
    }

    /// Stage 8: Hit-test a point to determine which result row was clicked.
    ///
    /// Returns the row index if the point is inside a result row.
    /// Does NOT mutate state. Does NOT close modal. Does NOT execute actions.
    /// Palette remains strictly presentation + internal state.
    ///
    /// SPEC-HAL-090 NOTE: Hardcoded values here MUST stay in sync with paint().
    /// Theme access unavailable in this method (no context). If layout changes,
    /// update both paint() and hit_row() together.
    pub fn hit_row(&self, point: crate::core::math::Point, palette_rect: RectF) -> Option<usize> {
        // Input area: padding_l (10px) + input_height (30px) + padding_l (10px) = 50px
        // UNMAPPED: No theme access - must match paint() values manually
        let input_height = 50.0;
        let results_y = palette_rect.y + input_height;
        let row_height = 30.0; // UNMAPPED: component-specific
        let max_visible = 20;

        // Check if point is within palette bounds
        if !palette_rect.contains(point) {
            return None;
        }

        // Check if point is in results area (not input area)
        if point.y < results_y {
            return None;
        }

        // Calculate which row was clicked
        let relative_y = point.y - results_y;
        let row_idx = (relative_y / row_height) as usize;

        // Return row index if it's valid and within visible results
        if row_idx < self.result_names.len().min(max_visible) {
            Some(row_idx)
        } else {
            None
        }
    }
}

impl WidgetDowncast for ActionPalette {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ActionPalette {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Palette size from theme geometry
        // Clamp to constraints if needed
        let width = cx
            .theme
            .geometry
            .palette
            .width
            .clamp(constraints.min_width, constraints.max_width);
        let height = cx
            .theme
            .geometry
            .palette
            .height
            .clamp(constraints.min_height, constraints.max_height);
        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Stage 6: Minimal palette rendering
        // SPEC-HAL-090: Use spacing/padding tokens instead of hardcoded literals
        let padding_l = cx.theme.padding.l;
        let padding_m = cx.theme.padding.m;
        let spacing_s = cx.theme.spacing.s;

        // Background
        cx.scene
            .push_rect(rect, cx.theme.colors.surfaces.content, 0, None);

        // Search input area (top section)
        // UNMAPPED: input_height = 30.0 is component-specific (needs SPEC-HAL-094 spec)
        let input_height = 30.0;
        let input_rect = RectF::new(
            rect.x + padding_l,
            rect.y + padding_l,
            rect.width - padding_l * 2.0,
            input_height,
        );
        cx.scene
            .push_rect(input_rect, cx.theme.colors.surfaces.base0, 1, None);

        // Render query text
        // NOTE: Hardcoded 16.0 - not using theme.typography.font_size to avoid
        // visual drift (SPEC-HAL-092 defines multiple typography roles).
        let text_size = 16.0;
        let text_color = cx.theme.colors.typography.primary;
        if !self.query.is_empty() {
            let run = cx.text_engine.shape(&self.query, text_size);
            cx.text_engine.draw(
                &run,
                input_rect.x + padding_m,
                input_rect.y + padding_m,
                text_color,
                LAYER_CONTENT + 1,
                cx.scene,
            );
        }

        // Results area (below input)
        let results_y = input_rect.y + input_rect.height + padding_l;
        // UNMAPPED: row_height = 30.0 is component-specific (needs SPEC-HAL-094 spec)
        let row_height = 30.0;
        let max_visible = 20; // Show first 20 results max

        for (idx, result_name) in self.result_names.iter().take(max_visible).enumerate() {
            let row_y = results_y + (idx as f32 * row_height);
            let row_rect = RectF::new(
                rect.x + padding_l,
                row_y,
                rect.width - padding_l * 2.0,
                row_height - 2.0,
            );

            // Highlight selected row
            if idx == self.selected {
                cx.scene
                    .push_rect(row_rect, cx.theme.colors.surfaces.level2, 2, None);
            }

            // Result text (action name + shortcut if present)
            let name_run = cx.text_engine.shape(result_name, text_size);
            cx.text_engine.draw(
                &name_run,
                row_rect.x + padding_m,
                row_rect.y + spacing_s,
                text_color,
                LAYER_CONTENT + 1,
                cx.scene,
            );
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        // Stage 6: No event handling in widget itself
        // Events will be handled by parent/modal logic in Stage 7
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    /// SPEC-HAL-038 Stage 4: ActionPalette is a focus scope.
    ///
    /// When the Action Palette Modal is open, Tab navigation is trapped
    /// within the palette's focusable descendants.
    fn is_focus_scope(&self) -> bool {
        true
    }
}

impl Default for ActionPalette {
    fn default() -> Self {
        Self::new()
    }
}
