// =============================================================================
// ListViewCore: shared ListView state
// =============================================================================

use std::cell::{Cell as StdCell, Ref};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::core::ordered_selection::{
    OrderedIndex, OrderedSelectionAction, OrderedSelectionState, SelectionMode,
    reduce_ordered_selection,
};
use crate::core::scroll_container::HorizontalScrollLink;
use crate::core::widget::Widget;
use crate::engines::cell_mechanics::{
    CellMechanicsSubstrate, CompositeChoiceAdmission, CompositeChoiceManifest,
    CompositeChoiceMountFacts, CompositeChoiceReadiness, CompositeChoiceRoleKey, CompositeChoiceWork,
};
use crate::engines::cell_source::{Cell as DataCell, ComboBoxCellEditorSpec, PropertyValue};
use crate::engines::hierarchical::{NodeMeta, VisibleRow};
use crate::model::tabular::column::{ColumnModel, ColumnScrollDomain};
use crate::model::tabular::row_heights::RowHeights;
use crate::utils::id::TreeWidgetId;
use crate::widgets::cell_editor_factory::DefaultCellEditorFactory;
use crate::widgets::tabular_view::column::{Column, ColumnId};
use crate::widgets::tabular_view::column_config::{ColumnConfigurationState, ColumnMetaSnapshot};
use crate::widgets::tabular_view::data_adapter::{
    AppliedSortSnapshot, CellValue, DataAdapter, DataSource, Extent, SortCapability, SortKey,
    SortOperation, SortRequestOutcome, ValueState,
};
use crate::widgets::tabular_view::intent_policy::TabularIntentPolicy;
use crate::widgets::tabular_view::intents::{
    ActivationIntent, ExpansionIntent, FocusIntent, ReorderIntent, ResizeIntent, SelectionIntent,
    TabularIntent, VisibilityIntent, WidthIntent,
};
pub use crate::widgets::tabular_view::row::RowId;
use crate::widgets::tabular_view::row::RowProjection;
use crate::widgets::tabular_view::view_config::{self, ViewConfigKey, ViewConfigStore};

use super::cell_painter::{CellPainter, DefaultCellPainter};

// =============================================================================
// Resize State Machine (SPEC-HAL-192)
// =============================================================================

/// Column resize state machine state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeState {
    /// No resize in progress.
    Idle,
    /// PointerDown detected on separator, waiting for first move.
    Armed {
        /// Column whose trailing separator is being dragged.
        column_id: ColumnId,
        /// Global X coordinate for the active drag baseline.
        start_x: f32,
        /// Column width for the active drag baseline.
        initial_width: f32,
        /// Original column width at arm, used for cancellation rollback.
        rollback_width: f32,
    },
    /// Active resize drag with pointer capture.
    Dragging {
        /// Column whose trailing separator is being dragged.
        column_id: ColumnId,
        /// Global X coordinate for the active drag baseline.
        start_x: f32,
        /// Column width for the active drag baseline.
        initial_width: f32,
        /// Original column width at arm, used for cancellation rollback.
        rollback_width: f32,
    },
}

impl Default for ResizeState {
    fn default() -> Self {
        Self::Idle
    }
}

struct ListViewSelectionIndex<'a, S: DataSource> {
    row_ids: &'a [RowId],
    adapter: &'a DataAdapter<S>,
    row_count: usize,
}

impl<S: DataSource> OrderedIndex<RowId> for ListViewSelectionIndex<'_, S> {
    fn len(&self) -> usize {
        self.row_count
    }

    fn key_at(&self, index: usize) -> RowId {
        if !self.row_ids.is_empty() {
            return self.row_ids[index];
        }
        match self.adapter.row_id_at(index) {
            ValueState::Ready(Some(row_id)) => row_id,
            _ => panic!("order window unavailable for accepted selection range"),
        }
    }

    fn index_of(&self, key: RowId) -> Option<usize> {
        if !self.row_ids.is_empty() {
            return self.row_ids.iter().position(|row_id| *row_id == key);
        }
        match self.adapter.locate_row(key) {
            ValueState::Ready(index) => index,
            _ => None,
        }
    }
}

/// Core state shared between ListView and ListViewBody.
pub struct ListViewCore<S: DataSource> {
    /// Cell mechanics substrate. HTS inside CMS owns geometry, rows, columns, and expansion.
    pub cms: CellMechanicsSubstrate,

    /// Data adapter (SPEC-HAL-188 integration seam).
    pub adapter: DataAdapter<S>,

    /// Column titles (presentation layer, keyed by ColumnId).
    pub column_titles: HashMap<ColumnId, String>,

    /// Item height (uniform collapsed strip height).
    pub item_height: f32,

    /// Row IDs for index-to-RowId mapping.
    /// Empty for canonical windowed providers.
    row_ids: Vec<RowId>,

    /// Provider extent projected as the current scrollable row count.
    row_count: usize,

    /// ListView-owned row-height oracle mirrored into HTS.
    row_heights: RowHeights,

    /// ListView-owned selection/focus state.
    pub selection: OrderedSelectionState<RowId>,

    /// Focused column used when emitting cell-focus shaped intents.
    focused_column: Option<ColumnId>,

    /// Widget has keyboard focus.
    pub focused: bool,

    /// Requested scroll Y for focus reveal.
    pub requested_scroll_y: Option<f32>,

    /// Selection mode.
    selection_mode: SelectionMode,

    /// Viewport height (cached for page size calculation).
    pub viewport_height: f32,

    /// Currently hovered column separator.
    pub hovered_separator: Option<ColumnId>,

    /// Currently resizing column separator.
    pub active_resize_separator: Option<ColumnId>,

    /// Body widget's TreeWidgetId for layout invalidation.
    pub body_widget_id: Option<TreeWidgetId>,

    /// Horizontal scroll link for header/footer alignment with body scroll.
    pub horizontal_scroll_link: Option<HorizontalScrollLink>,

    /// Horizontal scroll freeze flag for column resize.
    pub horizontal_scroll_freeze: Option<Rc<StdCell<bool>>>,

    /// Declarative intent policy for interaction interpretation.
    pub intent_policy: TabularIntentPolicy,

    /// View ID for persistence. None = persistence disabled.
    pub view_id: Option<String>,

    /// App namespace for persistence scoping.
    pub app_namespace: Option<String>,

    /// Persistence backend. None = persistence disabled.
    pub config_store: Option<Box<dyn ViewConfigStore>>,

    /// Baseline column configuration.
    pub default_column_config: Option<ColumnConfigurationState>,

    /// Whether the column config overlay is open.
    pub config_overlay_open: bool,

    /// Column currently being drag-reordered.
    pub dragged_column_id: Option<ColumnId>,

    /// Visual X offsets for live column sliding during drag-reorder.
    pub drag_visual_offsets: Option<HashMap<ColumnId, f32>>,

    /// Ghost column occlusion range in slot/content-space X coordinates.
    pub ghost_occlude_range: Option<(f32, f32)>,

    /// Custom cell painter. Default = text rendering.
    pub cell_painter: Box<dyn CellPainter>,

    /// True after a consumer installs a custom cell painter.
    pub custom_cell_painter: bool,

    /// SPEC-HAL-167: Cached SPEC-HAL-188 cell values for custom painters.
    realized_cell_values: HashMap<(u64, ColumnId), ValueState<CellValue>>,

    /// Columns configured to mount inline ComboBox editors for string values.
    pub combo_box_options: HashMap<ColumnId, Vec<(String, String)>>,

    /// Activation callback.
    pub activation_callback: Option<Rc<dyn Fn(RowId)>>,

    /// Row-axis focus callback.
    pub focused_row_callback: Option<Rc<dyn Fn(Option<RowId>)>>,

    /// Cached scale factor for physical pixel snapping.
    pub scale_factor: f32,

    /// Whether pointer position is within a row expansion region.
    pub pointer_in_expansion_region: bool,

    /// Pending row widget factory.
    pub row_widget_factory: Option<Box<dyn Fn(RowId) -> Box<dyn Widget>>>,
}

impl<S: DataSource> ListViewCore<S> {
    pub fn new(source: S) -> Self {
        let mut adapter = DataAdapter::new(source);
        let row_ids = if adapter.uses_windowed_order() {
            Vec::new()
        } else {
            adapter.fetch_row_ids().to_vec()
        };
        let row_count = Self::row_count_from_extent(adapter.extent(), row_ids.len());
        let item_height = 24.0;
        let row_heights = RowHeights::with_fixed_height_count(item_height, row_count);
        let mut cms = CellMechanicsSubstrate::new(std::rc::Rc::new(std::cell::RefCell::new(
            crate::engines::hierarchical::HierarchicalTabularSubstrate::new(),
        )));
        cms.set_editor_factory(Box::new(DefaultCellEditorFactory::new()));
        cms.hts_mut().set_row_heights(row_heights.clone());
        if !row_ids.is_empty() {
            cms.hts_mut()
                .set_tree_data(Self::flat_nodes_from_row_ids(&row_ids));
        }

        Self {
            cms,
            adapter,
            column_titles: HashMap::new(),
            item_height,
            row_ids,
            row_count,
            row_heights,
            selection: OrderedSelectionState::new(SelectionMode::Multi),
            focused_column: None,
            focused: false,
            requested_scroll_y: None,
            selection_mode: SelectionMode::Multi,
            viewport_height: 0.0,
            hovered_separator: None,
            active_resize_separator: None,
            body_widget_id: None,
            horizontal_scroll_link: None,
            horizontal_scroll_freeze: None,
            intent_policy: TabularIntentPolicy::multi(),
            view_id: None,
            app_namespace: None,
            config_store: None,
            default_column_config: None,
            config_overlay_open: false,
            dragged_column_id: None,
            drag_visual_offsets: None,
            ghost_occlude_range: None,
            cell_painter: Box::new(DefaultCellPainter),
            custom_cell_painter: false,
            realized_cell_values: HashMap::new(),
            combo_box_options: HashMap::new(),
            activation_callback: None,
            focused_row_callback: None,
            scale_factor: 1.0,
            pointer_in_expansion_region: false,
            row_widget_factory: None,
        }
    }

    fn row_count_from_extent(extent: Extent, legacy_len: usize) -> usize {
        match extent {
            Extent::Exact(count) | Extent::LowerBound(count) | Extent::UpperBound(count) => count,
            Extent::Bounded { lower, .. } => lower,
            Extent::Unknown | Extent::Streaming => legacy_len,
        }
    }

    fn flat_nodes_from_row_ids(row_ids: &[RowId]) -> Vec<NodeMeta> {
        row_ids
            .iter()
            .map(|row_id| NodeMeta {
                node_id: row_id.raw(),
                parent_index: None,
                depth: 0,
                has_children: false,
                label: String::new(),
            })
            .collect()
    }

    fn rebuild_flat_topology(&mut self) {
        self.cms.hts_mut().set_tree_data(Vec::new());
        self.cms.hts_mut().set_row_heights(self.row_heights.clone());
        self.cms
            .hts_mut()
            .set_tree_data(Self::flat_nodes_from_row_ids(&self.row_ids));
    }

    /// Add a column and return its ID.
    pub fn add_column(&mut self, title: &str, width: f32) -> ColumnId {
        let column = Column::new(width);
        self.add_column_raw(title, column)
    }

    /// Add a pre-built Column and return its ID.
    pub fn add_column_raw(&mut self, title: &str, column: Column) -> ColumnId {
        let column_id = column.id;
        self.cms.hts_mut().add_column(column);
        if self.focused_column.is_none() {
            self.focused_column = Some(column_id);
        }
        self.column_titles.insert(column_id, title.to_string());
        column_id
    }

    pub fn set_column_combo_options(&mut self, id: ColumnId, options: Vec<(String, String)>) {
        self.combo_box_options.insert(id, options);
    }

    /// Get total row count.
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// Get selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection_mode
    }

    /// Set selection mode.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
        self.selection.selection_mode = mode;
    }

    pub fn set_intent_policy(&mut self, policy: TabularIntentPolicy) -> bool {
        if self.selection.is_interaction_model_locked() {
            return self.intent_policy == policy;
        }

        self.set_selection_mode(policy.selection_mode);
        self.selection
            .set_interaction_model(policy.interaction_model);
        self.intent_policy = policy;
        true
    }

    pub fn index_to_row_id(&self, index: usize) -> Option<RowId> {
        if !self.row_ids.is_empty() {
            return self.row_ids.get(index).copied();
        }
        match self.adapter.row_id_at(index) {
            ValueState::Ready(row_id) => row_id,
            _ => None,
        }
    }

    pub fn row_id_to_index(&self, row_id: RowId) -> Option<usize> {
        if !self.row_ids.is_empty() {
            return self.row_ids.iter().position(|&id| id == row_id);
        }
        match self.adapter.locate_row(row_id) {
            ValueState::Ready(index) => index,
            _ => None,
        }
    }

    pub fn first_column_id(&self) -> Option<ColumnId> {
        self.cms
            .hts()
            .column_model()
            .columns()
            .first()
            .map(|c| c.id)
    }

    pub fn focused_row(&self) -> Option<RowId> {
        self.selection.focus_key
    }

    pub fn focused_index(&self) -> Option<usize> {
        self.selection
            .focus_key
            .and_then(|row_id| self.row_id_to_index(row_id))
    }

    pub fn is_row_focused(&self, row_id: RowId) -> bool {
        self.selection.focus_key == Some(row_id)
    }

    pub fn is_selected(&self, row_id: RowId) -> bool {
        self.selection.is_selected(row_id)
    }

    pub fn set_focus(&mut self, index: usize) {
        if let Some(row_id) = self.index_to_row_id(index) {
            self.dispatch_selection_action(OrderedSelectionAction::FocusTo { key: row_id });
        }
    }

    pub fn set_focus_by_row_id(&mut self, row_id: RowId) {
        self.dispatch_selection_action(OrderedSelectionAction::FocusTo { key: row_id });
    }

    pub fn action_click(&mut self, index: usize) {
        if let Some(row_id) = self.index_to_row_id(index) {
            self.action_select_row(row_id);
        }
    }

    pub fn action_ctrl_click(&mut self, index: usize) {
        if let Some(row_id) = self.index_to_row_id(index) {
            self.action_toggle_row(row_id);
        }
    }

    pub fn action_shift_click(&mut self, index: usize) {
        if self.selection_mode == SelectionMode::Single {
            self.action_click(index);
            return;
        }
        if let Some(row_id) = self.index_to_row_id(index) {
            self.action_extend_to_row(row_id);
        }
    }

    pub fn action_drag_select(&mut self, index: usize) {
        if self.selection_mode != SelectionMode::Multi {
            return;
        }
        if let Some(row_id) = self.index_to_row_id(index) {
            self.action_extend_to_row(row_id);
        }
    }

    pub fn action_select_row(&mut self, row_id: RowId) {
        self.dispatch_selection_action(OrderedSelectionAction::Select { key: row_id });
    }

    pub fn action_extend_to_row(&mut self, row_id: RowId) {
        if self.selection_mode == SelectionMode::Single {
            self.action_select_row(row_id);
            return;
        }
        self.dispatch_selection_action(OrderedSelectionAction::ExtendTo { key: row_id });
    }

    pub fn action_toggle_row(&mut self, row_id: RowId) {
        self.dispatch_selection_action(OrderedSelectionAction::Toggle { key: row_id });
    }

    pub fn page_size(&self) -> usize {
        if self.item_height > 0.0 && self.viewport_height > 0.0 {
            (self.viewport_height / self.item_height).floor() as usize
        } else {
            10
        }
        .max(1)
    }

    pub fn compute_new_focus(&self, delta: isize) -> usize {
        let last = self.row_count().saturating_sub(1);
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

    pub fn handle_nav_up(&mut self, shift: bool, ctrl: bool) {
        if shift && self.selection_mode == SelectionMode::Multi {
            self.handle_extend_direction(-1);
        } else if ctrl {
            self.handle_move_focus_direction(-1);
        } else {
            self.handle_focus_direction(-1);
        }
    }

    pub fn handle_nav_down(&mut self, shift: bool, ctrl: bool) {
        if shift && self.selection_mode == SelectionMode::Multi {
            self.handle_extend_direction(1);
        } else if ctrl {
            self.handle_move_focus_direction(1);
        } else {
            self.handle_focus_direction(1);
        }
    }

    pub fn handle_nav_home(&mut self, shift: bool, ctrl: bool) {
        if shift && self.selection_mode == SelectionMode::Multi {
            self.handle_extend_to(0);
        } else if ctrl {
            self.handle_move_focus_to(0);
        } else {
            self.handle_focus_to(0);
        }
    }

    pub fn handle_nav_end(&mut self, shift: bool, ctrl: bool) {
        let last = self.row_count().saturating_sub(1);
        if shift && self.selection_mode == SelectionMode::Multi {
            self.handle_extend_to(last);
        } else if ctrl {
            self.handle_move_focus_to(last);
        } else {
            self.handle_focus_to(last);
        }
    }

    pub fn handle_nav_page_up(&mut self, shift: bool, ctrl: bool) {
        let page = self.page_size();
        let current = self
            .focused_index()
            .unwrap_or(self.row_count().saturating_sub(1));
        let target = current.saturating_sub(page);
        if shift && self.selection_mode == SelectionMode::Multi {
            self.handle_extend_to(target);
        } else if ctrl {
            self.handle_move_focus_to(target);
        } else {
            self.handle_focus_to(target);
        }
    }

    pub fn handle_nav_page_down(&mut self, shift: bool, ctrl: bool) {
        let page = self.page_size();
        let current = self.focused_index().unwrap_or(0);
        let last = self.row_count().saturating_sub(1);
        let target = (current + page).min(last);
        if shift && self.selection_mode == SelectionMode::Multi {
            self.handle_extend_to(target);
        } else if ctrl {
            self.handle_move_focus_to(target);
        } else {
            self.handle_focus_to(target);
        }
    }

    fn handle_focus_direction(&mut self, delta: isize) {
        let new_focus = self.compute_new_focus(delta);
        if let Some(row_id) = self.index_to_row_id(new_focus) {
            if self.selection_mode == SelectionMode::None {
                self.dispatch_selection_action(OrderedSelectionAction::FocusTo { key: row_id });
            } else {
                self.dispatch_selection_action(OrderedSelectionAction::Select { key: row_id });
            }
        }
    }

    fn handle_move_focus_direction(&mut self, delta: isize) {
        let new_focus = self.compute_new_focus(delta);
        self.set_focus(new_focus);
    }

    fn handle_focus_to(&mut self, target: usize) {
        let target = target.min(self.row_count().saturating_sub(1));
        if let Some(row_id) = self.index_to_row_id(target) {
            if self.selection_mode == SelectionMode::None {
                self.dispatch_selection_action(OrderedSelectionAction::FocusTo { key: row_id });
            } else {
                self.dispatch_selection_action(OrderedSelectionAction::Select { key: row_id });
            }
        }
    }

    fn handle_move_focus_to(&mut self, target: usize) {
        let target = target.min(self.row_count().saturating_sub(1));
        self.set_focus(target);
    }

    fn handle_extend_direction(&mut self, delta: isize) {
        if self.row_count() == 0 {
            return;
        }
        let new_focus = self.compute_new_focus(delta);
        if let Some(row_id) = self.index_to_row_id(new_focus) {
            self.dispatch_selection_action(OrderedSelectionAction::ExtendTo { key: row_id });
        }
    }

    fn handle_extend_to(&mut self, target: usize) {
        if self.row_count() == 0 {
            return;
        }
        let target = target.min(self.row_count().saturating_sub(1));
        if let Some(row_id) = self.index_to_row_id(target) {
            self.dispatch_selection_action(OrderedSelectionAction::ExtendTo { key: row_id });
        }
    }

    pub fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.requested_scroll_y.take()
    }

    pub fn build_column_snapshot(&self) -> Vec<ColumnMetaSnapshot> {
        self.cms
            .hts()
            .column_model()
            .columns()
            .iter()
            .enumerate()
            .map(|(i, col)| ColumnMetaSnapshot {
                id: col.id,
                title: self.column_titles.get(&col.id).cloned().unwrap_or_default(),
                visible: col.visible,
                domain: col.scroll_domain,
                order_index: i,
            })
            .collect()
    }

    pub fn view_config_key(&self) -> Option<ViewConfigKey> {
        let view_id = self.view_id.as_ref()?;
        if view_id.is_empty() {
            return None;
        }
        Some(ViewConfigKey {
            app_namespace: self
                .app_namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            widget_type: "list_view".to_string(),
            view_id: view_id.clone(),
        })
    }

    /// Mapped guard over the column model (SPEC-HAL-337 INV-337-A1:
    /// frame-ephemeral; derive and drop within the call site's statement
    /// or scope).
    pub fn column_model(&self) -> Ref<'_, ColumnModel> {
        Ref::map(self.cms.hts(), |h| h.column_model())
    }

    pub fn set_column_visible(&mut self, id: ColumnId, visible: bool) -> bool {
        self.cms.hts_mut().set_column_visible(id, visible)
    }

    pub fn toggle_column_visible(&mut self, id: ColumnId) -> bool {
        self.cms.hts_mut().toggle_column_visible(id)
    }

    pub fn is_column_visible(&self, id: ColumnId) -> bool {
        self.cms.hts().is_column_visible(id)
    }

    pub fn set_column_scroll_domain(&mut self, id: ColumnId, domain: ColumnScrollDomain) -> bool {
        self.cms.hts_mut().set_column_scroll_domain(id, domain)
    }

    pub fn reorder_columns(&mut self, new_order: &[ColumnId]) -> bool {
        self.cms.hts_mut().reorder_columns(new_order)
    }

    pub fn set_column_width(&mut self, id: ColumnId, width: f32) {
        self.cms.hts_mut().set_column_width(id, width);
    }

    pub fn commit_user_resize(&mut self, id: ColumnId, proposed_width: f32) -> Option<f32> {
        self.cms.hts_mut().commit_user_resize(id, proposed_width)
    }

    pub fn apply_column_configuration(&mut self, config: &ColumnConfigurationState) -> bool {
        let live_ids: Vec<ColumnId> = self
            .cms
            .hts()
            .column_model()
            .columns()
            .iter()
            .map(|c| c.id)
            .collect();
        let live_set: HashSet<ColumnId> = live_ids.iter().copied().collect();
        let order_valid = config.column_order.len() == live_ids.len()
            && config.column_order.iter().all(|id| live_set.contains(id));
        let mut changed = false;

        if order_valid {
            changed |= self.reorder_columns(&config.column_order);
        }

        for id in live_ids {
            if let Some(width) = config.preferred_width_for(id)
                && width.is_finite()
                && width > 0.0
            {
                let target = self.column_model().get(id).map(|column| {
                    width
                        .max(
                            column
                                .min_width
                                .max(crate::model::tabular::column::MIN_COL_WIDTH),
                        )
                        .min(column.max_width)
                });
                if let Some(target) = target {
                    let current = self
                        .column_model()
                        .get(id)
                        .map(|column| column.preferred_width);
                    if current != Some(target) {
                        self.set_column_width(id, target);
                        changed = true;
                    }
                }
            }
            changed |= self.set_column_visible(id, config.is_visible(id));
            changed |= self.set_column_scroll_domain(id, config.domain_for(id));
        }

        changed
    }

    pub fn content_width(&self) -> f32 {
        self.cms.hts().column_layout().total_width()
    }

    pub fn content_height(&self) -> f32 {
        if let Some(h) = self.row_heights.fixed_height() {
            h * self.row_count as f32
        } else {
            (0..self.row_count)
                .map(|index| self.row_heights.height_at(index))
                .sum()
        }
    }

    pub fn row_height_at_index(&self, index: usize) -> f32 {
        self.row_heights.height_at(index)
    }

    pub fn row_top_at_index(&self, index: usize) -> f32 {
        if let Some(h) = self.row_heights.fixed_height() {
            index as f32 * h
        } else {
            (0..index).map(|i| self.row_heights.height_at(i)).sum()
        }
    }

    pub fn set_row_height_at(&mut self, row_id: RowId, height: f32) {
        let Some(index) = self.row_id_to_index(row_id) else {
            return;
        };
        if self.row_heights.fixed_height().is_some() {
            self.row_heights.promote_to_variable();
        }
        self.row_heights.set_height(index, height);
        self.cms.hts_mut().set_row_heights(self.row_heights.clone());
    }

    pub fn project_visible_rows(&self, visible_rows: &[VisibleRow]) -> Vec<RowProjection> {
        visible_rows
            .iter()
            .map(|vr| {
                let row_id = RowId::from_raw(vr.node_id);
                RowProjection::new(
                    row_id,
                    vr.row_index,
                    self.row_top_at_index(vr.row_index),
                    self.row_height_at_index(vr.row_index),
                )
            })
            .collect()
    }

    pub fn visible_row_projections(&self) -> Vec<RowProjection> {
        // SPEC-PM-017: f32 shim — rung 2.
        let scroll_y = self.cms.hts().scroll_offset() as f32;
        let viewport_bottom = scroll_y + self.viewport_height;
        let mut projections = Vec::new();
        let candidates: Vec<(usize, RowId)> = if !self.row_ids.is_empty() {
            self.row_ids.iter().copied().enumerate().collect()
        } else {
            self.adapter
                .cached_order_window()
                .map(|window| {
                    window
                        .row_ids
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(offset, row_id)| (window.start + offset, row_id))
                        .collect()
                })
                .unwrap_or_default()
        };
        for (index, row_id) in candidates {
            let y_offset = self.row_top_at_index(index);
            let height = self.row_height_at_index(index);
            if y_offset + height < scroll_y {
                continue;
            }
            if y_offset > viewport_bottom {
                break;
            }
            projections.push(RowProjection::new(row_id, index, y_offset, height));
        }
        projections
    }

    /// Resolve the current viewport to provider identities without acquiring
    /// row-order authority or materializing the complete universe.
    pub fn visible_rows_for_viewport(&mut self, viewport_height: f32) -> Vec<VisibleRow> {
        if !self.adapter.uses_windowed_order() {
            let (visible, _, _) = self.cms.hts_mut().visible_rows_with_total();
            return visible;
        }

        if self.row_count == 0 || viewport_height <= 0.0 {
            return Vec::new();
        }

        let scroll_y = self.cms.hts().scroll_offset().max(0.0);
        let (start, end) = if let Some(height) = self.row_heights.fixed_height() {
            let first = (scroll_y / height as f64).floor() as usize;
            let visible_count = (viewport_height / height).ceil() as usize;
            let start = first.saturating_sub(2).min(self.row_count);
            let end = first
                .saturating_add(visible_count)
                .saturating_add(3)
                .min(self.row_count);
            (start, end)
        } else {
            let mut y = 0.0f64;
            let mut start = 0usize;
            while start < self.row_count && y + self.row_heights.height_at(start) as f64 <= scroll_y
            {
                y += self.row_heights.height_at(start) as f64;
                start += 1;
            }
            let mut end = start;
            let viewport_bottom = scroll_y + viewport_height as f64;
            while end < self.row_count && y < viewport_bottom {
                y += self.row_heights.height_at(end) as f64;
                end += 1;
            }
            (
                start.saturating_sub(2),
                end.saturating_add(2).min(self.row_count),
            )
        };

        let length = end.saturating_sub(start);
        let ValueState::Ready(window) = self.adapter.fetch_order_window(start, length) else {
            return Vec::new();
        };

        window
            .row_ids
            .into_iter()
            .enumerate()
            .map(|(offset, row_id)| VisibleRow {
                node_id: row_id.raw(),
                row_index: window.start + offset,
                depth: 0,
                expanded: false,
                has_children: false,
                guide_mask: 0,
                stem_down: false,
                label: String::new(),
            })
            .collect()
    }

    pub fn applied_sort(&self) -> Option<AppliedSortSnapshot> {
        self.adapter.applied_sort()
    }

    pub fn sort_key_for_column(&self, column_id: ColumnId) -> Option<SortKey> {
        self.adapter.sort_key_for_column(column_id)
    }

    pub fn sort_capability_for_column(&self, column_id: ColumnId) -> SortCapability {
        let exposed = self
            .column_model()
            .get(column_id)
            .map(|column| column.sortable)
            .unwrap_or(false);
        if !exposed {
            return SortCapability::Unsupported;
        }
        self.adapter
            .sort_key_for_column(column_id)
            .map(|key| self.adapter.sort_capability(&key))
            .unwrap_or(SortCapability::Unsupported)
    }

    pub fn request_sort_for_column(
        &mut self,
        column_id: ColumnId,
        additive: bool,
    ) -> Option<SortRequestOutcome> {
        if !self.sort_capability_for_column(column_id).is_allowed() {
            return None;
        }
        let key = self.adapter.sort_key_for_column(column_id)?;
        let operation = if additive {
            SortOperation::CycleAdditive(key)
        } else {
            SortOperation::CycleReplace(key)
        };
        Some(self.request_sort(operation))
    }

    pub fn request_sort(&mut self, operation: SortOperation) -> SortRequestOutcome {
        let outcome = self.adapter.request_sort(operation);
        self.sync_data_source();
        if let SortRequestOutcome::Applied { snapshot, .. } = &outcome
            && let Some(key) = self.view_config_key()
            && let Some(store) = self.config_store.as_ref()
        {
            view_config::save_sort_preference(
                &self.column_model(),
                &snapshot.spec,
                &key,
                store.as_ref(),
            );
        }
        outcome
    }

    /// Delete the durable preference and request the provider baseline without
    /// converting that reset into a newly persisted empty preference.
    pub fn reset_sort_preference(&mut self) -> Option<SortRequestOutcome> {
        let baseline = self.applied_sort()?.spec.baseline;
        if let Some(key) = self.view_config_key()
            && let Some(store) = self.config_store.as_ref()
        {
            view_config::clear_sort_preference(&self.column_model(), &key, store.as_ref());
        }
        let outcome = self
            .adapter
            .request_sort(SortOperation::ClearToBaseline(baseline));
        self.sync_data_source();
        Some(outcome)
    }

    pub fn realize_visible_cells(&mut self, visible_rows: &[VisibleRow]) {
        self.realized_cell_values.clear();

        let column_ids: Vec<ColumnId> = self
            .cms
            .hts()
            .column_layout()
            .body_slots()
            .map(|slot| slot.column_id)
            .collect();
        let mut cells: HashMap<(u64, ColumnId), Option<DataCell>> = HashMap::new();
        for row in visible_rows {
            let row_id = RowId::from_raw(row.node_id);
            for column_id in &column_ids {
                let value = self.adapter.read_cell(row_id, *column_id);
                self.realized_cell_values
                    .insert((row.node_id, *column_id), value.clone());
                cells.insert((row.node_id, *column_id), self.cell_from_value_state(value));
            }
        }
        self.cms
            .realize_cell_values_with(visible_rows, |row_id, column_id| {
                cells.remove(&(row_id, column_id)).unwrap_or(None)
            });
    }

    /// Collect owned choice work at the engine's deferred boundary, before Layout.
    /// Reads the provider directly: display placeholders are never editor values.
    pub(crate) fn collect_composite_choice_work(
        &mut self,
        manifest: Option<CompositeChoiceManifest>,
        mounts: &HashMap<CompositeChoiceRoleKey, CompositeChoiceMountFacts>,
        on_commit: Rc<dyn Fn(CompositeChoiceRoleKey, String)>,
    ) -> Vec<CompositeChoiceWork> {
        if manifest
            .as_ref()
            .is_none_or(|manifest| manifest.roles.is_empty())
            && !self.cms.has_composite_choice_roles()
        {
            return Vec::new();
        }
        self.sync_data_source();
        let revision = self.adapter.order_revision();
        let columns: HashMap<_, _> = self
            .cms
            .hts()
            .column_model()
            .columns()
            .iter()
            .map(|column| (column.id, column.visible))
            .collect();
        let adapter = &self.adapter;
        let options = &self.combo_box_options;
        self.cms.collect_composite_choices(
            manifest,
            revision,
            mounts,
            |role, realized| {
                use CompositeChoiceAdmission::{NonReady, Ready, Retired};
                let (Some(&visible_column), Some(options)) =
                    (columns.get(&role.column_id), options.get(&role.column_id))
                else {
                    return Retired;
                };
                let row = RowId::from_raw(role.row_id);
                if !realized {
                    match adapter.locate_row(row) {
                        ValueState::Ready(Some(_)) => {}
                        ValueState::Ready(None) => return Retired,
                        ValueState::Pending => return NonReady(CompositeChoiceReadiness::Pending),
                        ValueState::Denied { .. } => return NonReady(CompositeChoiceReadiness::Denied),
                        ValueState::Error { .. } => return NonReady(CompositeChoiceReadiness::Error),
                        ValueState::Missing => return NonReady(CompositeChoiceReadiness::Missing),
                    }
                }
                let capabilities = adapter.capabilities(row, role.column_id);
                if !(visible_column && capabilities.visible && capabilities.readable) {
                    return NonReady(CompositeChoiceReadiness::PolicyDenied);
                }
                match adapter.read_cell(row, role.column_id) {
                    ValueState::Ready(value) if !matches!(value, CellValue::String(_)) => Retired,
                    _ if !(capabilities.editable && capabilities.committable) => {
                        NonReady(CompositeChoiceReadiness::PolicyDenied)
                    }
                    ValueState::Ready(CellValue::String(current_value)) => Ready(ComboBoxCellEditorSpec {
                        current_value,
                        options: options.clone(),
                    }),
                    ValueState::Ready(_) => Retired,
                    ValueState::Pending => NonReady(CompositeChoiceReadiness::Pending),
                    ValueState::Denied { .. } => NonReady(CompositeChoiceReadiness::Denied),
                    ValueState::Error { .. } => NonReady(CompositeChoiceReadiness::Error),
                    ValueState::Missing => NonReady(CompositeChoiceReadiness::Missing),
                }
            },
            on_commit,
        )
    }

    pub fn realized_cell_value_state(
        &self,
        node_id: u64,
        column_id: ColumnId,
    ) -> ValueState<CellValue> {
        self.realized_cell_values
            .get(&(node_id, column_id))
            .cloned()
            .unwrap_or(ValueState::Missing)
    }

    pub fn dispatch_intent(&mut self, intent: &TabularIntent) {
        let _ = self.dispatch_intent_with_selection_change(intent);
    }

    pub(crate) fn dispatch_intents_and_focus(
        &mut self,
        intents: &[TabularIntent],
        focus_row: Option<RowId>,
    ) -> ListViewDispatchCallbacks {
        if !intents.is_empty() || focus_row.is_some() {
            self.selection.lock_interaction_model();
        }

        let previous_focused_row = self.focused_row();
        let mut selection_changed = false;
        let selection_sets_focus = intents.iter().any(|intent| {
            matches!(
                intent,
                TabularIntent::Selection(
                    SelectionIntent::SelectRow { .. }
                        | SelectionIntent::ExtendToRow { .. }
                        | SelectionIntent::ToggleRow { .. }
                        | SelectionIntent::SetSelection { .. }
                )
            )
        });

        for intent in intents {
            selection_changed |= self.dispatch_intent_with_selection_change(intent);
        }
        if let Some(row_id) = focus_row
            && !selection_sets_focus
        {
            self.set_focus_by_row_id(row_id);
        }

        let focused_row = self.focused_row();
        let focused_row_change = (previous_focused_row != focused_row).then_some(focused_row);
        let activation_rows = intents
            .iter()
            .filter_map(|intent| match intent {
                TabularIntent::Activation(ActivationIntent::Activate { row_id }) => Some(*row_id),
                _ => None,
            })
            .collect();

        ListViewDispatchCallbacks {
            presentation_changed: selection_changed || previous_focused_row != focused_row,
            focused_row_callback: focused_row_change.and(self.focused_row_callback.clone()),
            focused_row_change,
            activation_callback: self.activation_callback.clone(),
            activation_rows,
        }
    }

    fn dispatch_intent_with_selection_change(&mut self, intent: &TabularIntent) -> bool {
        match intent {
            TabularIntent::Selection(selection) => self.dispatch_selection_intent(selection),
            TabularIntent::Focus(focus) => {
                self.dispatch_focus_intent(focus);
                false
            }
            TabularIntent::Expansion(expansion) => {
                self.dispatch_expansion_intent(expansion);
                false
            }
            TabularIntent::Resize(ResizeIntent::Commit {
                column_id,
                proposed_width,
            }) => {
                self.commit_user_resize(*column_id, *proposed_width);
                false
            }
            TabularIntent::Resize(ResizeIntent::Begin { .. })
            | TabularIntent::Resize(ResizeIntent::Update { .. })
            | TabularIntent::Resize(ResizeIntent::Cancel { .. }) => false,
            TabularIntent::Visibility(VisibilityIntent::SetVisible { column_id, visible }) => {
                self.set_column_visible(*column_id, *visible);
                false
            }
            TabularIntent::Width(WidthIntent::SetWidth { column_id, width }) => {
                self.set_column_width(*column_id, *width);
                false
            }
            TabularIntent::Reorder(ReorderIntent::Apply { new_order }) => {
                self.reorder_columns(new_order);
                false
            }
            TabularIntent::Sort(_)
            | TabularIntent::Edit(_)
            | TabularIntent::Tree(_)
            | TabularIntent::Activation(_) => false,
        }
    }

    pub fn sync_data_source(&mut self) {
        let result = self.adapter.process_invalidation();
        if !result.has_changes() {
            return;
        }

        if result.base_order_replaced {
            if self.adapter.uses_windowed_order() {
                self.row_ids.clear();
            } else {
                self.row_ids = self.adapter.cached_row_ids().to_vec();
            }
            self.row_count = Self::row_count_from_extent(self.adapter.extent(), self.row_ids.len());
            self.row_heights =
                RowHeights::with_fixed_height_count(self.item_height, self.row_count);
            self.cms.hts_mut().replace_base_order_revision();
            if self.adapter.uses_windowed_order() {
                self.cms.hts_mut().set_tree_data(Vec::new());
                self.cms.hts_mut().set_row_heights(self.row_heights.clone());
            } else {
                self.rebuild_flat_topology();
            }
        }

        if result.extent_changed && !result.base_order_replaced {
            self.row_count = Self::row_count_from_extent(self.adapter.extent(), self.row_ids.len());
            self.row_heights =
                RowHeights::with_fixed_height_count(self.item_height, self.row_count);
            self.cms.hts_mut().set_row_heights(self.row_heights.clone());
        }

        if result.row_heights_changed {
            for (row_id, new_height) in &result.height_changes {
                self.set_row_height_at(*row_id, *new_height);
            }
        }
    }

    fn dispatch_selection_intent(&mut self, intent: &SelectionIntent) -> bool {
        match intent {
            SelectionIntent::SelectRow { row_id } => {
                self.dispatch_selection_action(OrderedSelectionAction::Select { key: *row_id })
            }
            SelectionIntent::ExtendToRow { row_id } => {
                self.dispatch_selection_action(OrderedSelectionAction::ExtendTo { key: *row_id })
            }
            SelectionIntent::ToggleRow { row_id } => {
                self.dispatch_selection_action(OrderedSelectionAction::Toggle { key: *row_id })
            }
            SelectionIntent::ClearSelection => {
                self.dispatch_selection_action(OrderedSelectionAction::ClearSelection)
            }
            SelectionIntent::SetSelection { rows, anchor } => {
                let selection_changed = self.selection.selected_keys.len() != rows.len()
                    || rows
                        .iter()
                        .any(|row_id| !self.selection.selected_keys.contains(row_id));
                self.selection.selected_keys.clear();
                self.selection.selected_keys.extend(rows.iter().copied());
                self.selection.anchor_key = *anchor;
                self.selection.focus_key = *anchor;
                if let Some(row_id) = anchor {
                    self.request_scroll_to_reveal_row(*row_id);
                }
                selection_changed
            }
        }
    }

    fn dispatch_focus_intent(&mut self, intent: &FocusIntent) {
        match intent {
            FocusIntent::RequestFocus { row_id, column_id } => {
                self.focused_column = Some(*column_id);
                self.dispatch_selection_action(OrderedSelectionAction::FocusTo { key: *row_id });
            }
            FocusIntent::RequestClearFocus => {
                self.selection.focus_key = None;
            }
            FocusIntent::RequestNavigate { .. } => {}
        }
    }

    fn dispatch_expansion_intent(&mut self, intent: &ExpansionIntent) {
        let mut hts = self.cms.hts_mut();
        let expansion = hts.expansion_mut();
        match intent {
            ExpansionIntent::Expand { row_id } => {
                expansion.expand(*row_id);
            }
            ExpansionIntent::Collapse { row_id } => {
                expansion.collapse(*row_id);
            }
            ExpansionIntent::Toggle { row_id } | ExpansionIntent::ToggleWithCapture { row_id } => {
                expansion.toggle(*row_id);
            }
        }
    }

    fn dispatch_selection_action(&mut self, action: OrderedSelectionAction<RowId>) -> bool {
        let index = ListViewSelectionIndex {
            row_ids: &self.row_ids,
            adapter: &self.adapter,
            row_count: self.row_count,
        };
        let result = reduce_ordered_selection(&mut self.selection, action, &index);
        if let Some(row_id) = result.new_focus {
            if self.focused_column.is_none() {
                self.focused_column = self.first_column_id();
            }
            self.request_scroll_to_reveal_row(row_id);
        }
        result.selection_changed
    }

    fn request_scroll_to_reveal_row(&mut self, row_id: RowId) {
        let Some(index) = self.row_id_to_index(row_id) else {
            return;
        };
        if let Some(scroll_y) = self.compute_scroll_to_reveal_index(index) {
            self.requested_scroll_y = Some(scroll_y);
        }
    }

    fn compute_scroll_to_reveal_index(&self, index: usize) -> Option<f32> {
        let row_top = self.row_top_at_index(index);
        let row_height = self.row_height_at_index(index);
        let row_bottom = row_top + row_height;
        // SPEC-PM-017: f32 shim — rung 2.
        let scroll_y = self.cms.hts().scroll_offset() as f32;
        let viewport_bottom = scroll_y + self.cms.hts().viewport_height().max(self.viewport_height);

        if row_top < scroll_y {
            Some(row_top.max(0.0))
        } else if row_bottom > viewport_bottom {
            Some((row_bottom - self.viewport_height).max(0.0))
        } else {
            None
        }
    }

    fn cell_from_value_state(&self, value: ValueState<CellValue>) -> Option<DataCell> {
        match value {
            ValueState::Ready(CellValue::String(s)) => {
                Some(DataCell::new(PropertyValue::String(s), true))
            }
            ValueState::Ready(CellValue::Int(n)) => {
                Some(DataCell::new(PropertyValue::Integer(n), true))
            }
            ValueState::Ready(CellValue::Float(f)) => {
                Some(DataCell::new(PropertyValue::Float(f), true))
            }
            ValueState::Ready(CellValue::Bool(b)) => {
                Some(DataCell::new(PropertyValue::Bool(b), true))
            }
            ValueState::Ready(CellValue::Null) => Some(DataCell::new(
                PropertyValue::String("\u{2014}".to_string()),
                true,
            )),
            ValueState::Pending => Some(DataCell::new(
                PropertyValue::String("...".to_string()),
                true,
            )),
            ValueState::Denied { .. } => Some(DataCell::new(
                PropertyValue::String("[denied]".to_string()),
                true,
            )),
            ValueState::Error { .. } => Some(DataCell::new(
                PropertyValue::String("[error]".to_string()),
                true,
            )),
            ValueState::Missing => None,
        }
    }
}

pub(crate) struct ListViewDispatchCallbacks {
    presentation_changed: bool,
    focused_row_callback: Option<Rc<dyn Fn(Option<RowId>)>>,
    focused_row_change: Option<Option<RowId>>,
    activation_callback: Option<Rc<dyn Fn(RowId)>>,
    activation_rows: Vec<RowId>,
}

impl ListViewDispatchCallbacks {
    pub(crate) fn presentation_changed(&self) -> bool {
        self.presentation_changed
    }

    pub(crate) fn invoke(self) {
        if let (Some(callback), Some(focused_row)) =
            (self.focused_row_callback, self.focused_row_change)
        {
            callback(focused_row);
        }

        if let Some(callback) = self.activation_callback {
            for row_id in self.activation_rows {
                callback(row_id);
            }
        }
    }
}
