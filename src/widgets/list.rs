// =============================================================================
// List: TabularSubstrate-backed List Widget
// =============================================================================
//
// List is a real Widget that mirrors List v1's interaction logic structure
// while using TabularSubstrate as the selection/focus authority.
//
// Architecture:
// - TabularSubstrate owns selection/focus state (single source of truth)
// - List owns transient interaction state (drag, hover, pressed, range session)
// - List translates interactions into TabularSubstrate intents
// - Range union uses SetSelection intent (one batch operation)
//
// Event handling mirrors List v1:
// - Same capture/target phase logic
// - Same drag start/move/end state machine
// - Same activation suppression during drag
// =============================================================================

use std::any::Any;
use std::collections::HashSet;
use std::ops::Range;

use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, Modifiers, PointerButton, PointerEventKind,
};
use crate::core::input_projection::{InputProjectionProvider, LogicalItemId, ProjectedAction};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::ordered_selection::{SelectionInteractionModel, SelectionMode};
use crate::core::paint::Color;
use crate::core::scroll_state::{AnchorId, AnchorResolver, ScrollAnchor};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::tabular_view::TabularSubstrate;
use crate::widgets::tabular_view::column::Column;
use crate::widgets::tabular_view::intent_policy::{
    ActivationMode, DragPolicy, IntentContext, IntentInput, RangePolicy, SubcontrolBehavior,
    TabularIntentPolicy,
};
use crate::widgets::tabular_view::intents::{
    ActivationIntent, FocusIntent, SelectionIntent, TabularIntent,
};
use crate::widgets::tabular_view::row::RowId;

// =============================================================================
// List Widget
// =============================================================================

/// List: TabularSubstrate-backed list widget with List v1 interaction semantics.
pub struct List {
    id: WidgetInternalId,

    /// Embedded TabularSubstrate (selection/focus authority).
    tabular: TabularSubstrate,

    /// Row IDs for index-to-RowId mapping.
    row_ids: Vec<RowId>,

    /// Selection mode.
    selection_mode: SelectionMode,

    /// Interaction model (immediate vs transactional).
    interaction_model: SelectionInteractionModel,

    /// Item count.
    item_count: usize,

    /// Uniform item height.
    item_height: f32,

    /// Use theme row height token.
    /// When true, item_height is resolved from cx.theme.list_geom.row_height at layout.
    /// Set to false when set_item_height() is called (explicit user override).
    /// SPEC-HAL-094/SPEC-HAL-096C: Row height must be token-driven, not hardcoded.
    use_theme_row_height: bool,

    /// Visible range (for virtualization).
    visible_range: Range<usize>,

    /// Layout bounds.
    layout_bounds: Size,

    /// Viewport height (cached from constraints).
    viewport_height: f32,

    /// Scroll Y position.
    scroll_y: f32,

    /// Requested scroll Y for focus reveal.
    /// Set by navigation handlers, consumed by post_layout_resolve_scroll_anchors.
    requested_scroll_y: Option<f32>,

    /// Widget has focus.
    focused: bool,

    /// Disabled state.
    disabled: bool,

    /// Zebra striping enabled (alternating row colors).
    /// Default: true (matches List v1 default).
    zebra_enabled: bool,

    // =========================================================================
    // Transient Interaction State (List-owned, NOT authoritative)
    // =========================================================================
    /// Hovered item index.
    hovered_index: Option<usize>,

    /// Pressed item index.
    pressed_index: Option<usize>,

    // =========================================================================
    // SPEC-HAL-135: Drag Selection State
    // =========================================================================
    /// Drag selection active.
    drag_active: bool,

    /// Drag anchor index.
    drag_anchor: Option<usize>,

    /// Last index during drag.
    drag_last_index: Option<usize>,

    /// Toggle already handled in capture phase (dedup for input projection).
    ctrl_click_handled_index: Option<usize>,

    /// SPEC-HAL-200: Intent policy for interaction interpretation.
    intent_policy: TabularIntentPolicy,

    // =========================================================================
    // Callbacks
    // =========================================================================
    /// Selection changed callback.
    on_selection_changed: Option<Box<dyn FnMut(&HashSet<usize>)>>,

    /// Activation callback.
    on_activate: Option<Box<dyn FnMut(usize)>>,
}

impl List {
    /// Create a new List with the specified selection mode.
    pub fn new(selection_mode: SelectionMode) -> Self {
        Self::new_with_config(selection_mode, 0, 24.0)
    }

    /// Create with configuration.
    fn new_with_config(selection_mode: SelectionMode, item_count: usize, item_height: f32) -> Self {
        let mut tabular = TabularSubstrate::new();

        // Single column for list — elastic so layout can provide any width without violating INV-COL-WIDTH-1.
        tabular.add_column(Column::new(200.0).with_flex(1.0));

        // Forward selection mode to TabularSubstrate (authoritative).
        tabular.set_selection_mode(selection_mode);

        // INV-TV-ROW-HEIGHT-CONSISTENCY: Set fixed row height BEFORE adding rows.
        // This ensures RowModel uses the correct height for all rows.
        tabular.set_fixed_row_height(item_height);

        // Create row IDs.
        let row_ids: Vec<RowId> = (0..item_count).map(|_| RowId::new()).collect();
        for &id in &row_ids {
            tabular.add_row_with_height(id, item_height);
        }

        // INV-LIST2-ROW-ORDERING: List owns row ordering for its embedded TabularSubstrate.
        // TabularSubstrate requires non-empty sorted_indices for virtualization to work.
        // List uses natural order (identity mapping) since it doesn't support sorting.
        tabular.set_sorted_indices((0..item_count).collect());

        Self {
            id: WidgetInternalId::new(),
            tabular,
            row_ids,
            selection_mode,
            interaction_model: SelectionInteractionModel::Immediate,
            item_count,
            item_height,
            use_theme_row_height: true,
            visible_range: 0..item_count,
            layout_bounds: Size::zero(),
            viewport_height: 0.0,
            scroll_y: 0.0,
            requested_scroll_y: None,
            focused: false,
            disabled: false,
            zebra_enabled: true,
            hovered_index: None,
            pressed_index: None,
            drag_active: false,
            drag_anchor: None,
            drag_last_index: None,
            ctrl_click_handled_index: None,
            intent_policy: Self::build_intent_policy(selection_mode),
            on_selection_changed: None,
            on_activate: None,
        }
    }

    // =========================================================================
    // Configuration
    // =========================================================================

    /// Set item count.
    pub fn set_item_count(&mut self, count: usize) {
        let current = self.row_ids.len();
        if count > current {
            for _ in current..count {
                let id = RowId::new();
                self.row_ids.push(id);
                self.tabular.add_row_with_height(id, self.item_height);
            }
        } else if count < current {
            self.row_ids.truncate(count);
            self.tabular.clear_rows();
            for &id in &self.row_ids {
                self.tabular.add_row_with_height(id, self.item_height);
            }
        }
        self.item_count = count;
        self.visible_range = 0..count;

        // INV-LIST2-ROW-ORDERING: Update sorted_indices after row count change.
        // Must happen after rows are added/removed, before scroll-based realization.
        self.tabular.set_sorted_indices((0..count).collect());
    }

    /// Get item count.
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Set item height.
    ///
    /// **INV-TV-ROW-HEIGHT-CONSISTENCY**: Updates both List's cached item_height
    /// and TabularSubstrate's RowModel to ensure paint and hit-test use the same geometry.
    ///
    /// **SPEC-HAL-094/096C**: Explicit call disables theme-driven row height.
    /// After calling this, List uses the specified height instead of theme token.
    pub fn set_item_height(&mut self, height: f32) {
        // Explicit override disables theme-driven row height.
        self.use_theme_row_height = false;
        self.item_height = height;
        // INV-TV-ROW-HEIGHT-CONSISTENCY: Set fixed row height BEFORE rebuilding rows.
        // This ensures RowModel uses the correct height for all rows.
        self.tabular.set_fixed_row_height(height);
        // Rebuild TabularSubstrate rows with new height to maintain geometry consistency.
        self.tabular.clear_rows();
        for &id in &self.row_ids {
            self.tabular.add_row_with_height(id, height);
        }

        // INV-LIST2-ROW-ORDERING: Rebuild sorted_indices after row rebuild.
        self.tabular
            .set_sorted_indices((0..self.item_count).collect());
    }

    /// Get item height.
    pub fn item_height(&self) -> f32 {
        self.item_height
    }

    /// Set scroll Y position.
    ///
    /// **INV-SCROLL-SINGLE-SOURCE**: Forwards scroll to TabularSubstrate.
    /// TabularSubstrate is the authority for scroll state during hit-testing.
    /// List maintains a local copy for backward compatibility only.
    pub fn set_scroll_y(&mut self, scroll_y: f32) {
        self.scroll_y = scroll_y;
        // INV-SCROLL-SINGLE-SOURCE: Forward to TabularSubstrate.
        self.tabular.set_scroll_y(scroll_y);
    }

    /// Get scroll Y position.
    pub fn scroll_y(&self) -> f32 {
        // Return TabularSubstrate's scroll_y as the authoritative value.
        self.tabular.scroll_y()
    }

    /// Take the requested scroll Y for focus reveal.
    ///
    /// Called by post_layout_resolve_scroll_anchors to consume scroll requests.
    /// Returns None if no scroll request is pending.
    pub fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.requested_scroll_y.take()
    }

    /// Check if there's a pending scroll request.
    pub fn has_requested_scroll(&self) -> bool {
        self.requested_scroll_y.is_some()
    }

    /// Update visible range from external scroll offset (scroll sync entry point).
    ///
    /// Called by the engine's scroll sync mechanism to propagate ScrollArea's
    /// scroll offset to List's embedded TabularSubstrate.
    ///
    /// Returns true if the visible range changed (caller should mark layout dirty).
    pub fn update_visible_range_from_scroll(&mut self, scroll_y: f32) -> bool {
        self.scroll_y = scroll_y;
        self.tabular.update_visible_range_from_scroll(scroll_y)
    }

    /// Set selection mode.
    ///
    /// Forwards to TabularSubstrate (authoritative). Local field kept for
    /// temporary compatibility during migration.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
        self.tabular.set_selection_mode(mode);
        self.intent_policy = Self::build_intent_policy(mode);
    }

    /// Get selection mode.
    ///
    /// Reads from TabularSubstrate (authoritative).
    pub fn selection_mode(&self) -> SelectionMode {
        self.tabular.selection_mode()
    }

    /// Set interaction model.
    ///
    /// Forwards to TabularSubstrate (authoritative). Local field kept for
    /// temporary compatibility during migration.
    pub fn set_interaction_model(&mut self, model: SelectionInteractionModel) {
        self.interaction_model = model;
        self.tabular.set_interaction_model(model);
    }

    /// Get interaction model.
    ///
    /// Reads from TabularSubstrate (authoritative).
    pub fn interaction_model(&self) -> SelectionInteractionModel {
        self.tabular.interaction_model()
    }

    /// Check if transactional mode.
    ///
    /// Reads from TabularSubstrate (authoritative).
    pub fn is_transactional(&self) -> bool {
        self.tabular.is_transactional()
    }

    /// Set disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Check if disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Set selection changed callback.
    pub fn set_on_selection_changed<F>(&mut self, callback: F)
    where
        F: FnMut(&HashSet<usize>) + 'static,
    {
        self.on_selection_changed = Some(Box::new(callback));
    }

    /// Set activation callback.
    pub fn set_on_activate<F>(&mut self, callback: F)
    where
        F: FnMut(usize) + 'static,
    {
        self.on_activate = Some(Box::new(callback));
    }

    // =========================================================================
    // TabularSubstrate Access
    // =========================================================================

    /// Get read-only access to TabularSubstrate.
    pub fn tabular_view(&self) -> &TabularSubstrate {
        &self.tabular
    }

    /// Get mutable access to TabularSubstrate.
    pub fn tabular_view_mut(&mut self) -> &mut TabularSubstrate {
        &mut self.tabular
    }

    // =========================================================================
    // Index/RowId Conversion
    // =========================================================================

    fn index_to_row_id(&self, index: usize) -> Option<RowId> {
        self.row_ids.get(index).copied()
    }

    fn row_id_to_index(&self, row_id: RowId) -> Option<usize> {
        self.row_ids.iter().position(|&id| id == row_id)
    }

    // =========================================================================
    // Selection State Accessors
    // =========================================================================
    // Reads from ordered_selection (authoritative).

    /// Check if an item is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.index_to_row_id(index)
            .map(|id| self.tabular.ordered_selection().is_selected(id))
            .unwrap_or(false)
    }

    /// Get current selection as indices.
    pub fn selection(&self) -> HashSet<usize> {
        self.tabular
            .ordered_selection()
            .selected_keys_iter()
            .filter_map(|&id| self.row_id_to_index(id))
            .collect()
    }

    /// Check if any selection exists.
    pub fn has_selection(&self) -> bool {
        !self.tabular.ordered_selection().is_selection_empty()
    }

    /// Get selection count.
    pub fn selection_count(&self) -> usize {
        self.tabular.ordered_selection().selection_count()
    }

    /// Get persistent anchor index.
    pub fn persistent_anchor(&self) -> Option<usize> {
        self.tabular
            .ordered_selection()
            .anchor_key()
            .and_then(|id| self.row_id_to_index(id))
    }

    // =========================================================================
    // Focus State Accessors
    // =========================================================================

    /// Check if an item is focused.
    pub fn is_focused(&self, index: usize) -> bool {
        self.index_to_row_id(index)
            .map(|id| {
                self.tabular
                    .focus_state()
                    .focused()
                    .map(|c| c.row_id == id)
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Get focused index.
    pub fn focused_index(&self) -> Option<usize> {
        self.tabular
            .focus_state()
            .focused()
            .and_then(|c| self.row_id_to_index(c.row_id))
    }

    /// Check if list has focus.
    pub fn has_focus(&self) -> bool {
        self.focused
    }

    // =========================================================================
    // Transactional API (delegates to TabularSubstrate)
    // =========================================================================

    /// Begin transaction.
    pub fn begin_transaction(&mut self) -> bool {
        self.tabular.begin_transaction()
    }

    /// Check if transaction active.
    pub fn is_transaction_active(&self) -> bool {
        self.tabular.is_transaction_active()
    }

    /// Commit transaction.
    pub fn commit_transaction(&mut self) -> Option<HashSet<usize>> {
        if self.tabular.commit_transaction() {
            Some(self.selection())
        } else {
            None
        }
    }

    /// Cancel transaction.
    pub fn cancel_transaction(&mut self) -> bool {
        self.tabular.cancel_transaction()
    }

    // =========================================================================
    // Drag State Accessors
    // =========================================================================

    /// Get drag anchor index.
    pub fn drag_anchor_index(&self) -> Option<usize> {
        self.drag_anchor
    }

    /// Check if drag active.
    pub fn is_drag_active(&self) -> bool {
        self.drag_active
    }

    // =========================================================================
    // Intent Policy (SPEC-HAL-200)
    // =========================================================================

    fn build_intent_policy(mode: SelectionMode) -> TabularIntentPolicy {
        match mode {
            SelectionMode::None => TabularIntentPolicy {
                selection_mode: SelectionMode::None,
                activation_mode: ActivationMode::ExplicitOnly,
                range_selection: RangePolicy::Disabled,
                drag_selection: DragPolicy::Disabled,
                subcontrol_behavior: SubcontrolBehavior::ClickSelectsRow,
                interaction_model: SelectionInteractionModel::Immediate,
                expansion_enabled: false,
            },
            SelectionMode::Single => TabularIntentPolicy {
                selection_mode: SelectionMode::Single,
                activation_mode: ActivationMode::ExplicitOnly,
                range_selection: RangePolicy::Disabled,
                drag_selection: DragPolicy::Disabled,
                subcontrol_behavior: SubcontrolBehavior::ClickSelectsRow,
                interaction_model: SelectionInteractionModel::Immediate,
                expansion_enabled: false,
            },
            SelectionMode::Multi => TabularIntentPolicy {
                selection_mode: SelectionMode::Multi,
                activation_mode: ActivationMode::ExplicitOnly,
                range_selection: RangePolicy::ShiftRange,
                drag_selection: DragPolicy::RangeOnDrag,
                subcontrol_behavior: SubcontrolBehavior::ClickSelectsRow,
                interaction_model: SelectionInteractionModel::Immediate,
                expansion_enabled: false,
            },
        }
    }

    // =========================================================================
    // Focus Operations
    // =========================================================================

    fn column_id(&self) -> crate::widgets::tabular_view::column::ColumnId {
        self.tabular.column_model().columns()[0].id
    }

    fn set_focus(&mut self, index: usize) {
        if let Some(row_id) = self.index_to_row_id(index) {
            let col_id = self.column_id();
            self.tabular
                .handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
                    row_id,
                    column_id: col_id,
                }));

            // Request scroll to reveal the focused row.
            // List uses natural order, so index == visual position.
            if let Some(scroll_y) = self.tabular.compute_scroll_to_reveal_index(index) {
                self.requested_scroll_y = Some(scroll_y);
            }
        }
    }

    // =========================================================================
    // Drag State Cleanup
    // =========================================================================

    /// Finalize drag and clean up transient drag state.
    fn finalize_range_selection(&mut self) {
        if !self.drag_active {
            return;
        }

        self.drag_active = false;
        self.drag_anchor = None;
        self.drag_last_index = None;
    }

    // =========================================================================
    // Callback Firing
    // =========================================================================

    fn fire_selection_changed(&mut self) {
        let selection = self.selection();
        if let Some(callback) = &mut self.on_selection_changed {
            callback(&selection);
        }
    }

    // =========================================================================
    // Hit Testing
    // =========================================================================

    /// Hit-test to find the row index at a given viewport-local Y coordinate.
    ///
    /// **INV-ROW-GEOM-SINGLE-SOURCE**: Delegates to TabularSubstrate for row geometry.
    /// List does NOT compute row indices from `item_height`. TabularSubstrate is the
    /// single source of truth for row heights, offsets, and scroll state.
    ///
    /// # Arguments
    /// * `viewport_y` - Y coordinate relative to the List widget's viewport top.
    ///   Computed as `global_pos.y - rect.y`. TabularSubstrate adds scroll internally.
    fn hit_test_item(&self, viewport_y: f32) -> Option<usize> {
        // INV-ROW-GEOM-SINGLE-SOURCE: Delegate to TabularSubstrate.
        // TabularSubstrate.hit_test_row_id_viewport_y adds scroll_y to convert to content space.
        let row_id = self.tabular.hit_test_row_id_viewport_y(viewport_y)?;

        // Convert RowId back to index via our row_ids mapping
        self.row_id_to_index(row_id)
    }

    // =========================================================================
    // Keyboard Handling
    // =========================================================================

    fn handle_keyboard(&mut self, event: &KeyEvent) -> bool {
        if !event.is_down {
            return false;
        }

        // Lock interaction model on first user input affecting selection/focus.
        self.tabular.lock_interaction_model();

        match event.key_code {
            KeyCode::Escape => {
                if self.is_transactional() && self.is_transaction_active() {
                    self.cancel_transaction();
                    return true;
                }
                if !self.has_selection() {
                    return false;
                }
                // Route through policy → ClearSelection.
                let context = IntentContext {
                    focused: self.focused,
                    has_anchor: false,
                    is_editing: false,
                    focused_row: self.tabular.focus_state().focused().map(|c| c.row_id),
                    focused_row_expanded: false,
                };
                let input = IntentInput::KeyPress {
                    key: KeyCode::Escape,
                    modifiers: event.modifiers,
                };
                let intents = self.intent_policy.interpret(&input, &context);
                for intent in &intents {
                    self.tabular.handle_intent(intent);
                }
                self.fire_selection_changed();
                true
            }

            KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown => {
                // Compute navigation target index.
                let target_index = match event.key_code {
                    KeyCode::Up => self.compute_new_focus(-1),
                    KeyCode::Down => self.compute_new_focus(1),
                    KeyCode::Home => 0,
                    KeyCode::End => self.item_count.saturating_sub(1),
                    KeyCode::PageUp => {
                        let page = self.page_size();
                        let current = self
                            .focused_index()
                            .unwrap_or(self.item_count.saturating_sub(1));
                        current.saturating_sub(page)
                    }
                    KeyCode::PageDown => {
                        let page = self.page_size();
                        let current = self.focused_index().unwrap_or(0);
                        let last = self.item_count.saturating_sub(1);
                        (current + page).min(last)
                    }
                    _ => unreachable!(),
                };

                // Resolve target RowId and interpret through policy.
                if let Some(target_row_id) = self.index_to_row_id(target_index) {
                    let context = IntentContext {
                        focused: self.focused,
                        has_anchor: false,
                        is_editing: false,
                        focused_row: Some(target_row_id),
                        focused_row_expanded: false,
                    };
                    let input = IntentInput::KeyPress {
                        key: event.key_code,
                        modifiers: event.modifiers,
                    };
                    let intents = self.intent_policy.interpret(&input, &context);

                    // Transactional mode: suppress SelectRow (focus-only navigation).
                    let suppress_select = self.is_transactional();
                    let mut fired = false;
                    for intent in &intents {
                        if suppress_select
                            && matches!(
                                intent,
                                TabularIntent::Selection(SelectionIntent::SelectRow { .. })
                            )
                        {
                            continue;
                        }
                        self.tabular.handle_intent(intent);
                        fired = true;
                    }
                    self.set_focus(target_index);
                    if fired {
                        self.fire_selection_changed();
                    }
                } else {
                    self.set_focus(target_index);
                }
                true
            }

            KeyCode::Space => {
                // Route through policy → ToggleRow (Multi) or SelectRow (Single).
                if let Some(row_id) = self.tabular.focus_state().focused().map(|c| c.row_id) {
                    let context = IntentContext {
                        focused: self.focused,
                        has_anchor: false,
                        is_editing: false,
                        focused_row: Some(row_id),
                        focused_row_expanded: false,
                    };
                    let input = IntentInput::KeyPress {
                        key: KeyCode::Space,
                        modifiers: event.modifiers,
                    };
                    let intents = self.intent_policy.interpret(&input, &context);
                    for intent in &intents {
                        self.tabular.handle_intent(intent);
                    }
                    if !intents.is_empty() {
                        self.fire_selection_changed();
                    }
                }
                true
            }

            KeyCode::Enter => {
                // Route through policy → Activate.
                if let Some(row_id) = self.tabular.focus_state().focused().map(|c| c.row_id) {
                    let context = IntentContext {
                        focused: self.focused,
                        has_anchor: false,
                        is_editing: false,
                        focused_row: Some(row_id),
                        focused_row_expanded: false,
                    };
                    let input = IntentInput::KeyPress {
                        key: KeyCode::Enter,
                        modifiers: event.modifiers,
                    };
                    let intents = self.intent_policy.interpret(&input, &context);
                    for intent in &intents {
                        if let TabularIntent::Activation(ActivationIntent::Activate {
                            row_id: act_id,
                        }) = intent
                        {
                            if let Some(idx) = self.row_id_to_index(*act_id) {
                                if let Some(callback) = &mut self.on_activate {
                                    callback(idx);
                                }
                            }
                        } else {
                            self.tabular.handle_intent(intent);
                        }
                    }
                }
                true
            }

            _ => false,
        }
    }

    fn page_size(&self) -> usize {
        if self.item_height > 0.0 && self.viewport_height > 0.0 {
            (self.viewport_height / self.item_height).floor() as usize
        } else {
            10
        }
        .max(1)
    }

    fn compute_new_focus(&self, delta: isize) -> usize {
        let last = self.item_count.saturating_sub(1);
        match self.focused_index() {
            Some(idx) => {
                if delta < 0 {
                    idx.saturating_sub((-delta) as usize)
                } else {
                    (idx + delta as usize).min(last)
                }
            }
            None => {
                if delta < 0 {
                    last
                } else {
                    0
                }
            }
        }
    }
}

// =============================================================================
// Widget Implementation
// =============================================================================

impl WidgetDowncast for List {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Widget for List {
    fn has_requested_scroll(&self) -> bool {
        self.requested_scroll_y.is_some()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // =====================================================================
        // SPEC-HAL-094/096C: Theme-driven row height synchronization.
        // Row height comes from geometry token, not hardcoded constants.
        // =====================================================================
        if self.use_theme_row_height {
            let theme_row_height = cx.theme.list_geom.row_height;
            if (self.item_height - theme_row_height).abs() > 0.001 {
                // Sync internal state to theme token.
                self.item_height = theme_row_height;
                self.tabular.set_fixed_row_height(theme_row_height);
                // Rebuild TabularSubstrate rows with theme height.
                self.tabular.clear_rows();
                for &id in &self.row_ids {
                    self.tabular.add_row_with_height(id, theme_row_height);
                }
                self.tabular
                    .set_sorted_indices((0..self.item_count).collect());
            }
        }

        // INV-LIST2-VIEWPORT-SIZED-WIDGET: Cache viewport height from constraints.
        // This is the authoritative viewport size for realization.
        if constraints.max_height.is_finite() && constraints.max_height > 0.0 {
            self.viewport_height = constraints.max_height;
        }

        let width = constraints.max_width;
        let content_height = self.item_count as f32 * self.item_height;

        // INV-LIST2-VIEWPORT-SIZED: layout_bounds = viewport size.
        // ScrollArea uses content_size (not layout_bounds) for scroll extent.
        // List is viewport-sized; ScrollArea owns scroll transform.
        self.layout_bounds = Size::new(width, self.viewport_height);

        // INV-REALIZATION-DRIVEN-BY-SCROLLAREA: Set viewport for realization.
        // Realization is driven by scroll sync (update_visible_range_from_scroll).
        // Do NOT use content_height as fallback - that kills virtualization.
        self.tabular
            .set_body_viewport(RectF::new(0.0, 0.0, width, self.viewport_height));

        // Standalone List uses the token path for realization:
        // sync_scroll stores scroll_y → layout produces token → prepare_evaluation consumes it.
        self.tabular.begin_evaluation();
        self.tabular.update_visible_range_from_scroll(self.scroll_y);
        self.tabular.prepare_evaluation(width, 1.0);

        // layout_bounds = viewport height, content_size = full content height.
        // ScrollArea uses content_size for scroll extent calculation.
        LayoutResult::new(self.layout_bounds, Size::new(width, content_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Paint background at content rect. ScrollArea clips to viewport.
        cx.scene
            .push_rect(rect, cx.theme.colors.surfaces.content, 0, None);

        // Paint realized rows.
        // INV-SCROLL-SINGLE-SOURCE: Do NOT subtract scroll_y here.
        // ScrollArea's ScrollNode already applies -scroll_y to all primitives.
        // Subtracting here would double-apply scroll.
        let selection_state = self.tabular.ordered_selection();
        let focus_state = self.tabular.focus_state();
        let list_colors = &cx.theme.colors.list;

        // INV-HOVER-IDENTITY: hovered_index is a logical index into row_ids, but
        // RowProjection.row_index is frame-local (NOT identity) — they diverge once
        // the realized window scrolls off the top. Resolve the hovered row by identity
        // (RowId), mirroring selection (row_id-based), so the highlight tracks the row
        // under the pointer at any scroll offset.
        let hovered_row_id = self
            .hovered_index
            .and_then(|i| self.row_ids.get(i).copied());

        // Pressed state darken factor (matches List v1 VirtualizedSlot).
        const PRESSED_DARKEN: f32 = 0.70;
        // Selection indicator stripe width (matches List v1).
        const SELECTION_INDICATOR_WIDTH: f32 = 2.0;

        for row_proj in self.tabular.body().realized_rows() {
            let row_y = rect.y + row_proj.y_offset;
            let row_rect = RectF::new(rect.x, row_y, rect.width, row_proj.height);

            // Determine row state for styling.
            let is_selected = selection_state.is_selected(row_proj.row_id);
            let is_hovered = Some(row_proj.row_id) == hovered_row_id;
            let is_pressed = Some(row_proj.row_index) == self.pressed_index;
            let is_focused = focus_state
                .focused()
                .map(|c| c.row_id == row_proj.row_id)
                .unwrap_or(false);

            // ===================================================================
            // Background color priority (matches List v1 VirtualizedSlot):
            //   1. selected+active → row_selected
            //   2. selected+inactive → row_selected_inactive
            //   3. hovered → row_hover
            //   4. zebra alt (odd index) → row_background_alt
            //   5. default → row_background
            //   6. pressed → darken modifier on top
            // ===================================================================
            let base_color = if is_selected {
                if self.focused {
                    list_colors.row_selected
                } else {
                    list_colors.row_selected_inactive
                }
            } else if is_hovered {
                list_colors.row_hover
            } else if self.zebra_enabled && row_proj.row_index % 2 == 1 {
                list_colors.row_background_alt
            } else {
                list_colors.row_background
            };

            // Apply pressed modifier (darken, multiplicative).
            let row_bg = if is_pressed {
                Color::from_linear(
                    base_color.r * PRESSED_DARKEN,
                    base_color.g * PRESSED_DARKEN,
                    base_color.b * PRESSED_DARKEN,
                    base_color.a,
                )
            } else {
                base_color
            };

            cx.scene.push_rect(row_rect, row_bg, 0, None);

            // Paint focus outline (always visible if item is focused, regardless of list focus).
            // Matches List v1: "Always show focused item outline, even when List doesn't have keyboard focus."
            if is_focused {
                cx.scene
                    .push_border_rect(row_rect, cx.theme.colors.borders.focus, 1.0, 1, None);
            }

            // Paint selection indicator stripe (left edge, active selection only).
            // Active selection = selected AND list has keyboard focus.
            if is_selected && self.focused {
                let stripe_rect =
                    RectF::new(rect.x, row_y, SELECTION_INDICATOR_WIDTH, row_proj.height);
                cx.scene
                    .push_rect(stripe_rect, list_colors.selection_indicator, 1, None);
            }

            // INV-PAINT-CONTENT-EXISTS: Paint deterministic per-row content.
            // This ensures rows are visually distinguishable and verifies hit-test correctness.
            let text_color = if is_selected {
                list_colors.text_selected
            } else {
                list_colors.text
            };
            let padding_x = cx.theme.list_geom.padding_x;
            let text_x = rect.x + padding_x;
            let item_text = format!("Item {}", row_proj.row_index);
            let font_size = cx.theme.typography.font_size;
            let run = cx.text_engine.shape(&item_text, font_size);
            // SPEC-HAL-092 §8A (INV-TYPO-CTRL-1): place row text from font
            // metrics, not run.height. run.height is the content-tight glyph
            // extent; centering it while the renderer anchors the baseline at
            // top + ascent drifts the text downward in the row.
            let metrics = cx.text_engine.font_metrics(font_size);
            let baseline_y = row_y
                + compute_control_text_baseline_y(
                    row_proj.height,
                    metrics.ascent,
                    metrics.descent,
                    cx.theme.typography.baseline_offset,
                );
            let text_y = baseline_y - metrics.ascent;
            cx.text_engine
                .draw(&run, text_x, text_y, text_color, 2, cx.scene);
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn needs_viewport_constraint(&self) -> bool {
        true
    }

    fn hit_testable(&self) -> bool {
        true
    }

    /// Standalone List scroll sync: stores scroll_y for the next layout pass.
    ///
    /// Called by pre_layout_sync_scroll_to_lists / post_layout_sync_scroll_to_lists.
    /// Returns true when scroll_y changed so the caller marks layout dirty,
    /// which triggers List::layout → token path → realization.
    fn sync_scroll(&mut self, scroll_y: f32) -> bool {
        if (self.scroll_y - scroll_y).abs() < 0.001 {
            return false;
        }
        self.scroll_y = scroll_y;
        true
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if self.disabled {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                let Some(rect) = cx.widget_global_rect() else {
                    return false;
                };

                // Compute viewport-local Y coordinate.
                // rect.y is the widget's layout position (does NOT include scroll offset).
                // hit_test_item() will add scroll internally via TabularSubstrate.
                let viewport_local_y = pe.global_pos.1 - rect.y;

                match pe.kind {
                    // =========================================================
                    // Move: Hover tracking + Drag selection
                    // =========================================================
                    PointerEventKind::Move => {
                        if phase == EventPhase::Capture || phase == EventPhase::Target {
                            let hit_index = self.hit_test_item(viewport_local_y);

                            // Drag selection via policy.
                            if self.drag_active {
                                if let (Some(_anchor), Some(current)) =
                                    (self.drag_anchor, hit_index)
                                {
                                    if self.drag_last_index != hit_index {
                                        if let Some(row_id) = self.index_to_row_id(current) {
                                            let context = IntentContext {
                                                focused: self.focused,
                                                has_anchor: false,
                                                is_editing: false,
                                                focused_row: self
                                                    .tabular
                                                    .focus_state()
                                                    .focused()
                                                    .map(|c| c.row_id),
                                                focused_row_expanded: false,
                                            };
                                            let input = IntentInput::DragMove { target: row_id };
                                            let intents =
                                                self.intent_policy.interpret(&input, &context);
                                            if !intents.is_empty() {
                                                for intent in &intents {
                                                    self.tabular.handle_intent(intent);
                                                }
                                                self.set_focus(current);
                                                self.fire_selection_changed();
                                            }
                                        }
                                        self.drag_last_index = hit_index;
                                    }
                                }
                            }

                            if self.hovered_index != hit_index {
                                self.hovered_index = hit_index;
                                // Demand-driven renderer: per-row hover is paint-only
                                // state; without a redraw request row_hover never paints.
                                cx.invalidate_layout();
                            }
                        }
                        false
                    }

                    // =========================================================
                    // Down: Pressed tracking + Drag start (SPEC-HAL-200)
                    // =========================================================
                    PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                        if phase == EventPhase::Capture {
                            // Lock interaction model on first user input affecting selection/focus.
                            self.tabular.lock_interaction_model();

                            let hit_index = self.hit_test_item(viewport_local_y);
                            self.pressed_index = hit_index;

                            if let Some(idx) = hit_index {
                                if let Some(row_id) = self.index_to_row_id(idx) {
                                    let context = IntentContext {
                                        focused: self.focused,
                                        has_anchor: false,
                                        is_editing: false,
                                        focused_row: self
                                            .tabular
                                            .focus_state()
                                            .focused()
                                            .map(|c| c.row_id),
                                        focused_row_expanded: false,
                                    };
                                    let input = IntentInput::Click {
                                        target: row_id,
                                        modifiers: pe.modifiers,
                                        is_subcontrol: false,
                                    };
                                    let intents = self.intent_policy.interpret(&input, &context);

                                    let is_range = intents.iter().any(|i| {
                                        matches!(
                                            i,
                                            TabularIntent::Selection(
                                                SelectionIntent::ExtendToRow { .. }
                                            )
                                        )
                                    });

                                    if is_range {
                                        // Commit the range immediately on press,
                                        // then arm the drag so a subsequent move
                                        // keeps extending from the same anchor.
                                        for intent in &intents {
                                            self.tabular.handle_intent(intent);
                                        }
                                        self.set_focus(idx);
                                        self.fire_selection_changed();
                                        self.drag_active = true;
                                        self.drag_anchor = Some(idx);
                                        self.drag_last_index = Some(idx);
                                    } else {
                                        let is_toggle = intents.iter().any(|i| {
                                            matches!(
                                                i,
                                                TabularIntent::Selection(
                                                    SelectionIntent::ToggleRow { .. }
                                                )
                                            )
                                        });

                                        if self.is_transactional() && !is_toggle {
                                            // Transactional + non-toggle = focus only.
                                            self.set_focus(idx);
                                        } else {
                                            for intent in &intents {
                                                self.tabular.handle_intent(intent);
                                            }
                                            self.set_focus(idx);
                                            self.fire_selection_changed();
                                            if is_toggle {
                                                self.ctrl_click_handled_index = Some(idx);
                                            }
                                        }
                                    }
                                }
                            } else {
                                self.finalize_range_selection();
                            }

                            if self.hovered_index != hit_index {
                                self.hovered_index = hit_index;
                            }
                        }
                        false
                    }

                    // =========================================================
                    // Up: Drag end + Activation (SPEC-HAL-200)
                    // =========================================================
                    PointerEventKind::Up if pe.button == Some(PointerButton::Primary) => {
                        if phase == EventPhase::Capture || phase == EventPhase::Target {
                            if self.drag_active {
                                // Range gesture: selection was committed on press
                                // and during any drag-move. Just clear drag state.
                                self.pressed_index = None;
                                self.drag_active = false;
                                self.drag_anchor = None;
                                self.drag_last_index = None;

                                // Range gestures never activate.
                                return true;
                            }
                        }

                        // Empty space click.
                        if phase == EventPhase::Target {
                            if viewport_local_y >= self.layout_bounds.height || self.item_count == 0
                            {
                                if !self.is_transactional() {
                                    // Route through policy for ClearSelection.
                                    let context = IntentContext {
                                        focused: self.focused,
                                        has_anchor: false,
                                        is_editing: false,
                                        focused_row: None,
                                        focused_row_expanded: false,
                                    };
                                    let input = IntentInput::KeyPress {
                                        key: KeyCode::Escape,
                                        modifiers: Modifiers::default(),
                                    };
                                    let intents = self.intent_policy.interpret(&input, &context);
                                    for intent in &intents {
                                        self.tabular.handle_intent(intent);
                                    }
                                    self.fire_selection_changed();
                                }
                                return true;
                            }
                        }
                        false
                    }

                    _ => false,
                }
            }

            Event::Key(ke) if phase == EventPhase::Target => self.handle_keyboard(ke),

            _ => false,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        self.pressed_index = None;
        self.finalize_range_selection();
    }

    fn accepts_keyboard_activation(&self) -> bool {
        !self.disabled
    }

    fn on_keyboard_activate(&mut self, _cx: &mut EventContext) {
        if let Some(idx) = self.focused_index() {
            if let Some(callback) = &mut self.on_activate {
                callback(idx);
            }
        }
    }

    fn handle_projected_action(
        &mut self,
        _cx: &mut EventContext,
        _logical_id: LogicalItemId,
        logical_index: usize,
        action: ProjectedAction,
    ) -> bool {
        if self.disabled || logical_index >= self.item_count {
            return false;
        }

        // Lock interaction model on first user input affecting selection/focus.
        self.tabular.lock_interaction_model();

        match action {
            ProjectedAction::Select { shift, ctrl_or_cmd } => {
                self.pressed_index = Some(logical_index);
                self.hovered_index = Some(logical_index);

                if let Some(row_id) = self.index_to_row_id(logical_index) {
                    let modifiers = Modifiers {
                        shift,
                        ctrl: ctrl_or_cmd,
                        alt: false,
                        meta: false,
                    };
                    let context = IntentContext {
                        focused: self.focused,
                        has_anchor: false,
                        is_editing: false,
                        focused_row: self.tabular.focus_state().focused().map(|c| c.row_id),
                        focused_row_expanded: false,
                    };
                    let input = IntentInput::Click {
                        target: row_id,
                        modifiers,
                        is_subcontrol: false,
                    };
                    let intents = self.intent_policy.interpret(&input, &context);

                    let is_range = intents.iter().any(|i| {
                        matches!(
                            i,
                            TabularIntent::Selection(SelectionIntent::ExtendToRow { .. })
                        )
                    });
                    let is_toggle = intents.iter().any(|i| {
                        matches!(
                            i,
                            TabularIntent::Selection(SelectionIntent::ToggleRow { .. })
                        )
                    });

                    if is_range {
                        // Commit the range immediately, then arm the drag so a
                        // subsequent move keeps extending from the same anchor.
                        for intent in &intents {
                            self.tabular.handle_intent(intent);
                        }
                        self.set_focus(logical_index);
                        self.fire_selection_changed();
                        self.drag_active = true;
                        self.drag_anchor = Some(logical_index);
                        self.drag_last_index = Some(logical_index);
                    } else if is_toggle {
                        if self.ctrl_click_handled_index == Some(logical_index) {
                            self.ctrl_click_handled_index = None;
                        } else {
                            for intent in &intents {
                                self.tabular.handle_intent(intent);
                            }
                            self.set_focus(logical_index);
                            self.fire_selection_changed();
                        }
                    } else if self.is_transactional() {
                        self.set_focus(logical_index);
                    } else {
                        for intent in &intents {
                            self.tabular.handle_intent(intent);
                        }
                        self.set_focus(logical_index);
                        self.fire_selection_changed();
                    }
                }
                true
            }
            ProjectedAction::Focus => {
                self.set_focus(logical_index);
                true
            }
            ProjectedAction::Activate => {
                if let Some(row_id) = self.index_to_row_id(logical_index) {
                    let context = IntentContext {
                        focused: self.focused,
                        has_anchor: false,
                        is_editing: false,
                        focused_row: self.tabular.focus_state().focused().map(|c| c.row_id),
                        focused_row_expanded: false,
                    };
                    let input = IntentInput::Click {
                        target: row_id,
                        modifiers: Modifiers::default(),
                        is_subcontrol: false,
                    };
                    let intents = self.intent_policy.interpret(&input, &context);
                    for intent in &intents {
                        self.tabular.handle_intent(intent);
                    }
                    self.set_focus(logical_index);
                    self.fire_selection_changed();
                }
                // Fire activation callback.
                if let Some(idx) = self.focused_index() {
                    if let Some(callback) = &mut self.on_activate {
                        callback(idx);
                    }
                }
                true
            }
        }
    }

    fn as_anchor_resolver(&self) -> Option<&dyn AnchorResolver> {
        if self.item_count > 0 {
            Some(self)
        } else {
            None
        }
    }

    fn as_input_projection_provider(&self) -> Option<&dyn InputProjectionProvider> {
        if self.item_count > 0 {
            Some(self)
        } else {
            None
        }
    }

    fn debug_type_name(&self) -> &'static str {
        "List"
    }
}

impl AnchorResolver for List {
    fn resolve_anchor(&self, anchor: &ScrollAnchor) -> Option<Point> {
        let index = anchor.target_id.0 as usize;
        if index >= self.item_count || self.item_height <= 0.0 {
            return None;
        }
        let y = index as f32 * self.item_height + anchor.target_offset.y;
        Some(Point::new(0.0, y))
    }

    fn find_nearest_anchor(&self, anchor: &ScrollAnchor) -> Option<ScrollAnchor> {
        if self.item_count == 0 {
            return None;
        }
        let index = anchor.target_id.0 as usize;
        let clamped = index.min(self.item_count - 1);
        if clamped == index {
            return None;
        }
        Some(ScrollAnchor::new(
            AnchorId::new(clamped as u64),
            anchor.target_offset,
            anchor.viewport_offset,
        ))
    }
}

impl InputProjectionProvider for List {
    fn logical_item_count(&self) -> usize {
        self.item_count
    }

    fn logical_item_bounds(&self, index: usize) -> Option<RectF> {
        if index >= self.item_count || self.item_height <= 0.0 {
            return None;
        }
        let y = index as f32 * self.item_height;
        Some(RectF::new(
            0.0,
            y,
            self.layout_bounds.width,
            self.item_height,
        ))
    }

    fn logical_item_id(&self, index: usize) -> Option<LogicalItemId> {
        if index < self.item_count {
            Some(LogicalItemId::new(index as u64 + 1))
        } else {
            None
        }
    }

    fn realized_widget_for_item(&self, _index: usize) -> Option<crate::utils::id::TreeWidgetId> {
        // List delegates to TabularSubstrate; no direct realized widgets.
        None
    }

    fn realized_range(&self) -> Range<usize> {
        // All items are conceptually realized (TabularSubstrate handles virtualization).
        self.visible_range.clone()
    }

    fn content_width(&self) -> f32 {
        self.layout_bounds.width
    }
}
