// =============================================================================
// SPEC-HAL-164: TabularSubstrate — Root Abstraction
// =============================================================================
//
// TabularSubstrate is a FIRST-CLASS columnar structure with explicit geometry domains.
// It is NOT a List with columns. ListView, TableView, and TreeView are projections
// over TabularSubstrate, not extensions of List.
//
// SPEC-HAL-164 INVARIANT-1: Column geometry resolved once per evaluation.
// SPEC-HAL-164 INVARIANT-2: Header/Body/Footer must not compute column widths independently.
// SPEC-HAL-164 INVARIANT-3: No projection may mutate column geometry during paint or input.
//
// This module provides foundation structure only - no visuals, no behaviors
// beyond what is explicitly mandated by the SPECs.
// =============================================================================
//
// =============================================================================
// SEALED BASE: SPEC-HAL-164 through SPEC-HAL-178
// =============================================================================
//
// The following SPECs form the sealed TabularSubstrate base layer:
//
//   164 — Root Abstraction and Invariants
//   165 — Column Model and Geometry Resolution
//   166 — Header Domain
//   167 — Body Domain and Row Projection
//   168 — Horizontal Geometry Resolution
//   169 — Column Resize Interaction Contract
//   170 — Column Separators and Visual Guides
//   171 — Sort and Header Affordances
//   172 — Tree Projection
//   173 — Cell Content Projection Contract
//   174 — Inline Editing Lifecycle (+ 174A: Deferred Comparator)
//   175 — Cell Focus and Navigation Semantics
//   177 — Deferred Intent Queue Semantics
//   178 — Row Selection Semantics
//
// These SPECs are CLOSED. No mutation of their invariants, semantics, or
// structural contracts is permitted. Extensions must layer via new SPECs only.
// Any change to behavior governed by 164–178 requires an explicit new SPEC.
//
// =============================================================================

pub mod body;
pub mod cell_content;
pub mod cell_lifecycle;
pub mod column;
pub mod column_config;
pub mod data_adapter;
pub mod dirty;
pub mod expandable_row_host;
pub mod geometry;
pub mod guards;
pub mod header;
pub mod intent_policy;
pub mod intents;
pub mod row;
pub mod separators;
pub mod sort;
pub mod tree_paint;
pub mod view_config;

use std::cell::UnsafeCell;

use guards::{ColumnModelGuard, EvaluationPhaseGuard, RowModelGuard, SortProcessingGuard};

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::ordered_selection::{
    OrderedSelectionAction, OrderedSelectionState, SelectionInteractionModel, SelectionMode,
    reduce_ordered_selection,
};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, ExpansionPolicy, RowExpansion};
use crate::utils::id::WidgetInternalId;

/// Active edit session data.
///
/// SPEC-HAL-174: Tracks which cell is being edited and the focus state
/// prior to edit entry. Used to enforce exclusive edit authority and
/// restore focus on cancel.
#[derive(Debug, Clone, PartialEq)]
pub struct EditSession {
    editing_cell: CellId,
    pre_edit_focus: Option<CellId>,
}

impl EditSession {
    /// Get the cell currently being edited.
    pub fn editing_cell(&self) -> CellId {
        self.editing_cell
    }

    /// Get the focus state before edit began (restored on cancel).
    pub fn pre_edit_focus(&self) -> Option<CellId> {
        self.pre_edit_focus
    }
}

/// Edit lifecycle state.
///
/// SPEC-HAL-174 INV-174-2: Explicit lifecycle: Idle → Armed → Editing → Idle.
/// No implicit transitions.
///
/// INV-174-1: Exactly one CellId at a time. No multi-edit.
/// INV-174-3: Owned by TabularSubstrate, not by embedded widgets.
#[derive(Debug, Clone, PartialEq)]
pub enum EditState {
    /// No active edit.
    Idle,
    /// Edit intent received, preparing to edit.
    /// (Transitions immediately to Editing in current implementation.)
    Armed(EditSession),
    /// Actively editing a cell.
    Editing(EditSession),
}

impl EditState {
    /// Check if an edit is active (Armed or Editing).
    pub fn is_active(&self) -> bool {
        !matches!(self, EditState::Idle)
    }

    /// Get the edit session if in Editing state.
    pub fn session(&self) -> Option<&EditSession> {
        match self {
            EditState::Editing(s) => Some(s),
            _ => None,
        }
    }

    /// Get the editing cell if in Armed or Editing state.
    pub fn editing_cell(&self) -> Option<CellId> {
        match self {
            EditState::Armed(s) | EditState::Editing(s) => Some(s.editing_cell),
            EditState::Idle => None,
        }
    }
}

/// Cell focus state.
///
/// SPEC-HAL-175: Stores focused cell and anchor for navigation/selection.
/// Only mutated via FocusIntent in handle_intent(). Sort and tree intents
/// must NOT mutate FocusState.
#[derive(Debug, Clone, Default)]
pub struct FocusState {
    /// Currently focused cell (None = no focus).
    focused: Option<CellId>,
    /// Anchor cell for range selection (None = no anchor).
    anchor: Option<CellId>,
    /// Version counter for change detection.
    version: u64,
}

impl FocusState {
    /// Create a new unfocused state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently focused cell.
    pub fn focused(&self) -> Option<CellId> {
        self.focused
    }

    /// Get the anchor cell (for range selection).
    pub fn anchor(&self) -> Option<CellId> {
        self.anchor
    }

    /// Get the version counter.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Check if a cell is focused.
    pub fn is_focused(&self) -> bool {
        self.focused.is_some()
    }
}

/// Selection state for TabularSubstrate.
///
/// SPEC-HAL-178: Selection is a set of RowId identities.
/// Independent of FocusState (INV-178-2). Never mutates focus.
/// Identity-sticky across sort/tree/scroll (INV-178-3).
#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    /// Anchor row for range operations (where the range started).
    anchor: Option<row::RowId>,
    /// Selected rows (identity-based, not index-based).
    selected_rows: Vec<row::RowId>,
    /// Version counter for change detection.
    version: u64,
}

impl SelectionState {
    /// Create a new empty selection state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the anchor row.
    pub fn anchor(&self) -> Option<row::RowId> {
        self.anchor
    }

    /// Get the selected rows.
    pub fn selected_rows(&self) -> &[row::RowId] {
        &self.selected_rows
    }

    /// Get the version counter.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Check if any rows are selected.
    pub fn has_selection(&self) -> bool {
        !self.selected_rows.is_empty()
    }

    /// Check if a specific row is selected.
    pub fn is_selected(&self, row_id: row::RowId) -> bool {
        self.selected_rows.contains(&row_id)
    }

    /// Get the number of selected rows.
    pub fn count(&self) -> usize {
        self.selected_rows.len()
    }
}

/// Engine invalidation state owned by the substrate orchestrator.
///
/// SPEC-HAL-258: All engine invalidation must flow through the substrate.
/// No engine may invalidate another engine directly (INV-IG-1).
/// The substrate is the single coordination point (INV-IG-2).
///
/// Allowed invalidation direction (INV-IG-3):
///   Model/Intent → Substrate flags → Geometry resolve → Viewport realization → Paint
///
/// Invalidation direction contract (SPEC-HAL-258):
///
///   ALLOWED:
///     - COLUMN MODEL change → invalidates GEOMETRY
///     - WIDTH change         → invalidates GEOMETRY
///     - TOPOLOGY change      → invalidates VIRTUALIZATION
///
///   FORBIDDEN:
///     - GEOMETRY may NOT invalidate TOPOLOGY
///     - GEOMETRY may NOT invalidate VIRTUALIZATION
///     - TOPOLOGY may NOT invalidate GEOMETRY
///     - VIRTUALIZATION may NOT invalidate GEOMETRY
///     - SELECTION may NOT invalidate GEOMETRY
///
///   Only COLUMN MODEL and WIDTH may invalidate GEOMETRY.
///   Only TOPOLOGY may invalidate VIRTUALIZATION.
#[derive(Debug)]
struct EngineInvalidation {
    // =========================================================================
    // INVALIDATION LAYERING (SPEC-HAL-258)
    // =========================================================================
    //
    // GEOMETRY invalidation must NOT trigger:
    //   - TOPOLOGY
    //   - VIRTUALIZATION
    //
    // TOPOLOGY invalidation must trigger:
    //   - VIRTUALIZATION
    //
    // VIRTUALIZATION invalidation must NOT trigger:
    //   - GEOMETRY
    //   - TOPOLOGY
    //
    // No logic enforcement yet. Structural clarity only.
    // =========================================================================
    /// Column geometry needs re-resolution.
    geometry_dirty: bool,
    /// Virtualization needs recompute (VirtualizationEngine).
    #[allow(dead_code)]
    virtualization_dirty: bool,
    /// Selection needs recompute (placeholder).
    #[allow(dead_code)]
    selection_dirty: bool,
    /// Last available width seen by resolve_geometry (pixel-snapped).
    /// Used to detect container width changes and set geometry_dirty.
    last_geometry_width: f32,
}

impl EngineInvalidation {
    fn new() -> Self {
        Self {
            geometry_dirty: true,       // initial frame must resolve geometry
            virtualization_dirty: true, // initial frame must realize visible rows
            selection_dirty: false,
            last_geometry_width: -1.0, // sentinel: no width seen yet
        }
    }
}

/// Single-use range token for cached visible range.
///
/// INV-VZ-30: Cached visible range is represented by Option<VisibleRangeToken>.
/// INV-VZ-31: Range token must be consumed exactly once via take().
/// INV-VZ-32: token.epoch == scroll_epoch required for validity.
/// INV-VZ-33: Layout path never produces a token.
#[must_use]
struct VisibleRangeToken {
    first: usize,
    last: usize,
    y_offset: f32,
    epoch: u64,
    _no_copy_no_clone: UnsafeCell<()>,
}

/// Per-frame evaluation mode — tracks which path owns realization authority.
///
/// Exactly one of these must be set per frame:
/// - `TokenProduced`: scroll sync produced a token; `prepare_evaluation` consumes it.
/// - `LayoutHandled`: Widget::layout computed and realized range directly.
/// - `None`: No realization path ran — contract violation if realization is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvaluationMode {
    /// No realization path has run this frame.
    None,
    /// Scroll sync produced a VisibleRangeToken for prepare_evaluation to consume.
    TokenProduced,
    /// Widget::layout handled realization directly; prepare_evaluation must skip.
    LayoutHandled,
}

/// TabularSubstrate widget — root abstraction for columnar views.
///
/// SPEC-HAL-164: TabularSubstrate is a structural coordinator, not a content container.
/// It defines column geometry, resolves horizontal extents, coordinates vertical
/// projection, and exposes stable identity hooks for projections.
///
/// It does NOT:
/// - own arbitrary child widgets
/// - perform rendering directly
/// - implement interaction semantics directly
pub struct TabularSubstrate {
    /// Widget internal ID.
    id: WidgetInternalId,

    /// Column model (owned exclusively by TabularSubstrate).
    column_model: ColumnModel,

    /// Resolved column geometry (frame-local, immutable after resolution).
    geometry_table: ColumnGeometryTable,

    /// Header domain (structural sibling of body).
    header: HeaderDomain,

    /// Body domain (scrollable region containing row projections).
    body: BodyDomain,

    /// Sort state (owned exclusively by TabularSubstrate).
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-1: Sort state owned exclusively here.
    sort_state: SortState,

    /// Row comparator for sorting.
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-6: If None or returns None, sorting is no-op.
    comparator: Option<Box<dyn RowComparator + Send + Sync>>,

    /// Deferred comparator change (SPEC-HAL-174A).
    ///
    /// SPEC-HAL-174A: Comparator changes are configuration, not intents.
    /// During active edit, set_comparator defers here instead of applying.
    /// Applied immediately when edit session ends (commit or cancel).
    pending_comparator: Option<Box<dyn RowComparator + Send + Sync>>,

    /// Cell focus state (SPEC-HAL-175).
    ///
    /// SPEC-HAL-175: Only mutated via FocusIntent in handle_intent().
    /// Sort and tree intents must not touch this.
    focus_state: FocusState,

    /// Edit lifecycle state (SPEC-HAL-174).
    ///
    /// SPEC-HAL-174 INV-174-3: Owned by TabularSubstrate, not embedded widgets.
    /// INV-174-5: Sort/tree/scroll/column-hide during edit → Cancel.
    edit_state: EditState,

    /// Selection state (SPEC-HAL-178).
    ///
    /// # DEPRECATED — DO NOT USE FOR NEW CODE
    ///
    /// This is a legacy compatibility mirror. The authoritative selection state
    /// is `ordered_selection_state`. This field exists solely for backward
    /// compatibility during migration.
    ///
    /// **Invariant:** This field may ONLY be mutated by `mirror_ordered_into_legacy_selection()`.
    /// Any other mutation is a bug.
    ///
    /// **Reads:** Use `ordered_selection()` for new code. This field will be removed
    /// once all consumers migrate.
    ///
    /// SPEC-HAL-178: Row-level selection, identity-sticky.
    /// Independent of FocusState. Cleared on edit begin.
    selection_state: SelectionState,

    /// Ordered selection state (ordered_selection core).
    ///
    /// # AUTHORITATIVE SELECTION STATE
    ///
    /// This is the single source of truth for selection. All selection logic
    /// flows through the reducer (`reduce_ordered_selection`).
    ///
    /// **Invariant (INV-TAB-SEL-1):** This is the ONLY authoritative selection state.
    /// The legacy `selection_state` is a mirror that will be removed.
    ///
    /// **Writes:** Via `handle_selection_intent()` → reducer → mirror
    /// **Reads:** Via `ordered_selection()` accessor
    ordered_selection_state: OrderedSelectionState<row::RowId>,

    /// Frame phase guard for invariant enforcement.
    evaluation_guard: EvaluationPhaseGuard,

    /// Column model mutation guard.
    column_guard: ColumnModelGuard,

    /// Row realization guard.
    row_guard: RowModelGuard,

    /// Sort processing guard (SPEC-HAL-171 identity stability).
    sort_guard: SortProcessingGuard,

    /// Pending intents emitted during the current frame.
    pending_intents: Vec<TabularIntent>,

    /// External intent handler callback.
    on_intent: Option<Box<dyn Fn(&TabularIntent) + Send + Sync>>,

    /// SPEC-HAL-258: Engine invalidation graph owned by the orchestrator.
    engine_invalidation: EngineInvalidation,

    /// SPEC-HAL-256: Row topology engine — owns sorted indices, versions, dirty flag.
    row_topology: Box<dyn crate::engines::row_topology::TopologyEngine>,

    /// SPEC-HAL-257: Virtualization engine — owns visible range computation.
    virtualization: Box<dyn crate::engines::virtualization::VirtualizationStrategy>,

    /// Single-use cached visible range from VirtualizationEngine.
    /// INV-VZ-30: Represented as Option<VisibleRangeToken>.
    /// INV-VZ-31: Consumed exactly once via take() in prepare_evaluation.
    /// INV-VZ-33: Layout path never produces a token.
    visible_range: Option<VisibleRangeToken>,

    /// Monotonic scroll epoch. Incremented on every scroll_y mutation.
    /// INV-VZ-32: token.epoch must equal scroll_epoch for validity.
    scroll_epoch: u64,

    /// Per-frame evaluation mode — which path owns realization authority.
    /// Set by update_visible_range_from_scroll (TokenProduced) or Widget::layout (LayoutHandled).
    /// Checked by prepare_evaluation to determine token consumption vs early return.
    evaluation_mode: EvaluationMode,

    /// Whether this substrate is embedded in a ListViewCore.
    /// When true, update_visible_range_from_scroll (token path) is forbidden;
    /// scroll sync must use set_scroll_y_from_scroll_area instead.
    #[cfg(any(debug_assertions, test))]
    is_listview_embedded: bool,

    /// SPEC-HAL-313 §3.2: Row expansion state.
    ///
    /// INV-313-SUBSTRATE-OWNS-1: Expansion state is owned exclusively here.
    /// Configured via set_expansion_policy / set_expanded_height_policy.
    /// Mutated via handle_intent (ExpansionIntent arm, added in T4).
    row_expansion: RowExpansion,
}

impl TabularSubstrate {
    /// Create a new TabularSubstrate with default configuration.
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            column_model: ColumnModel::new(),
            geometry_table: ColumnGeometryTable::new(),
            header: HeaderDomain::default(),
            body: BodyDomain::default(),
            sort_state: SortState::new(),
            comparator: None,
            pending_comparator: None,
            focus_state: FocusState::new(),
            edit_state: EditState::Idle,
            selection_state: SelectionState::new(),
            ordered_selection_state: OrderedSelectionState::new(SelectionMode::Multi),
            evaluation_guard: EvaluationPhaseGuard::new(),
            column_guard: ColumnModelGuard::new(),
            row_guard: RowModelGuard::new(),
            sort_guard: SortProcessingGuard::new(),
            pending_intents: Vec::new(),
            on_intent: None,
            engine_invalidation: EngineInvalidation::new(),
            row_topology: Box::new(crate::engines::row_topology::RowTopologyEngine::new()),
            virtualization: Box::new(crate::engines::virtualization::VirtualizationEngine::new()),
            visible_range: None,
            scroll_epoch: 0,
            evaluation_mode: EvaluationMode::None,
            #[cfg(any(debug_assertions, test))]
            is_listview_embedded: false,
            row_expansion: RowExpansion::default(),
        }
    }

    /// Create a TabularSubstrate with explicit columns.
    pub fn with_columns(columns: Vec<Column>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            column_model: ColumnModel::with_columns(columns),
            geometry_table: ColumnGeometryTable::new(),
            header: HeaderDomain::default(),
            body: BodyDomain::default(),
            sort_state: SortState::new(),
            comparator: None,
            pending_comparator: None,
            focus_state: FocusState::new(),
            edit_state: EditState::Idle,
            selection_state: SelectionState::new(),
            ordered_selection_state: OrderedSelectionState::new(SelectionMode::Multi),
            evaluation_guard: EvaluationPhaseGuard::new(),
            column_guard: ColumnModelGuard::new(),
            row_guard: RowModelGuard::new(),
            sort_guard: SortProcessingGuard::new(),
            pending_intents: Vec::new(),
            on_intent: None,
            engine_invalidation: EngineInvalidation::new(),
            row_topology: Box::new(crate::engines::row_topology::RowTopologyEngine::new()),
            virtualization: Box::new(crate::engines::virtualization::VirtualizationEngine::new()),
            visible_range: None,
            scroll_epoch: 0,
            evaluation_mode: EvaluationMode::None,
            #[cfg(any(debug_assertions, test))]
            is_listview_embedded: false,
            row_expansion: RowExpansion::default(),
        }
    }

    /// Set header height.
    pub fn with_header_height(mut self, height: f32) -> Self {
        self.header.set_height(height);
        self
    }

    /// Set row model.
    ///
    /// INV-ROW-ORDER-EPOCH-1: Bumps base_row_order_epoch (structural row mutation).
    pub fn with_row_model(mut self, row_model: RowModel) -> Self {
        self.body = BodyDomain::with_row_model(row_model);
        self.bump_base_row_order_epoch();
        self
    }

    /// Set intent handler callback.
    pub fn with_on_intent<F>(mut self, handler: F) -> Self
    where
        F: Fn(&TabularIntent) + Send + Sync + 'static,
    {
        self.on_intent = Some(Box::new(handler));
        self
    }

    /// Set row comparator for sorting.
    ///
    /// SPEC-HAL-171: The comparator is called during sort order computation.
    /// If not set, sorting is a no-op (natural order preserved).
    pub fn with_comparator<C>(mut self, comparator: C) -> Self
    where
        C: RowComparator + Send + Sync + 'static,
    {
        self.comparator = Some(Box::new(comparator));
        self
    }

    // =========================================================================
    // Column Model Access (SPEC-HAL-165)
    // =========================================================================

    /// Get immutable reference to column model.
    pub fn column_model(&self) -> &ColumnModel {
        &self.column_model
    }

    /// Get mutable reference to column model.
    pub fn column_model_mut(&mut self) -> &mut ColumnModel {
        &mut self.column_model
    }

    /// Mark geometry as dirty (forces re-resolution on next layout).
    ///
    /// Call after mutating the column model through `column_model_mut()`.
    pub fn mark_geometry_dirty(&mut self) {
        self.engine_invalidation.geometry_dirty = true;
    }

    /// Add a column.
    ///
    /// SPEC-HAL-165 INVARIANT-1: Column Model owned exclusively by TabularSubstrate.
    /// SPEC-HAL-165: Structural mutation, increments version.
    pub fn add_column(&mut self, column: Column) {
        self.column_guard.assert_mutation_allowed();
        self.column_model.push(column);
        self.engine_invalidation.geometry_dirty = true;
    }

    /// Set column visibility.
    ///
    /// SPEC-HAL-164 INVARIANT-A1: Routes through intent pipeline.
    /// SPEC-HAL-165: Visibility toggling triggers layout invalidation.
    /// SPEC-HAL-174 INV-174-5: Hiding the editing column → Cancel (handled in handle_intent).
    pub fn set_column_visible(&mut self, id: ColumnId, visible: bool) -> bool {
        let version_before = self.column_model.version();
        self.handle_intent(&TabularIntent::Visibility(
            intents::VisibilityIntent::SetVisible {
                column_id: id,
                visible,
            },
        ));
        self.column_model.version() != version_before
    }

    /// Toggle column visibility.
    pub fn toggle_column_visible(&mut self, id: ColumnId) -> bool {
        let currently_visible = self.column_model.get(id).map_or(false, |c| c.visible);
        self.set_column_visible(id, !currently_visible)
    }

    /// Query column visibility.
    pub fn is_column_visible(&self, id: ColumnId) -> bool {
        self.column_model.get(id).map_or(false, |c| c.visible)
    }

    /// Update column preferred width.
    ///
    /// SPEC-HAL-164 INVARIANT-A1: Routes through intent pipeline.
    /// SPEC-HAL-165: Structural mutation, increments version.
    pub fn set_column_width(&mut self, id: ColumnId, width: f32) -> bool {
        let version_before = self.column_model.version();
        self.handle_intent(&TabularIntent::Width(intents::WidthIntent::SetWidth {
            column_id: id,
            width,
        }));
        self.column_model.version() != version_before
    }

    /// Apply a column configuration state to this substrate.
    ///
    /// Reorders, sets visibility, and sets scroll domain for all columns.
    /// Returns true if any mutation occurred.
    pub fn apply_column_configuration(
        &mut self,
        config: &column_config::ColumnConfigurationState,
    ) -> bool {
        let mut changed = false;

        // Reorder first.
        if self.column_model.reorder(&config.column_order) {
            changed = true;
        }

        // Snapshot IDs to avoid borrow conflicts.
        let ids: Vec<ColumnId> = self.column_model.columns().iter().map(|c| c.id).collect();

        // Apply preferred widths, visibility, and domain as configuration inputs.
        for id in &ids {
            if let Some(width) = config.preferred_width_for(*id)
                && width.is_finite()
                && width > 0.0
                && let Some(column) = self.column_model.get(*id)
            {
                let clamped = width
                    .max(column.min_width.max(column::MIN_COL_WIDTH))
                    .min(column.max_width);
                if column.preferred_width != clamped
                    && self.column_model.set_preferred_width(*id, clamped)
                {
                    changed = true;
                }
            }
            let visible = config.is_visible(*id);
            if self.column_model.set_visible(*id, visible) {
                changed = true;
            }
            let domain = config.domain_for(*id);
            if self.column_model.set_scroll_domain(*id, domain) {
                changed = true;
            }
        }

        if changed {
            self.engine_invalidation.geometry_dirty = true;
        }
        changed
    }

    // =========================================================================
    // Geometry Access (SPEC-HAL-168)
    // =========================================================================

    /// Get epoch-safe geometry view.
    ///
    /// SPEC-HAL-168: All geometry reads go through this view, which enforces:
    /// - INVARIANT-6: Panics if geometry not resolved
    /// - INVARIANT-7: Panics (in debug) if geometry is stale
    ///
    /// This is the ONLY public method to access geometry. There is no raw
    /// &ColumnGeometryTable accessor.
    pub fn geometry_view(&self) -> GeometryView<'_> {
        GeometryView::new(
            &self.geometry_table,
            self.evaluation_guard.evaluation_epoch(),
        )
    }

    /// Check if column geometry is resolved for the current evaluation epoch.
    ///
    /// Read-only predicate — does not cause resolution or mutate state.
    /// Used by wrapper widgets to gate geometry reads (e.g., separator_hit_bands)
    /// so they never access geometry before TabularSubstrate::layout() resolves it.
    #[allow(dead_code, reason = "Retained current-epoch geometry admission query")]
    pub(crate) fn geometry_resolved_for_current_epoch(&self) -> bool {
        self.geometry_table.is_resolved()
            && !self
                .geometry_table
                .is_stale(self.evaluation_guard.evaluation_epoch())
    }

    /// Get total content width.
    pub fn content_width(&self) -> f32 {
        if self.geometry_table.is_resolved() {
            self.geometry_table
                .total_width(self.evaluation_guard.evaluation_epoch())
        } else {
            0.0
        }
    }

    /// Get total content height (header + body).
    pub fn content_height(&self) -> f32 {
        self.header.height() + self.body.content_height()
    }

    // =========================================================================
    // Row Model Access (SPEC-HAL-167)
    // =========================================================================

    /// Get immutable reference to row model.
    pub fn row_model(&self) -> &RowModel {
        self.body.row_model()
    }

    /// Bump the base row order epoch and invalidate derived caches.
    ///
    /// INV-ROW-ORDER-EPOCH-1: Called from every operation that changes the base
    /// RowId sequence / row model structure. Increments by exactly +1.
    ///
    /// Sets sorted_indices_epoch to 0 to force recompute on next semantic consumption.
    fn bump_base_row_order_epoch(&mut self) {
        self.row_topology.increment_base_epoch();
    }

    /// Add a row.
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-4: Panics if called during sort processing.
    /// INV-ROW-ORDER-EPOCH-1: Bumps base_row_order_epoch (structural row mutation).
    pub fn add_row(&mut self, id: RowId) {
        self.sort_guard.assert_row_mutation_allowed();
        self.body.row_model_mut().push(id);
        self.bump_base_row_order_epoch();
    }

    /// Add a row with specific height.
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-4: Panics if called during sort processing.
    /// INV-ROW-ORDER-EPOCH-1: Bumps base_row_order_epoch (structural row mutation).
    pub fn add_row_with_height(&mut self, id: RowId, height: f32) {
        self.sort_guard.assert_row_mutation_allowed();
        self.body.row_model_mut().push_with_height(id, height);
        self.bump_base_row_order_epoch();
    }

    /// Clear all rows.
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-4: Panics if called during sort processing.
    /// INV-ROW-ORDER-EPOCH-1: Bumps base_row_order_epoch (structural row mutation).
    pub fn clear_rows(&mut self) {
        self.sort_guard.assert_row_mutation_allowed();
        self.body.row_model_mut().clear();
        self.bump_base_row_order_epoch();
    }

    /// Set the fixed row height for all rows.
    ///
    /// **INV-TV-ROW-HEIGHT-CONSISTENCY**: This sets the uniform height for all rows.
    /// Must be called BEFORE adding rows to ensure correct geometry.
    /// When item_height changes, call this then rebuild rows.
    pub fn set_fixed_row_height(&mut self, height: f32) {
        self.body.row_model_mut().set_fixed_height(height);
    }

    /// Get the fixed row height (if using fixed-height mode).
    pub fn fixed_row_height(&self) -> Option<f32> {
        self.body.row_model().fixed_height()
    }

    // =========================================================================
    // Domain Access
    // =========================================================================

    /// Get header domain reference.
    pub fn header(&self) -> &HeaderDomain {
        &self.header
    }

    /// Get header domain reference (mutable).
    ///
    /// SPEC-HAL-186 INV-PART-INTENT-2: Header emits intents only.
    /// Mutable access is needed for resize lifecycle (begin/commit/cancel)
    /// which maintains internal resize state.
    pub fn header_mut(&mut self) -> &mut HeaderDomain {
        &mut self.header
    }

    /// Get body domain reference.
    pub fn body(&self) -> &BodyDomain {
        &self.body
    }

    /// Get body domain mutable reference.
    #[cfg(test)]
    pub(crate) fn body_mut(&mut self) -> &mut BodyDomain {
        &mut self.body
    }

    /// Set the body viewport.
    ///
    /// **INV-ROW-GEOM-SINGLE-SOURCE**: Allows external widgets (like List)
    /// to configure the body viewport for correct hit-testing geometry.
    /// The viewport defines where the body starts (after header) and its size.
    pub fn set_body_viewport(&mut self, viewport: RectF) {
        self.body.set_viewport(viewport);
        self.engine_invalidation.virtualization_dirty = true;
    }

    /// Prepare TabularSubstrate for geometry evaluation: resolve geometry and realize visible rows.
    ///
    /// **INV-LIST2-REALIZATION-NONEMPTY**: This must be called during layout to ensure
    /// rows are realized before paint. Without this call, realized_rows() will be empty.
    ///
    /// Begin a new evaluation epoch.
    ///
    /// This resets the geometry_resolved flag, allowing prepare_evaluation to
    /// re-resolve column geometry with updated preferred widths.
    ///
    /// **MUST** be called once per frame before prepare_evaluation when column
    /// widths may have changed (e.g., during resize drag).
    pub fn begin_evaluation(&self) {
        self.evaluation_guard.begin_evaluation();
    }

    /// Centralized topology orchestration.
    ///
    /// SPEC-HAL-261 INVARIANT-261-RT-1: Engine owns ALL gating.
    /// This is the single place that builds the sorting closure,
    /// calls ensure_sorted_order, clears sort_state, and sets
    /// virtualization_dirty.
    fn ensure_topology(&mut self) {
        let base_row_count = self.body.row_model().len();
        let sort_dirty = self.sort_state.is_dirty();

        let sort_state = &self.sort_state;
        let comparator = self.comparator.as_deref();
        let sorter = |indices: &mut [usize]| {
            if let (Some(col), Some(dir)) = (sort_state.active_column(), sort_state.direction()) {
                let cmp: &dyn RowComparator = match comparator {
                    Some(c) => c,
                    None => &NullComparator,
                };
                indices.sort_by(|&a, &b| match cmp.compare_rows(a, b, col) {
                    Some(ord) => match dir {
                        intents::SortDirection::Ascending => ord,
                        intents::SortDirection::Descending => ord.reverse(),
                    },
                    None => a.cmp(&b),
                });
            }
        };

        let recomputed =
            self.row_topology
                .ensure_sorted_order(base_row_count, None, sort_dirty, &sorter);
        if recomputed {
            self.sort_state.clear_dirty();
            self.engine_invalidation.virtualization_dirty = true;
        }
    }

    /// # Arguments
    /// * `available_width` - The width available for column layout
    ///
    /// # Panics
    /// Panics if body viewport has zero height (must call set_body_viewport first).
    pub fn prepare_evaluation(&mut self, available_width: f32, scale_factor: f32) {
        // Step 0: Process topology — engine owns ALL gating (SPEC-HAL-258).
        self.ensure_topology();

        // Step 1: Resolve column geometry (only if not already resolved this evaluation)
        if !self.evaluation_guard.is_geometry_resolved() {
            self.resolve_geometry(available_width, scale_factor);
        }

        // Step 2: Realize visible rows using sorted_indices (gated by virtualization_dirty).
        // INV-VZ-30: Range token consumed exactly once via take().
        // INV-VZ-32: token.epoch == scroll_epoch guarantees range matches current scroll_y.
        // INV-VZ-3: Realization iterates only the visible slice — O(visible).
        if self
            .row_topology
            .should_realize(self.engine_invalidation.virtualization_dirty)
        {
            match self.evaluation_mode {
                EvaluationMode::TokenProduced => {
                    // INV-VZ-31: Consume the token exactly once. Panics if no token was produced.
                    let token = self.visible_range.take().expect(
                        "INV-VZ-31: visible_range token must be produced before prepare_evaluation consumes it",
                    );

                    // INV-VZ-32: Token epoch must match current scroll epoch.
                    debug_assert_eq!(
                        token.epoch, self.scroll_epoch,
                        "INV-VZ-32: token epoch ({}) is stale vs scroll epoch ({})",
                        token.epoch, self.scroll_epoch
                    );

                    self.prepare_evaluation_core(token.first, token.last, token.y_offset);
                }
                EvaluationMode::LayoutHandled => {
                    // Layout already computed and realized the visible range this frame.
                    // Nothing to do — realization authority was exercised by Widget::layout.
                    return;
                }
                EvaluationMode::None => {
                    panic!(
                        "INV-VZ-31: No realization path ran this frame. \
                         Neither scroll sync (TokenProduced) nor Widget::layout (LayoutHandled) \
                         set evaluation_mode before prepare_evaluation was called."
                    );
                }
            }
        }
    }

    /// Core realization: assumes token already extracted and validated.
    ///
    /// Only called from prepare_evaluation after token consumption.
    /// Layout path bypasses this entirely and calls realize_with_range directly.
    fn prepare_evaluation_core(&mut self, first: usize, last: usize, y_offset: f32) {
        // INV-TS-ORDERING-SINGLE-SOURCE: sorted_indices is the only source of row ordering.
        // INV-TS-ORDERING-NONEMPTY: Must have ordering to realize.
        debug_assert!(
            !self.row_topology.sorted_indices().is_empty(),
            "INV-TS-ORDERING-NONEMPTY: sorted_indices must be set before prepare_evaluation"
        );

        let epoch = self.evaluation_guard.evaluation_epoch();
        self.body.realize_with_range(
            GeometryView::new(&self.geometry_table, epoch),
            self.row_topology.sorted_indices(),
            first,
            last,
            y_offset,
        );
        self.row_topology.on_realized();
        self.engine_invalidation.virtualization_dirty = false;
    }

    /// Mark this substrate as embedded in a ListViewCore.
    ///
    /// When embedded, the token-based scroll path (update_visible_range_from_scroll)
    /// is forbidden in debug builds. Scroll sync must use set_scroll_y_from_scroll_area.
    #[cfg(test)]
    pub(crate) fn set_listview_embedded(&mut self) {
        self.is_listview_embedded = true;
    }

    /// Whether this substrate is embedded in a ListViewCore.
    #[cfg(any(debug_assertions, test))]
    pub(crate) fn is_embedded_in_listview(&self) -> bool {
        self.is_listview_embedded
    }

    /// Layout-phase geometry resolution for embedded TabularSubstrate instances.
    ///
    /// Begins a new evaluation frame, resolves column geometry unconditionally,
    /// projects header, and realizes visible rows from the body's current scroll
    /// state and viewport.
    ///
    /// Called by wrapper layouts (ListViewBody) to guarantee geometry is resolved
    /// before any paint-phase reads (geometry_view, header_slots, etc.).
    ///
    /// Layout phase = resolution phase. No geometry reads may occur until this
    /// has run. This method also drains any pre-existing VisibleRangeToken to
    /// prevent INV-VZ-34 in subsequent frames.
    #[cfg(test)]
    pub(crate) fn resolve_layout_geometry(&mut self, available_width: f32, scale_factor: f32) {
        // SPEC-HAL-164: Begin new evaluation frame.
        self.evaluation_guard.begin_evaluation();
        self.evaluation_guard.enter_layout();

        // Process any pending intents (including sort intents).
        self.process_pending_intents();

        // SPEC-HAL-171: Lock RowModel mutation when sort is active.
        let projection_active = self.sort_state.is_active() || self.sort_state.is_dirty();
        if projection_active {
            self.sort_guard.begin_sort_processing();
        }

        // SPEC-HAL-171/258: Ensure sorted order via topology engine.
        self.ensure_topology();

        // SPEC-HAL-168: Resolve column geometry — unconditional, no conditional
        // based on virtualization or dirty flags.
        self.resolve_geometry(available_width, scale_factor);

        // SPEC-HAL-166: Project header cells from resolved geometry.
        let epoch = self.evaluation_guard.evaluation_epoch();
        self.column_guard.lock();
        self.header
            .project(GeometryView::new(&self.geometry_table, epoch));
        self.column_guard.unlock();

        // Layout owns realization this frame. Discard any pre-existing token
        // from scroll sync to prevent INV-VZ-34 on next frame.
        self.evaluation_mode = EvaluationMode::LayoutHandled;
        drop(self.visible_range.take());

        // SPEC-HAL-167: Realize visible rows from body's current scroll state.
        // Body viewport was set by parent widget (ListView::layout) before this runs.
        self.row_guard.unlock();
        let body_height = self.body.viewport().height;
        let viewport_top = self.body.scroll_y();
        let viewport_bottom = viewport_top + body_height;
        // SPEC-PM-017: f32 boundary shims — legacy tabular_view stays f32
        // internally until the rung-5 audit.
        let (first, last, y_offset) = self.virtualization.compute_visible_range(
            viewport_top as f64,
            viewport_bottom as f64,
            self.body.row_model().heights(),
            self.row_topology.sorted_indices(),
            self.row_topology.topology_version(),
        );
        let y_offset = y_offset as f32;
        self.body.realize_with_range(
            GeometryView::new(&self.geometry_table, epoch),
            self.row_topology.sorted_indices(),
            first,
            last,
            y_offset,
        );

        self.row_guard.lock();

        // SPEC-HAL-171: End sort processing.
        if projection_active {
            self.sort_guard.end_sort_processing();
        }
        debug_assert!(
            !self.sort_guard.is_sort_processing(),
            "SPEC-HAL-171: SortProcessingGuard stuck in locked state after layout"
        );
    }

    // =========================================================================
    // Scroll Control (SPEC-HAL-168)
    // =========================================================================

    /// Set horizontal scroll offset.
    ///
    /// SPEC-HAL-168 INVARIANT-7/8: Scroll is a pure transform applied after resolution.
    pub fn set_scroll_x(&mut self, offset: f32) {
        self.header.set_scroll_x(offset);
        self.body.set_scroll_x(offset);
    }

    /// Set vertical scroll offset.
    ///
    /// SPEC-HAL-174 INV-174-5: Scroll during editing → Cancel.
    pub fn set_scroll_y(&mut self, offset: f32) {
        if self.is_editing() && offset != self.body.scroll_y() {
            self.cancel_edit_for_destabilization();
        }
        self.body.set_scroll_y(offset);
        self.engine_invalidation.virtualization_dirty = true;
        self.scroll_epoch += 1;
    }

    /// Get horizontal scroll offset.
    pub fn scroll_x(&self) -> f32 {
        self.body.scroll_x()
    }

    /// Get vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.body.scroll_y()
    }

    /// Non-token scroll setter for ScrollArea sync.
    ///
    /// Updates scroll state (body scroll_y, scroll_epoch, virtualization_dirty)
    /// WITHOUT producing a VisibleRangeToken. The layout pass
    /// (resolve_layout_geometry) will realize rows for the new scroll position.
    ///
    /// Returns true if scroll_y changed (caller should mark layout dirty).
    ///
    /// SPEC-HAL-174 INV-174-5: Scroll during editing → Cancel.
    #[cfg(test)]
    pub(crate) fn set_scroll_y_from_scroll_area(&mut self, scroll_y: f32) -> bool {
        if scroll_y == self.body.scroll_y() {
            return false;
        }
        // SPEC-HAL-174 INV-174-5: Scroll during editing → Cancel.
        if self.is_editing() {
            self.cancel_edit_for_destabilization();
        }
        self.body.set_scroll_y(scroll_y);
        self.scroll_epoch += 1;
        self.engine_invalidation.virtualization_dirty = true;
        true
    }

    /// Update visible range from external scroll offset (like List pattern).
    ///
    /// This is called by the scroll sync mechanism (post_layout_sync_scroll_to_tabular_views)
    /// to update the scroll offset and detect if the visible range changed.
    ///
    /// Returns true if the visible range changed (caller should mark layout dirty).
    ///
    /// INV-SCROLL-SYNC: TabularSubstrate receives scroll from ScrollArea via this method,
    /// matching the List widget's update_visible_range_from_scroll pattern.
    ///
    /// INV-VZ-1/VZ-5: Uses VirtualizationEngine for O(1) fixed / O(log N) variable
    /// mapping instead of linear scan from row 0.
    ///
    /// SPEC-HAL-174 INV-174-5: Scroll during editing → Cancel.
    pub fn update_visible_range_from_scroll(&mut self, scroll_y: f32) -> bool {
        #[cfg(debug_assertions)]
        {
            // ListView must not use token-based scroll path.
            // ListViewBody::sync_scroll must call set_scroll_y_from_scroll_area().
            debug_assert!(
                !self.is_embedded_in_listview(),
                "ListView attempted to use update_visible_range_from_scroll (token path). \
                 This violates the single-authority layout model."
            );
        }
        // Handle edit destabilization
        if self.is_editing() && scroll_y != self.body.scroll_y() {
            self.cancel_edit_for_destabilization();
        }
        self.body.set_scroll_y(scroll_y);
        self.engine_invalidation.virtualization_dirty = true;
        self.scroll_epoch += 1;

        // Compute visible range via engine (O(1) fixed / O(log N) variable).
        // INV-VZ-20: This is the SINGLE compute point per frame for the scroll sync path.
        // Result is cached for consumption by prepare_evaluation — no recompute there.
        let viewport_bottom = scroll_y + self.body.viewport().height;
        // SPEC-PM-017: f32 boundary shims — legacy tabular_view stays f32
        // internally until the rung-5 audit.
        let (first, last, y_offset) = self.virtualization.compute_visible_range(
            scroll_y as f64,
            viewport_bottom as f64,
            self.body.row_model().heights(),
            self.row_topology.sorted_indices(),
            self.row_topology.topology_version(),
        );
        let y_offset = y_offset as f32;

        // INV-VZ-34: Must not overwrite an unconsumed token.
        debug_assert!(
            self.visible_range.is_none(),
            "INV-VZ-34: overwriting unconsumed VisibleRangeToken"
        );

        // INV-VZ-30: Store as single-use token with current scroll epoch.
        self.visible_range = Some(VisibleRangeToken {
            first,
            last,
            y_offset,
            epoch: self.scroll_epoch,
            _no_copy_no_clone: UnsafeCell::new(()),
        });

        self.evaluation_mode = EvaluationMode::TokenProduced;

        self.body.update_cached_visible_range((first, last))
    }

    // =========================================================================
    // Sort State Access (SPEC-HAL-171)
    // =========================================================================

    /// Get sort state (read-only).
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-1: Sort state is owned by TabularSubstrate.
    /// Header and Body may observe but not mutate.
    pub fn sort_state(&self) -> &SortState {
        &self.sort_state
    }

    /// Get sorted row indices (read-only).
    ///
    /// Returns the current realization order. Empty if no rows exist.
    pub fn sorted_indices(&self) -> &[usize] {
        self.row_topology.sorted_indices()
    }

    /// Set the row ordering for virtualization.
    ///
    /// **INV-TV-ORDERING-PERMUTATION**: `indices` must be a permutation of `0..row_count`.
    /// No duplicates, no out-of-range values. Caller owns ordering policy.
    ///
    /// For widgets like List that don't support sorting, this is identity order `(0..item_count)`.
    /// For sorted views, this reflects the current sort order.
    pub fn set_sorted_indices(&mut self, indices: Vec<usize>) {
        let row_count = self.body.row_model().len();
        self.row_topology.set_sorted_indices(indices, row_count);
    }

    // =========================================================================
    // Scroll-to-Reveal Computation (SPEC-HAL-131E equivalent for TabularSubstrate)
    // =========================================================================

    /// Compute the scroll_y needed to make a row visible.
    ///
    /// Returns `Some(scroll_y)` if the row is outside the viewport.
    /// Returns `None` if the row is already fully visible or if row_id is invalid.
    ///
    /// INV-TS-ORDERING-SINGLE-SOURCE: Uses sorted_indices for row position.
    pub fn compute_scroll_to_reveal_row(&self, row_id: RowId) -> Option<f32> {
        // Find row's index in RowModel
        let row_index = self.body.row_model().index_of(row_id)?;

        // Find row's position in current ordering
        let visual_position = self
            .row_topology
            .sorted_indices()
            .iter()
            .position(|&idx| idx == row_index)?;

        // Compute cumulative Y offset for the row
        let row_height = self.body.row_model().height_at(row_index);
        let mut row_top: f32 = 0.0;
        for i in 0..visual_position {
            let idx = self.row_topology.sorted_indices()[i];
            row_top += self.body.row_model().height_at(idx);
        }
        let row_bottom = row_top + row_height;

        // Get current viewport
        let viewport = self.body.viewport();
        let viewport_height = viewport.height;
        let current_scroll = self.body.scroll_y();
        let viewport_top = current_scroll;
        let viewport_bottom = current_scroll + viewport_height;

        // Check if row is already fully visible
        if row_top >= viewport_top && row_bottom <= viewport_bottom {
            return None;
        }

        // Compute scroll_y to reveal the row
        let scroll_y = if row_top < viewport_top {
            // Row is above viewport: top-align
            row_top
        } else {
            // Row is below viewport: bottom-align
            (row_bottom - viewport_height).max(0.0)
        };

        Some(scroll_y)
    }

    /// Compute the scroll_y for a specific row index (0-based visual position).
    ///
    /// Used for Home (index 0) and End (last index) navigation.
    /// Returns `Some(scroll_y)` to make the row visible, `None` if already visible.
    pub fn compute_scroll_to_reveal_index(&self, visual_index: usize) -> Option<f32> {
        let sorted = self.row_topology.sorted_indices();
        if visual_index >= sorted.len() {
            return None;
        }

        // Get the row_index from sorted_indices
        let row_index = sorted[visual_index];

        // Compute cumulative Y offset
        let row_height = self.body.row_model().height_at(row_index);
        let mut row_top: f32 = 0.0;
        for i in 0..visual_index {
            let idx = sorted[i];
            row_top += self.body.row_model().height_at(idx);
        }
        let row_bottom = row_top + row_height;

        // Get current viewport
        let viewport = self.body.viewport();
        let viewport_height = viewport.height;
        let current_scroll = self.body.scroll_y();
        let viewport_top = current_scroll;
        let viewport_bottom = current_scroll + viewport_height;

        // Check if row is already fully visible
        if row_top >= viewport_top && row_bottom <= viewport_bottom {
            return None;
        }

        // Compute scroll_y to reveal the row
        let scroll_y = if row_top < viewport_top {
            // Row is above viewport: top-align
            row_top
        } else {
            // Row is below viewport: bottom-align
            (row_bottom - viewport_height).max(0.0)
        };

        Some(scroll_y)
    }

    /// Set row comparator.
    ///
    /// SPEC-HAL-171: The comparator is called during sort order computation.
    /// SPEC-HAL-174A: During active edit, the comparator change is deferred
    /// to `pending_comparator` and applied when the edit session ends.
    pub fn set_comparator<C>(&mut self, comparator: C)
    where
        C: RowComparator + Send + Sync + 'static,
    {
        debug_assert!(
            !self.is_editing(),
            "SPEC-HAL-174: set_comparator forbidden during active edit session"
        );
        if self.is_editing() {
            self.pending_comparator = Some(Box::new(comparator));
            return;
        }
        self.comparator = Some(Box::new(comparator));
        // Mark sort as dirty to recompute with new comparator
        if self.sort_state.is_active() {
            self.sort_state
                .apply_intent(&intents::SortIntent::RequestSort {
                    column_id: self.sort_state.active_column().unwrap(),
                    direction: self.sort_state.direction(),
                });
        }
    }

    /// Clear row comparator.
    pub fn clear_comparator(&mut self) {
        self.comparator = None;
    }

    // =========================================================================
    // =========================================================================
    // Focus State (SPEC-HAL-175)
    // =========================================================================

    /// Get focus state (read-only).
    ///
    /// SPEC-HAL-175: Focus state is only mutated via FocusIntent.
    pub fn focus_state(&self) -> &FocusState {
        &self.focus_state
    }

    /// Get the legacy selection state (read-only).
    ///
    /// # DEPRECATED
    ///
    /// This accessor returns the legacy `SelectionState` which is a compatibility
    /// mirror only. Use `ordered_selection()` instead for all new code.
    ///
    /// **Migration path:**
    /// - `legacy_selection_state().is_selected(id)` → `ordered_selection().is_selected(id)`
    /// - `legacy_selection_state().selected_rows().iter()` → `ordered_selection().selected_keys_iter()`
    /// - `legacy_selection_state().has_selection()` → `!ordered_selection().is_selection_empty()`
    /// - `legacy_selection_state().count()` → `ordered_selection().selection_count()`
    /// - `legacy_selection_state().anchor()` → `ordered_selection().anchor_key()`
    ///
    /// This method will be removed once all consumers migrate.
    ///
    /// SPEC-HAL-178: Selection is independent of focus.
    #[deprecated(note = "Use ordered_selection() instead. This is a legacy compatibility mirror.")]
    pub fn legacy_selection_state(&self) -> &SelectionState {
        &self.selection_state
    }

    // =========================================================================
    // Ordered Selection Read API
    // =========================================================================
    //
    // This is the authoritative read surface for selection state.
    // When legacy selection_state is removed, this becomes the ONLY read path.
    //
    // =========================================================================
    // ARCHITECTURE INVARIANTS — READ BEFORE MODIFYING
    // =========================================================================
    //
    // INV-TAB-SEL-1: SINGLE WRITE AUTHORITY
    //   - ordered_selection_state is the ONLY authoritative selection truth
    //   - selection_state is a DEPRECATED MIRROR (write-only via mirror function)
    //   - All selection mutations flow through handle_selection_intent()
    //     → reducer → mirror_ordered_into_legacy_selection()
    //
    // INV-TAB-SEL-2: READ SURFACE SPLIT
    //   - ordered_selection() = AUTHORITATIVE (use for all new code)
    //   - selection_state()   = DEPRECATED MIRROR (legacy consumers only)
    //   - New consumers MUST use ordered_selection()
    //   - Existing consumers should migrate to ordered_selection()
    //
    // INV-TAB-SEL-3: FORBIDDEN ACTIONS
    //   - Widgets MUST NOT mutate selection directly (use SelectionIntent)
    //   - Widgets MUST NOT cache selection state (read from source each frame)
    //   - Consumers MUST NOT rely on selection iteration order (HashSet)
    //   - No code may mutate selection_state except mirror_ordered_into_legacy_selection()
    //
    // =========================================================================
    // Read Invariants (behavior guarantees)
    // =========================================================================
    //
    // 1. ORDERING GUARANTEE:
    //    - Selection is by RowId, not by visual index
    //    - selected_keys_iter() returns keys in arbitrary HashSet order
    //    - For ordered iteration, consumer must resolve keys through sorted_indices
    //
    // 2. VIRTUALIZATION INDEPENDENCE:
    //    - Selection survives scroll (rows not realized remain selected)
    //    - Selection survives sort (RowId is identity-stable)
    //    - Selection survives filter (hidden rows may remain selected)
    //
    // 3. TRANSACTIONAL VISIBILITY:
    //    - In Immediate mode: selected_keys == visible selection
    //    - In Transactional mode during transaction: selected_keys == working set
    //    - Use committed_selection() for last-committed state
    //
    // 4. FOCUS/SELECTION INDEPENDENCE:
    //    - Focus can be on an unselected row
    //    - Selection can exist without focus
    //    - Navigation does not implicitly mutate selection
    //
    // 5. WHAT IS NOT GUARANTEED:
    //    - No iteration order guarantee for selected_keys_iter()
    //    - No guarantee that selected RowIds still exist in row model
    //      (stale selections are silently tolerated until next mutation)
    //    - No derived "first selected" / "last selected" accessors
    //      (consumer must compute via sorted_indices if needed)
    //
    // =========================================================================

    /// Get the ordered selection state (read-only).
    ///
    /// This is the authoritative selection state. The legacy `selection_state()`
    /// method is deprecated and will be removed once all consumers migrate.
    ///
    /// # Available Queries
    ///
    /// **Selection:**
    /// - `is_selected(row_id)` — Check if a specific row is selected
    /// - `is_selection_empty()` — Check if no rows are selected
    /// - `selection_count()` — Get the number of selected rows
    /// - `selected_keys_iter()` — Iterate over selected RowIds (unordered)
    ///
    /// **Anchor:**
    /// - `anchor_key()` — Persistent anchor for range operations
    /// - `range_anchor_key()` — Dedicated range anchor for Shift+* operations
    ///
    /// **Transaction:**
    /// - `is_transactional()` — Check if transactional mode is configured
    /// - `is_transaction_active()` — Check if a transaction is in progress
    /// - `working_selection()` — Current working selection (during transaction)
    /// - `committed_selection()` — Last committed selection
    ///
    /// # Example
    ///
    /// ```ignore
    /// let sel = substrate.ordered_selection();
    /// if sel.is_selected(some_row_id) {
    ///     // Row is selected
    /// }
    /// for &row_id in sel.selected_keys_iter() {
    ///     // Process each selected row (order not guaranteed)
    /// }
    /// ```
    pub fn ordered_selection(&self) -> &OrderedSelectionState<row::RowId> {
        &self.ordered_selection_state
    }

    // =========================================================================
    // Selection Policy Configuration (ordered_selection)
    // =========================================================================

    /// Set selection mode.
    ///
    /// Configures how many items can be selected (None/Single/Multi).
    /// This is the authoritative configuration - List and other consumers
    /// should forward their configuration here.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.ordered_selection_state.selection_mode = mode;
    }

    /// Get selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.ordered_selection_state.selection_mode
    }

    /// Set interaction model.
    ///
    /// Configures whether selection is immediate or transactional.
    /// This is the authoritative configuration - List and other consumers
    /// should forward their configuration here.
    ///
    /// Note: Cannot be changed after first input event (locked via lock_interaction_model).
    pub fn set_interaction_model(&mut self, model: SelectionInteractionModel) {
        self.ordered_selection_state.set_interaction_model(model);
    }

    /// Get interaction model.
    pub fn interaction_model(&self) -> SelectionInteractionModel {
        self.ordered_selection_state.interaction_model
    }

    /// Check if transactional mode is configured.
    pub fn is_transactional(&self) -> bool {
        self.ordered_selection_state.is_transactional()
    }

    /// Lock the interaction model (called on first input event).
    ///
    /// After this call, set_interaction_model() becomes a no-op.
    pub fn lock_interaction_model(&mut self) {
        self.ordered_selection_state.lock_interaction_model();
    }

    // =========================================================================
    // SPEC-HAL-313: Row Expansion Configuration
    // =========================================================================

    /// SPEC-HAL-313 §3.2: Read-only access to expansion state.
    pub fn expansion(&self) -> &RowExpansion {
        &self.row_expansion
    }

    /// SPEC-HAL-313 §3.2: Mutable access to expansion state.
    ///
    /// Used by intent processing (handle_intent) to mutate expansion state
    /// in response to ExpansionIntent.
    pub fn expansion_mut(&mut self) -> &mut RowExpansion {
        &mut self.row_expansion
    }

    /// SPEC-HAL-313 §3.2: Set expansion policy.
    ///
    /// Configures how many rows can be expanded simultaneously.
    /// Does NOT clear existing expansions — if switching from Multi to
    /// Single while multiple rows are expanded, the caller must collapse
    /// extras explicitly.
    pub fn set_expansion_policy(&mut self, policy: ExpansionPolicy) {
        self.row_expansion.set_policy(policy);
    }

    /// SPEC-HAL-313 §3.2: Set expanded height policy.
    ///
    /// Configures how the expanded region height is determined from the
    /// child widget's natural size.
    pub fn set_expanded_height_policy(&mut self, policy: ExpandedHeightPolicy) {
        self.row_expansion.set_height_policy(policy);
    }

    // =========================================================================
    // Transaction State (ordered_selection passthrough)
    // =========================================================================

    /// Begin a transactional selection session.
    ///
    /// Returns true if transaction was started, false if:
    /// - Already in a transaction (nested transactions forbidden)
    /// - Not in transactional mode
    pub fn begin_transaction(&mut self) -> bool {
        self.ordered_selection_state.begin_transaction()
    }

    /// Check if a transaction is currently active.
    pub fn is_transaction_active(&self) -> bool {
        self.ordered_selection_state.is_transaction_active()
    }

    /// Commit the transactional selection session.
    ///
    /// Returns the committed selection snapshot, or None if no transaction was active.
    pub fn commit_transaction(&mut self) -> bool {
        self.ordered_selection_state.commit_transaction().is_some()
    }

    /// Cancel the transactional selection session.
    ///
    /// Restores working selection to committed selection.
    /// Returns true if a transaction was cancelled.
    pub fn cancel_transaction(&mut self) -> bool {
        let cancelled = self.ordered_selection_state.cancel_transaction();
        if cancelled {
            // Mirror reverted state to legacy selection_state
            self.mirror_ordered_into_legacy_selection();
        }
        cancelled
    }

    // =========================================================================
    // Edit State (SPEC-HAL-174)
    // =========================================================================

    /// Get edit state (read-only).
    ///
    /// SPEC-HAL-174 INV-174-2: Explicit lifecycle Idle/Armed/Editing.
    pub fn edit_state(&self) -> &EditState {
        &self.edit_state
    }

    /// Get edit session state (read-only).
    ///
    /// SPEC-HAL-174: Returns Some if in Editing state.
    pub fn edit_session(&self) -> Option<&EditSession> {
        self.edit_state.session()
    }

    /// Check if an edit session is active.
    ///
    /// SPEC-HAL-174 INV-174-1: Exactly one CellId at a time.
    pub fn is_editing(&self) -> bool {
        self.edit_state.is_active()
    }

    /// Get the content projection for the currently editing cell.
    ///
    /// SPEC-HAL-174 INV-174-4: CellContentProjection is the ONLY geometry
    /// source for editor placement.
    pub fn editing_content_projection(&self) -> Option<&CellContentProjection> {
        let cell_id = self.edit_state.editing_cell()?;
        self.body
            .content_projections()
            .iter()
            .find(|p| p.cell_id == cell_id)
    }

    /// Cancel the current edit due to a destabilizing operation.
    ///
    /// SPEC-HAL-174 INV-174-5: Scroll, sort, tree collapse, column hide
    /// during editing deterministically resolve to Cancel.
    /// Restores pre-edit focus, applies pending comparator.
    fn cancel_edit_for_destabilization(&mut self) {
        if let EditState::Editing(session) | EditState::Armed(session) = &self.edit_state {
            let pre_focus = session.pre_edit_focus;
            let focus_changed = self.focus_state.focused != pre_focus;
            self.focus_state.focused = pre_focus;
            if focus_changed {
                self.focus_state.version += 1;
            }
            self.edit_state = EditState::Idle;
            self.apply_pending_comparator();
        }
    }

    // =========================================================================
    // Hit Testing (SPEC-HAL-172)
    // =========================================================================

    /// Hit-test the body domain.
    ///
    /// Returns `BodyHitResult::Cell` for cell hits, None for misses.
    /// Tree-aware hit testing is handled by TreeView, not TabularSubstrate.
    pub fn body_hit_test(&self, x: f32, y: f32) -> Option<body::BodyHitResult> {
        self.body.hit_test(x, y).map(body::BodyHitResult::Cell)
    }

    // =========================================================================
    // Row Hit-Testing (INV-ROW-GEOM-SINGLE-SOURCE)
    // =========================================================================

    /// Hit-test to find the row at a given viewport-local Y coordinate.
    ///
    /// **INV-ROW-GEOM-SINGLE-SOURCE**: This method uses TabularSubstrate's internal
    /// geometry (row heights, offsets, scroll state) exclusively. No external
    /// row height or scroll state should be used for hit-testing.
    ///
    /// **INV-PAINT-HITTEST-IDENTITY**: A pixel that paints row R must hit-test
    /// to row R. This method uses the exact same geometry model as paint.
    ///
    // =========================================================================
    // Hit Testing (INV-TV-HITTEST-SPACE-EXPLICIT)
    // =========================================================================
    //
    // Two explicit APIs for different coordinate spaces:
    // - viewport_y: caller provides viewport-local Y, we add scroll
    // - content_y: caller provides content-local Y, we use directly
    //
    // This prevents "sometimes add scroll, sometimes don't" ambiguity.
    // =========================================================================

    /// Hit-test to find the row at a VIEWPORT-LOCAL Y coordinate.
    ///
    /// INV-TV-HITTEST-SPACE-EXPLICIT: This variant expects viewport-local Y
    /// (relative to visible viewport top) and internally adds scroll_y.
    ///
    /// Use this when the caller has viewport-local coordinates (e.g., from
    /// `global_pos - widget_rect` where widget_rect doesn't include scroll).
    ///
    /// # Arguments
    /// * `viewport_local_y` - Y relative to the top of the visible viewport.
    pub fn hit_test_row_id_viewport_y(&self, viewport_local_y: f32) -> Option<RowId> {
        if viewport_local_y < 0.0 {
            return None;
        }
        // Transform viewport-local → content-local by adding scroll offset.
        let content_y = viewport_local_y + self.body.scroll_y();
        self.hit_test_row_id_content_y(content_y)
    }

    /// Hit-test to find the row at a CONTENT-LOCAL Y coordinate.
    ///
    /// INV-TV-HITTEST-SPACE-EXPLICIT: This variant expects content-local Y
    /// (already includes scroll offset) and does NOT add scroll_y.
    ///
    /// Use this when the caller has content-local coordinates (e.g., from
    /// `pe.local_pos` which the event system computes as pointer + scroll - rect).
    ///
    /// # Arguments
    /// * `content_y` - Y in content space (viewport_y + scroll_y).
    pub fn hit_test_row_id_content_y(&self, content_y: f32) -> Option<RowId> {
        if content_y < 0.0 {
            return None;
        }
        // Use RowModel's row_at_y which respects actual row heights.
        // INV-ROW-GEOM-SINGLE-SOURCE: No external item_height used here.
        let row_index = self.body.row_model().row_at_y(content_y)?;
        self.body.row_model().get(row_index)
    }

    /// Hit-test to find the row INDEX at a CONTENT-LOCAL Y coordinate.
    ///
    /// Similar to `hit_test_row_id_content_y` but returns index instead of RowId.
    pub fn hit_test_row_index_content_y(&self, content_y: f32) -> Option<usize> {
        if content_y < 0.0 {
            return None;
        }
        self.body.row_model().row_at_y(content_y)
    }

    /// DEPRECATED: Use `hit_test_row_id_viewport_y` or `hit_test_row_id_content_y` instead.
    ///
    /// This wrapper exists for backward compatibility. It assumes viewport-local input.
    #[deprecated(
        note = "Use hit_test_row_id_viewport_y or hit_test_row_id_content_y for explicit space"
    )]
    pub fn hit_test_row_id(&self, viewport_local_y: f32) -> Option<RowId> {
        self.hit_test_row_id_viewport_y(viewport_local_y)
    }

    /// DEPRECATED: Use `hit_test_row_index_content_y` instead.
    #[deprecated(note = "Use hit_test_row_index_content_y for explicit space")]
    pub fn hit_test_row_index(&self, viewport_local_y: f32) -> Option<usize> {
        if viewport_local_y < 0.0 {
            return None;
        }
        let content_y = viewport_local_y + self.body.scroll_y();
        self.body.row_model().row_at_y(content_y)
    }

    // =========================================================================
    // Intent Handling
    // =========================================================================
    //
    // INTENT AUTHORITY INVARIANTS (SPEC-HAL-174):
    //
    // * INV-174-5: Sort/tree/focus intents during edit → cancel edit first,
    //   then apply the intent normally.
    // * Resize intents pass through (do not affect edit).
    // * Intent sources (HeaderState, BodyDomain): pure constructors, no queueing.
    //   External caller feeds returned intents into handle_intent directly,
    //   or into emit_intent for deferred processing.
    // * emit_intent is the sole queue entry point. No public alternative exists.
    //
    // =========================================================================

    /// Drain pending intents in FIFO order without executing them.
    pub fn take_intents(&mut self) -> Vec<TabularIntent> {
        std::mem::take(&mut self.pending_intents)
    }

    /// Admit an intent to the deferred queue under SPEC-HAL-177A.
    /// The callback observes admission before the intent is enqueued.
    #[allow(
        dead_code,
        reason = "HAL-177A retains the dormant private deferred intent entry"
    )]
    fn emit_intent(&mut self, intent: TabularIntent) {
        if let Some(handler) = &self.on_intent {
            handler(&intent);
        }
        self.pending_intents.push(intent);
    }

    /// Process a resize intent from header interaction.
    ///
    /// SPEC-HAL-180 INV-COL-RESIZE-1: Two-column delta transfer.
    /// SPEC-HAL-180 INV-COL-RESIZE-2: Last column right edge immovable.
    /// SPEC-HAL-180 INV-COL-RESIZE-3: Hard minimum on neighbor.
    /// SPEC-HAL-169 INVARIANT-5: Substrate applies clamping, not interaction code.
    fn elastic_freeze_snapshot_for_resize(
        &self,
        target_column_id: ColumnId,
    ) -> Vec<(ColumnId, f32)> {
        let epoch = self.evaluation_guard.evaluation_epoch();
        if !self.geometry_table.is_resolved() || self.geometry_table.is_stale(epoch) {
            return Vec::new();
        }

        self.geometry_table
            .header_slots(epoch)
            .filter_map(|slot| {
                if slot.column_id == target_column_id {
                    return None;
                }
                let column = self.column_model.get(slot.column_id)?;
                column
                    .flex
                    .is_some()
                    .then_some((slot.column_id, slot.width))
            })
            .collect()
    }

    fn handle_resize_intent(&mut self, intent: &intents::ResizeIntent) {
        match intent {
            intents::ResizeIntent::Commit {
                column_id,
                proposed_width,
            } => {
                self.column_guard.assert_mutation_allowed();

                // SPEC-HAL-165 INV-1: column-geometry contract (per-column
                // resizable gate, clamp with min/max + MIN_COL_WIDTH floor,
                // INV-RESIZE-CLEARS-ELASTIC, set preferred_width) lives on
                // ColumnModel. handle_resize_intent is the substrate-level
                // entry point that adds the interaction-model guard and
                // tracks invalidation; the model performs the mutation.
                let elastic_freeze = self.elastic_freeze_snapshot_for_resize(*column_id);
                let froze_elastic = self.column_model.freeze_elastic_widths(elastic_freeze);
                let resized = self
                    .column_model
                    .commit_user_resize(*column_id, *proposed_width);

                if froze_elastic || resized {
                    self.engine_invalidation.geometry_dirty = true;
                }
            }
            _ => {
                // Begin/Update/Cancel: no mutation
            }
        }
    }

    /// Process a visibility intent.
    ///
    /// SPEC-HAL-164 INVARIANT-A1: Visibility mutations route through intent pipeline.
    fn handle_visibility_intent(&mut self, intent: &intents::VisibilityIntent) {
        match intent {
            intents::VisibilityIntent::SetVisible { column_id, visible } => {
                self.column_guard.assert_mutation_allowed();
                self.column_model.set_visible(*column_id, *visible);
                self.engine_invalidation.geometry_dirty = true;
            }
        }
    }

    /// Process a width intent.
    ///
    /// SPEC-HAL-164 INVARIANT-A1: Width mutations route through intent pipeline.
    fn handle_reorder_intent(&mut self, intent: &intents::ReorderIntent) {
        match intent {
            intents::ReorderIntent::Apply { new_order } => {
                self.column_guard.assert_mutation_allowed();
                if self.column_model.reorder(new_order) {
                    self.engine_invalidation.geometry_dirty = true;
                }
            }
        }
    }

    fn handle_width_intent(&mut self, intent: &intents::WidthIntent) {
        match intent {
            intents::WidthIntent::SetWidth { column_id, width } => {
                self.column_guard.assert_mutation_allowed();
                let clamped = width.max(column::MIN_COL_WIDTH);
                self.column_model.set_preferred_width(*column_id, clamped);
                self.engine_invalidation.geometry_dirty = true;
            }
        }
    }

    /// Process a sort intent.
    ///
    /// SPEC-HAL-171 INVARIANT-SORT-2: Sort changes happen ONLY via SortIntent.
    /// SPEC-HAL-171 INVARIANT-SORT-5: May only be called in update/handle_intent path,
    /// not during paint/input phases.
    fn handle_sort_intent(&mut self, intent: &intents::SortIntent) {
        self.sort_state.apply_intent(intent);
    }

    /// Process a focus/navigation intent.
    ///
    /// SPEC-HAL-175: FocusState is only mutated here.
    /// Sort and tree intents never call this path.
    fn handle_focus_intent(&mut self, intent: &intents::FocusIntent) {
        match intent {
            intents::FocusIntent::RequestFocus { row_id, column_id } => {
                let cell = row::CellId::new(*row_id, *column_id);
                self.focus_state.focused = Some(cell);
                self.focus_state.version += 1;
            }
            intents::FocusIntent::RequestClearFocus => {
                if self.focus_state.focused.is_some() {
                    self.focus_state.focused = None;
                    self.focus_state.anchor = None;
                    self.focus_state.version += 1;
                }
            }
            intents::FocusIntent::RequestNavigate { direction, .. } => {
                if let Some(target) = self.resolve_navigation(*direction) {
                    self.focus_state.focused = Some(target);
                    self.focus_state.version += 1;
                }
            }
        }
    }

    /// Process a selection intent.
    ///
    /// SPEC-HAL-178: SelectionState is only mutated here.
    /// Focus, sort, tree intents never call this path.
    /// INV-178-2: FocusState is never mutated here.
    ///
    /// Implementation routes through ordered_selection reducer, then syncs
    /// back to legacy selection_state for backward compatibility.
    fn handle_selection_intent(&mut self, intent: &intents::SelectionIntent) {
        // TRACE-LISTVIEW: Log selection state before/after
        #[cfg(feature = "trace-listview")]
        let before_count = self.selection_state.selected_rows.len();
        #[cfg(feature = "trace-listview")]
        let before_anchor = self.selection_state.anchor;

        // =====================================================================
        // Route through ordered_selection reducer
        // =====================================================================
        // Map SelectionIntent → OrderedSelectionAction, apply reducer, sync back.
        // SetSelection is a direct state replacement and bypasses the reducer.
        // =====================================================================

        let action = match intent {
            intents::SelectionIntent::SelectRow { row_id } => {
                Some(OrderedSelectionAction::Select { key: *row_id })
            }
            intents::SelectionIntent::ExtendToRow { row_id } => {
                Some(OrderedSelectionAction::ExtendTo { key: *row_id })
            }
            intents::SelectionIntent::ToggleRow { row_id } => {
                Some(OrderedSelectionAction::Toggle { key: *row_id })
            }
            intents::SelectionIntent::ClearSelection => {
                Some(OrderedSelectionAction::ClearSelection)
            }
            intents::SelectionIntent::SetSelection { .. } => {
                // SetSelection is a direct state replacement, handled below
                None
            }
        };

        if let Some(action) = action {
            // Apply reducer with RowModel as oracle
            let _result = reduce_ordered_selection(
                &mut self.ordered_selection_state,
                action,
                self.body.row_model(),
            );

            // Mirror reducer output to legacy selection_state
            self.mirror_ordered_into_legacy_selection();
        } else if let intents::SelectionIntent::SetSelection { rows, anchor } = intent {
            // Direct state replacement for SetSelection
            // Update ordered_selection_state (authoritative), then mirror to legacy
            let current_keys: std::collections::HashSet<_> = self
                .ordered_selection_state
                .selected_keys
                .iter()
                .copied()
                .collect();
            let new_keys: std::collections::HashSet<_> = rows.iter().copied().collect();
            let keys_changed = current_keys != new_keys;
            let anchor_changed = self.ordered_selection_state.anchor_key != *anchor;

            if keys_changed || anchor_changed {
                self.ordered_selection_state.selected_keys.clear();
                for row_id in rows {
                    self.ordered_selection_state.selected_keys.insert(*row_id);
                }
                self.ordered_selection_state.anchor_key = *anchor;

                // Mirror to legacy selection_state
                self.mirror_ordered_into_legacy_selection();
            }
        }

        // TRACE-LISTVIEW: Log selection state after
        #[cfg(feature = "trace-listview")]
        tracing::info!(
            "[TABULAR.handle_selection_intent] intent={:?} before=(anchor={:?}, count={}) after=(anchor={:?}, count={})",
            intent,
            before_anchor,
            before_count,
            self.selection_state.anchor,
            self.selection_state.selected_rows.len()
        );
    }

    /// Mirror ordered_selection_state into legacy selection_state.
    ///
    /// INVARIANT: This is the ONLY function that may mutate selection_state.
    /// selection_state is deprecated and exists solely for backward compatibility
    /// during migration. All selection logic flows through ordered_selection_state;
    /// this function mirrors that state into the legacy container.
    ///
    /// Once all consumers migrate to reading from ordered_selection_state,
    /// both this function and selection_state will be removed.
    fn mirror_ordered_into_legacy_selection(&mut self) {
        // Convert HashSet to Vec (order is not preserved, but selection_state
        // doesn't guarantee order either)
        let new_rows: Vec<row::RowId> = self
            .ordered_selection_state
            .selected_keys
            .iter()
            .copied()
            .collect();
        let new_anchor = self.ordered_selection_state.anchor_key;

        let rows_changed = self.selection_state.selected_rows != new_rows;
        let anchor_changed = self.selection_state.anchor != new_anchor;

        if rows_changed || anchor_changed {
            self.selection_state.selected_rows = new_rows;
            self.selection_state.anchor = new_anchor;
            self.selection_state.version += 1;
        }
    }

    /// Compute all rows between anchor and target in effective order.
    ///
    /// SPEC-HAL-178 INV-178-7: Range computed over effective_row_order().
    #[allow(dead_code, reason = "Retained effective-order row-range computation")]
    fn compute_row_range(&mut self, anchor: row::RowId, target: row::RowId) -> Vec<row::RowId> {
        let row_order = self.effective_row_order();
        let anchor_pos = row_order
            .iter()
            .position(|&idx| self.body.row_model().get(idx) == Some(anchor));
        let target_pos = row_order
            .iter()
            .position(|&idx| self.body.row_model().get(idx) == Some(target));

        match (anchor_pos, target_pos) {
            (Some(a), Some(t)) => {
                let (start, end) = if a <= t { (a, t) } else { (t, a) };
                row_order[start..=end]
                    .iter()
                    .filter_map(|&idx| self.body.row_model().get(idx))
                    .collect()
            }
            _ => {
                // Anchor or target not in effective order — select just the target
                vec![target]
            }
        }
    }

    /// Resolve navigation direction to a target cell.
    ///
    /// SPEC-HAL-175: Uses effective row order (tree + sort aware) and
    /// visible columns. Clamps to bounds, never panics.
    /// If focus is None, defaults to top-left visible cell.
    fn resolve_navigation(
        &mut self,
        direction: intents::NavigationDirection,
    ) -> Option<row::CellId> {
        let row_order = self.effective_row_order();
        if row_order.is_empty() {
            return None;
        }

        let visible_cols: Vec<column::ColumnId> =
            self.column_model.visible_columns().map(|c| c.id).collect();
        if visible_cols.is_empty() {
            return None;
        }

        // Find current position in the effective ordering
        let (row_pos, col_pos) = match self.focus_state.focused {
            Some(cell_id) => {
                let rp = row_order
                    .iter()
                    .position(|&idx| self.body.row_model().get(idx) == Some(cell_id.row_id))
                    .unwrap_or(0);
                let cp = visible_cols
                    .iter()
                    .position(|&id| id == cell_id.column_id)
                    .unwrap_or(0);
                (rp, cp)
            }
            None => (0, 0), // Default: top-left visible cell
        };

        let last_row = row_order.len() - 1;
        let last_col = visible_cols.len() - 1;

        let (new_row, new_col) = match direction {
            intents::NavigationDirection::Up => (row_pos.saturating_sub(1), col_pos),
            intents::NavigationDirection::Down => ((row_pos + 1).min(last_row), col_pos),
            intents::NavigationDirection::Left => (row_pos, col_pos.saturating_sub(1)),
            intents::NavigationDirection::Right => (row_pos, (col_pos + 1).min(last_col)),
            intents::NavigationDirection::Home => (row_pos, 0),
            intents::NavigationDirection::End => (row_pos, last_col),
            intents::NavigationDirection::PageUp => {
                let page = self.page_size();
                (row_pos.saturating_sub(page), col_pos)
            }
            intents::NavigationDirection::PageDown => {
                let page = self.page_size();
                ((row_pos + page).min(last_row), col_pos)
            }
        };

        let target_row_idx = row_order[new_row];
        let target_row_id = self.body.row_model().get(target_row_idx)?;
        let target_col_id = visible_cols[new_col];

        Some(row::CellId::new(target_row_id, target_col_id))
    }

    /// Compute effective row order from sort state.
    ///
    /// SPEC-HAL-175/178: Navigation and selection use this ordering.
    ///
    /// INV-TS-ORDERING-SINGLE-SOURCE: sorted_indices is the only source of row ordering.
    /// This method returns sorted_indices — never generates implicit ordering.
    ///
    /// INV-ROW-ORDER-EPOCH-3: Self-healing — recomputes if stale, never panics from
    /// caller forgetting to precompute. Postcondition asserts epoch correctness.
    pub(crate) fn effective_row_order(&mut self) -> Vec<usize> {
        // SPEC-HAL-261 INVARIANT-261-RT-1: Engine owns ALL gating.
        self.ensure_topology();

        // Postcondition: epoch must now match
        debug_assert!(
            self.row_topology.sorted_indices_epoch() == self.row_topology.base_row_order_epoch(),
            "INV-ROW-ORDER-EPOCH-3 POSTCONDITION FAILED: sorted_indices_epoch ({}) != base_row_order_epoch ({}) after recompute",
            self.row_topology.sorted_indices_epoch(),
            self.row_topology.base_row_order_epoch()
        );

        // INV-TS-ORDERING-SINGLE-SOURCE: Return sorted_indices — the authoritative ordering.
        self.row_topology.sorted_indices().to_vec()
    }

    /// Compute page size for PageUp/PageDown navigation.
    ///
    /// Uses viewport height and first row height as representative.
    pub(crate) fn page_size(&self) -> usize {
        let viewport_height = self.body.viewport().height;
        if viewport_height <= 0.0 {
            return 10; // Default when viewport not set
        }
        let row_height = if !self.body.row_model().is_empty() {
            self.body.row_model().height_at(0)
        } else {
            24.0
        };
        if row_height <= 0.0 {
            return 10;
        }
        (viewport_height / row_height).floor().max(1.0) as usize
    }

    /// Handle an intent from the pending queue.
    ///
    /// SPEC-HAL-171/172/174: Called during update phase to process accumulated intents.
    /// SPEC-HAL-174 INV-174-5: Sort/Tree/Focus intents during edit → cancel edit,
    /// then apply the intent normally. Resize passes through.
    pub fn handle_intent(&mut self, intent: &TabularIntent) {
        // NOTE: Interaction model locking is NOT done here.
        // Locking happens at the input-driven entrypoints (List::handle_event,
        // handle_keyboard, handle_projected_action) so that programmatic
        // selection/focus changes do not lock the model.

        match intent {
            TabularIntent::Resize(resize) => self.handle_resize_intent(resize),
            TabularIntent::Sort(sort) => {
                if self.is_editing() {
                    self.cancel_edit_for_destabilization();
                }
                self.handle_sort_intent(sort);
            }
            TabularIntent::Tree(_) => {
                // Tree intents are handled by TreeView, not flat TabularSubstrate.
                // No-op here. TreeView uses HierarchicalTabularSubstrate directly.
            }
            TabularIntent::Focus(focus) => {
                if self.is_editing() {
                    self.cancel_edit_for_destabilization();
                }
                self.handle_focus_intent(focus);
            }
            TabularIntent::Edit(edit) => self.handle_edit_intent(edit),
            TabularIntent::Selection(sel) => {
                if self.is_editing() {
                    self.cancel_edit_for_destabilization();
                }
                self.handle_selection_intent(sel);
            }
            TabularIntent::Activation(_) => {
                // SPEC-HAL-200: Activation intents are forwarded to the consumer
                // (ListView, List, etc.) via callback. The substrate does not
                // handle activation directly — it has no knowledge of what
                // "activate" means for a given row.
            }
            TabularIntent::Expansion(expansion) => {
                // SPEC-HAL-313 §3.2: Mutate RowExpansion state.
                // Height propagation is deferred to body layout (T5/T6).
                // The body detects RowExpansion.version changes and applies
                // heights + mounts/unmounts child widgets.
                use crate::widgets::tabular_view::intents::ExpansionIntent;
                match expansion {
                    ExpansionIntent::Toggle { row_id } => {
                        self.row_expansion.toggle(*row_id);
                    }
                    ExpansionIntent::Expand { row_id } => {
                        self.row_expansion.expand(*row_id);
                    }
                    ExpansionIntent::Collapse { row_id } => {
                        self.row_expansion.collapse(*row_id);
                    }
                    ExpansionIntent::ToggleWithCapture { row_id } => {
                        // Phase A stub: behave as Toggle.
                        // SPEC-HAL-176 Phase 3 will add focus scope push.
                        tracing::warn!(
                            "SPEC-HAL-313: ToggleWithCapture stubbed as Toggle \
                             (focus scopes not yet implemented)"
                        );
                        self.row_expansion.toggle(*row_id);
                    }
                }
            }
            TabularIntent::Visibility(vis) => {
                // SPEC-HAL-174 INV-174-5: Hiding edited column cancels edit.
                if let intents::VisibilityIntent::SetVisible {
                    column_id,
                    visible: false,
                } = vis
                {
                    if let Some(cell_id) = self.edit_state.editing_cell() {
                        if cell_id.column_id == *column_id {
                            self.cancel_edit_for_destabilization();
                        }
                    }
                }
                self.handle_visibility_intent(vis);
            }
            TabularIntent::Width(w) => {
                self.handle_width_intent(w);
            }
            TabularIntent::Reorder(reorder) => {
                if self.is_editing() {
                    self.cancel_edit_for_destabilization();
                }
                self.handle_reorder_intent(reorder);
            }
        }
    }

    // -------------------------------------------------------------------------
    // EDIT SESSION INVARIANTS (SPEC-HAL-174 + SPEC-HAL-174A):
    //
    // * Comparator changes are configuration, not intents.
    //   They do NOT flow through handle_intent or emit_intent.
    // * During active edit, set_comparator defers to pending_comparator (SPEC-HAL-174A).
    //   Sort state is never dirtied while is_editing() == true.
    // * On edit end (commit/cancel), apply_pending_comparator is called.
    //   This is the sole post-edit comparator application path.
    // * All comparator application funnels through exactly two paths:
    //   - set_comparator (normal, not editing)
    //   - apply_pending_comparator (post-edit only)
    // -------------------------------------------------------------------------

    /// Process an edit intent.
    ///
    /// SPEC-HAL-174 INV-174-2: Explicit lifecycle transitions.
    /// BeginEdit: Idle → Editing (Armed → Editing immediate). Nested begin forbidden.
    /// CommitEdit: Editing → Idle. Focus remains on edited cell. Target must match.
    /// CancelEdit: Editing → Idle. Restores pre-edit focus. Target must match.
    fn handle_edit_intent(&mut self, intent: &intents::EditIntent) {
        match intent {
            intents::EditIntent::RequestBeginEdit { row_id, column_id } => {
                debug_assert!(
                    !self.is_editing(),
                    "SPEC-HAL-174 INV-174-1: nested RequestBeginEdit forbidden"
                );
                if self.is_editing() {
                    return;
                }
                let cell = row::CellId::new(*row_id, *column_id);
                let session = EditSession {
                    editing_cell: cell,
                    pre_edit_focus: self.focus_state.focused,
                };
                // Armed → Editing is immediate (synchronous)
                self.edit_state = EditState::Editing(session);
                // Focus moves to the edited cell
                self.focus_state.focused = Some(cell);
                self.focus_state.version += 1;
                // SPEC-HAL-178 INV-178-5: Beginning edit clears selection
                // Clear ordered_selection_state (authoritative), then mirror to legacy
                if !self.ordered_selection_state.selected_keys.is_empty()
                    || self.ordered_selection_state.anchor_key.is_some()
                {
                    self.ordered_selection_state.selected_keys.clear();
                    self.ordered_selection_state.anchor_key = None;
                    self.mirror_ordered_into_legacy_selection();
                }
            }
            intents::EditIntent::RequestCommitEdit {
                row_id, column_id, ..
            } => {
                if let EditState::Editing(ref session) = self.edit_state {
                    let target = row::CellId::new(*row_id, *column_id);
                    debug_assert!(
                        session.editing_cell == target,
                        "SPEC-HAL-174: commit target mismatch (expected {:?}, got {:?})",
                        session.editing_cell,
                        target
                    );
                    if session.editing_cell != target {
                        return;
                    }
                    // Focus remains on the edited cell
                    let cell = session.editing_cell;
                    let focus_changed = self.focus_state.focused != Some(cell);
                    self.focus_state.focused = Some(cell);
                    if focus_changed {
                        self.focus_state.version += 1;
                    }
                    self.edit_state = EditState::Idle;
                    self.apply_pending_comparator();
                }
            }
            intents::EditIntent::RequestCancelEdit { row_id, column_id } => {
                if let EditState::Editing(ref session) = self.edit_state {
                    let target = row::CellId::new(*row_id, *column_id);
                    debug_assert!(
                        session.editing_cell == target,
                        "SPEC-HAL-174: cancel target mismatch (expected {:?}, got {:?})",
                        session.editing_cell,
                        target
                    );
                    if session.editing_cell != target {
                        return;
                    }
                    let pre_focus = session.pre_edit_focus;
                    let focus_changed = self.focus_state.focused != pre_focus;
                    self.focus_state.focused = pre_focus;
                    if focus_changed {
                        self.focus_state.version += 1;
                    }
                    self.edit_state = EditState::Idle;
                    self.apply_pending_comparator();
                }
            }
        }
    }

    /// Apply a deferred comparator change after edit session ends.
    ///
    /// SPEC-HAL-174A: Sole post-edit application path for deferred comparators.
    /// Re-dirties sort state for the next layout recomputation.
    fn apply_pending_comparator(&mut self) {
        if let Some(comparator) = self.pending_comparator.take() {
            self.comparator = Some(comparator);
            if self.sort_state.is_active() {
                self.sort_state
                    .apply_intent(&intents::SortIntent::RequestSort {
                        column_id: self.sort_state.active_column().unwrap(),
                        direction: self.sort_state.direction(),
                    });
            }
        }
    }

    /// Process all pending intents.
    ///
    /// Called at the start of layout to apply any queued intent changes.
    fn process_pending_intents(&mut self) {
        // Take intents to avoid borrow issues
        let intents = std::mem::take(&mut self.pending_intents);
        for intent in &intents {
            self.handle_intent(intent);
        }
    }

    /// Sort a set of row indices in-place using the current sort state.
    ///
    /// SPEC-HAL-171: Applies sort ordering to an arbitrary index set.
    /// Used when tree projection filters rows before sort.
    #[allow(dead_code, reason = "Retained subset ordering helper")]
    fn sort_visible_indices(&self, indices: &mut Vec<usize>) {
        let (column_id, direction) =
            match (self.sort_state.active_column(), self.sort_state.direction()) {
                (Some(col), Some(dir)) => (col, dir),
                _ => return, // No active sort
            };

        if let Some(ref comparator) = self.comparator {
            indices.sort_by(|&a, &b| match comparator.compare_rows(a, b, column_id) {
                Some(ord) => match direction {
                    intents::SortDirection::Ascending => ord,
                    intents::SortDirection::Descending => ord.reverse(),
                },
                None => a.cmp(&b),
            });
        }
    }

    // =========================================================================
    // Layout Implementation
    // =========================================================================

    /// Resolve column geometry for the current frame.
    ///
    /// SPEC-HAL-168: This is the SINGLE resolution point.
    /// Called exactly once per layout pass.
    ///
    /// SPEC-HAL-258: Substrate decides recompute vs epoch-stamp.
    /// No engine autonomy — all full recompute triggers are substrate-driven.
    fn resolve_geometry(&mut self, available_width: f32, scale_factor: f32) {
        // SPEC-HAL-258: Topology → Virtualization invalidation invariant.
        debug_assert!(
            !self.row_topology.is_topology_dirty() || self.engine_invalidation.virtualization_dirty,
            "INV-HAL-258: topology_dirty requires virtualization_dirty"
        );

        // SPEC-HAL-258: Detect container width change → mark geometry dirty.
        let snapped = available_width.round();
        if snapped != self.engine_invalidation.last_geometry_width {
            self.engine_invalidation.geometry_dirty = true;
            self.engine_invalidation.last_geometry_width = snapped;
        }

        let epoch = self.evaluation_guard.evaluation_epoch();

        // SPEC-HAL-258: Substrate decides recompute vs epoch-stamp.
        if self.engine_invalidation.geometry_dirty {
            self.geometry_table
                .resolve(&self.column_model, available_width, epoch, scale_factor);
        } else {
            self.geometry_table.stamp_epoch(epoch);
        }

        self.evaluation_guard.mark_geometry_resolved();
        self.engine_invalidation.geometry_dirty = false;
    }

    /// Logical row count — single query surface for all row count reads.
    ///
    /// SPEC-HAL-256: Delegates through RowTopologyEngine. The engine is a
    /// pure passthrough today; future extraction will intercept here.
    #[allow(dead_code, reason = "Retained topology-engine query delegation")]
    fn logical_row_count(&self) -> usize {
        let count = self.body.row_model().len();
        self.row_topology.logical_row_count(count)
    }

    /// Visible row range — single query surface for virtualization reads.
    ///
    /// SPEC-HAL-257: Delegates through VirtualizationEngine. The engine is a
    /// pure passthrough today; future extraction will intercept here.
    #[allow(dead_code, reason = "Retained virtualization-engine query delegation")]
    fn visible_row_range(&self, start: usize, end: usize) -> (usize, usize) {
        self.virtualization.visible_range_from(start, end)
    }
}

impl Default for TabularSubstrate {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Test-only accessors for internal verification
// =============================================================================
#[cfg(test)]
impl TabularSubstrate {
    /// Test accessor: body domain reference.
    pub(crate) fn body_ref(&self) -> &BodyDomain {
        &self.body
    }

    /// Test accessor: begin evaluation.
    pub(crate) fn test_begin_evaluation(&mut self) {
        self.evaluation_guard.begin_evaluation();
    }

    /// Test accessor: mark geometry resolved.
    pub(crate) fn test_mark_geometry_resolved(&mut self) {
        self.evaluation_guard.mark_geometry_resolved();
    }

    /// Test accessor: resolve geometry (routes through substrate resolve_geometry).
    ///
    /// SPEC-HAL-258: Uses the substrate's dirty-flag-aware path, not the engine directly.
    /// This also calls mark_geometry_resolved() internally.
    pub(crate) fn test_resolve_geometry(&mut self, available_width: f32) {
        self.resolve_geometry(available_width, 1.0);
    }

    /// Test accessor: debug geometry resolve counters.
    #[cfg(debug_assertions)]
    pub(crate) fn debug_geometry_counters(&self) -> (u64, u64) {
        (
            self.geometry_table.debug_full_resolve_count(),
            self.geometry_table.debug_short_circuit_count(),
        )
    }

    /// Test accessor: base row order epoch for invariant verification.
    ///
    /// INV-ROW-ORDER-EPOCH-1: Increments by exactly +1 on structural row mutations.
    pub(crate) fn base_row_order_epoch_for_test(&self) -> u64 {
        self.row_topology.base_row_order_epoch()
    }

    /// Test accessor: realize viewport with current state.
    /// INV-VZ-11: Uses VirtualizationEngine for range, then realize_with_range.
    pub(crate) fn test_realize_viewport(&mut self) {
        let viewport_top = self.body.scroll_y();
        let viewport_bottom = viewport_top + self.body.viewport().height;
        let sorted = self.row_topology.sorted_indices();
        let version = self.row_topology.topology_version();
        // SPEC-PM-017: f32 boundary shims — test accessor mirrors the
        // production call shape; rung-5 audit.
        let (first, last, y_offset) = self.virtualization.compute_visible_range(
            viewport_top as f64,
            viewport_bottom as f64,
            self.body.row_model().heights(),
            sorted,
            version,
        );
        let y_offset = y_offset as f32;
        let epoch = self.evaluation_guard.evaluation_epoch();
        let sorted = self.row_topology.sorted_indices();
        self.body.realize_with_range(
            GeometryView::new(&self.geometry_table, epoch),
            sorted,
            first,
            last,
            y_offset,
        );
    }

    /// Test accessor: set topology dirty flag (SPEC-HAL-258: engine-owned).
    pub(crate) fn test_set_topology_dirty(&mut self, dirty: bool) {
        self.row_topology.set_topology_dirty(dirty);
    }

    /// Test accessor: topology version.
    pub(crate) fn test_topology_version(&self) -> u64 {
        self.row_topology.topology_version()
    }

    /// Test accessor: compute visible range via virtualization engine.
    /// SPEC-PM-017: signature mirrors the f64 trait surface — no shims in
    /// test plumbing. All three callers pass unsuffixed literals (verified).
    pub(crate) fn test_compute_visible_range(
        &mut self,
        vp_top: f64,
        vp_bottom: f64,
    ) -> (usize, usize, f64) {
        let sorted = self.row_topology.sorted_indices();
        let version = self.row_topology.topology_version();
        self.virtualization.compute_visible_range(
            vp_top,
            vp_bottom,
            self.body.row_model().heights(),
            sorted,
            version,
        )
    }

    /// Test accessor: set virtualization dirty flag (SPEC-HAL-258 placeholder).
    pub(crate) fn test_set_virtualization_dirty(&mut self, dirty: bool) {
        self.engine_invalidation.virtualization_dirty = dirty;
    }

    /// Test accessor: set selection dirty flag (SPEC-HAL-258 placeholder).
    pub(crate) fn test_set_selection_dirty(&mut self, dirty: bool) {
        self.engine_invalidation.selection_dirty = dirty;
    }

    /// Test accessor: check if a VisibleRangeToken is pending.
    pub(crate) fn test_has_visible_range_token(&self) -> bool {
        self.visible_range.is_some()
    }

    /// Test accessor: read virtualization_dirty flag.
    pub(crate) fn test_is_virtualization_dirty(&self) -> bool {
        self.engine_invalidation.virtualization_dirty
    }
}

impl WidgetDowncast for TabularSubstrate {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TabularSubstrate {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // SPEC-HAL-164: Begin new frame
        self.evaluation_guard.begin_evaluation();
        self.evaluation_guard.enter_layout();

        // Process any pending intents (including sort intents)
        self.process_pending_intents();

        // SPEC-HAL-171: Lock RowModel mutation when sort is active.
        let projection_active = self.sort_state.is_active() || self.sort_state.is_dirty();
        if projection_active {
            self.sort_guard.begin_sort_processing();
        }

        // SPEC-HAL-171/258: Compute sorted indices — engine owns ALL gating.
        self.ensure_topology();

        // SPEC-HAL-168: Resolve column geometry - single resolution point
        let available_width = constraints.max_width;
        self.resolve_geometry(available_width, 1.0);

        // SPEC-HAL-166: Project header cells from resolved geometry
        // SPEC-HAL-168: Use GeometryView for epoch-safe access
        let epoch = self.evaluation_guard.evaluation_epoch();
        self.column_guard.lock();
        self.header
            .project(GeometryView::new(&self.geometry_table, epoch));
        self.column_guard.unlock();

        // SPEC-HAL-167: Set body viewport and realize visible rows
        let header_height = self.header.height();
        let body_height = (constraints.max_height - header_height).max(0.0);
        let total_width = self.geometry_table.total_width(epoch);

        self.body
            .set_viewport(RectF::new(0.0, header_height, total_width, body_height));

        // SPEC-HAL-171: Realize with sorted order via VirtualizationEngine.
        self.evaluation_mode = EvaluationMode::LayoutHandled;
        drop(self.visible_range.take());
        self.row_guard.unlock();
        let viewport_top = self.body.scroll_y();
        let viewport_bottom = viewport_top + body_height;
        let sorted = self.row_topology.sorted_indices();
        let version = self.row_topology.topology_version();
        // SPEC-PM-017: f32 boundary shims — legacy tabular_view stays f32
        // internally until the rung-5 audit.
        let (first, last, y_offset) = self.virtualization.compute_visible_range(
            viewport_top as f64,
            viewport_bottom as f64,
            self.body.row_model().heights(),
            sorted,
            version,
        );
        let y_offset = y_offset as f32;
        let sorted = self.row_topology.sorted_indices();
        self.body.realize_with_range(
            GeometryView::new(&self.geometry_table, epoch),
            sorted,
            first,
            last,
            y_offset,
        );

        self.row_guard.lock();

        // SPEC-HAL-171: End sort processing - unlocks RowModel mutation.
        if projection_active {
            self.sort_guard.end_sort_processing();
        }
        // Safety net: assert guard is never stuck locked after layout completes
        debug_assert!(
            !self.sort_guard.is_sort_processing(),
            "SPEC-HAL-171: SortProcessingGuard stuck in locked state after layout"
        );

        // Calculate final size
        let width = total_width.min(constraints.max_width);
        let height = (header_height + self.body.content_height()).min(constraints.max_height);
        let size = Size::new(width, height);

        // Content size may exceed viewport (scrollable)
        let content_size = Size::new(total_width, header_height + self.body.content_height());

        LayoutResult::new(size, content_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // SPEC-HAL-164: Paint phase - geometry is read-only
        let epoch = self.evaluation_guard.evaluation_epoch();

        // SPEC-HAL-170: Paint column separators.
        // INV-170-1: Derived from Column Geometry Table only.
        // INV-170-2: No persistent state; computed fresh each frame.
        // INV-170-3: Hard guard — columns() panics if geometry unresolved.
        // INV-170-4: Exactly visible_columns - 1 separators.
        {
            let columns = self.geometry_table.columns(epoch);
            let total_height = self.header.height() + self.body.content_height();
            let active_resize = self.header.resizing_column();
            separators::emit_column_separators(
                columns,
                total_height,
                active_resize,
                |segment, color| {
                    cx.scene.push_rect(
                        segment.rect,
                        color,
                        crate::core::paint::LAYER_CONTENT,
                        None,
                    );
                },
            );
        }

        let _ = rect;
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // SPEC-HAL-164: Event phase - geometry is read-only
        self.evaluation_guard.enter_event();

        // Foundation pass: wire intent emission paths but don't implement resolution logic

        let _ = (cx, event, phase);
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "TabularSubstrate"
    }
}

// =============================================================================
// Re-exports for convenience
// =============================================================================

pub use crate::engines::column_geometry::ColumnGeometryEngine;
pub use body::{BodyDomain, BodyHitResult};
pub use cell_content::{CellContentProjection, project_from_cell};
pub use column::{Column, ColumnId, ColumnModel, ColumnScrollDomain, MIN_COL_WIDTH};
pub use geometry::{ColumnGeometryTable, ColumnSlot, GeometryView, ResolvedColumnGeometry};
pub use header::{HeaderCell, HeaderDomain, HeaderHitResult};
pub use intent_policy::{
    ActivationMode, DragPolicy, IntentContext, IntentInput, RangePolicy, SubcontrolBehavior,
    TabularIntentPolicy,
};
pub use intents::{
    ActivationIntent, EditIntent, EditPayload, FocusIntent, NavigationDirection, ResizeIntent,
    SelectionIntent, SortDirection, SortIntent, TabularIntent, TreeIntent, VisibilityIntent,
    WidthIntent,
};
pub use row::{CellId, CellProjection, RowId, RowModel, RowProjection, RowRealizationState};
pub use separators::{
    SEPARATOR_COLOR, SEPARATOR_HOVER_COLOR, SEPARATOR_WIDTH, SeparatorSegment,
    emit_column_separators,
};
pub use sort::{NullComparator, RowComparator, SortState};
pub use tree_paint::{TreeGuideKind, TreeGuideSegment, TreeRenderData};

pub use column_config::{ColumnConfigurationState, ColumnMetaSnapshot};
pub use view_config::{
    ViewConfigEnvelope, ViewConfigKey, ViewConfigSchema, ViewConfigStore, apply_raw_config_json,
    clear_sort_preference, compute_structure_hash, load_and_apply_column_config, normalize_json,
    reconcile_json, reset_column_config, save_column_config, save_sort_preference,
    sort_preference_from_raw_json,
};

// SPEC-HAL-188 Data Adapter exports
pub use data_adapter::{
    AdapterInvalidationResult, AppliedSortSnapshot, BaselineOrderId, CellCapabilities, CellValue,
    ColumnInvalidationScope, DataAdapter, DataInvalidationEvent, DataSource, Extent, OrderAccess,
    OrderRevision, OrderWindow, OrderWindowRequest, PolicyInvalidationScope, SortCapability,
    SortExactness, SortKey, SortKeyId, SortOperation, SortRequest, SortRequestId,
    SortRequestOutcome, SortSpec, SortTerm, UniverseMode, ValueState,
};

/// Tree indentation step in pixels per depth level.
///
/// SPEC-HAL-172 INVARIANT-TREE-3: This is a content offset only.
/// Column geometry is invariant.
pub const TREE_INDENT_STEP: f32 = 16.0;

/// Tree control zone width in pixels.
///
/// SPEC-HAL-172: The disclosure toggle affordance occupies this width
/// at the left side of the first column's indented cell rect.
pub const TREE_CONTROL_WIDTH: f32 = 16.0;

/// Tree guide line width in pixels.
///
/// SPEC-HAL-172: Width of vertical/horizontal guide line segments.
pub const TREE_GUIDE_LINE_WIDTH: f32 = 1.0;

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn setup_view() -> (TabularSubstrate, Vec<column::ColumnId>, Vec<row::RowId>) {
        let mut view = TabularSubstrate::new();
        view.add_column(Column::new(100.0));
        view.add_column(Column::new(100.0));
        let col_a = view.column_model().columns()[0].id;
        let col_b = view.column_model().columns()[1].id;

        let r0 = row::RowId::new();
        let r1 = row::RowId::new();
        let r2 = row::RowId::new();
        view.add_row(r0);
        view.add_row(r1);
        view.add_row(r2);

        (view, vec![col_a, col_b], vec![r0, r1, r2])
    }

    // =========================================================================
    // SPEC-HAL-174: Intent queue and destabilization cancel
    // =========================================================================

    #[test]
    fn test_queue_accepts_all_intents_during_edit() {
        let (mut view, cols, rows) = setup_view();

        // Begin edit
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        let editing_cell = view.edit_state().editing_cell();
        let focus = view.focus_state().focused();
        let sort_version = view.sort_state().version();
        let expected = [
            TabularIntent::Focus(intents::FocusIntent::RequestFocus {
                row_id: rows[2],
                column_id: cols[1],
            }),
            TabularIntent::Sort(intents::SortIntent::RequestSort {
                column_id: cols[0],
                direction: Some(intents::SortDirection::Ascending),
            }),
            TabularIntent::Tree(intents::TreeIntent::Toggle { row_index: 0 }),
            TabularIntent::Resize(intents::ResizeIntent::Begin {
                column_id: cols[0],
                initial_width: 100.0,
            }),
        ];
        for intent in &expected {
            view.emit_intent(intent.clone());
        }

        let pending = view.take_intents();
        assert_eq!(pending, expected);
        assert_eq!(view.edit_state().editing_cell(), editing_cell);
        assert_eq!(view.focus_state().focused(), focus);
        assert_eq!(view.sort_state().version(), sort_version);
        assert!(!view.sort_state().is_active());

        view.process_pending_intents();
        assert!(view.take_intents().is_empty());
        assert_eq!(view.edit_state().editing_cell(), editing_cell);
        assert_eq!(view.focus_state().focused(), focus);
        assert_eq!(view.sort_state().version(), sort_version);
    }

    #[test]
    fn test_queued_sort_notifies_once_and_executes_only_once() {
        let (mut view, cols, rows) = setup_view();
        view.handle_intent(&TabularIntent::Focus(intents::FocusIntent::RequestFocus {
            row_id: rows[1],
            column_id: cols[1],
        }));
        let pre_edit_focus = view.focus_state().focused();
        assert!(pre_edit_focus.is_some());
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        let notifications = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&notifications);
        view.on_intent = Some(Box::new(move |intent| {
            received.lock().unwrap().push(intent.clone());
        }));
        let sort = TabularIntent::Sort(intents::SortIntent::RequestSort {
            column_id: cols[0],
            direction: None,
        });
        let sort_version = view.sort_state().version();
        view.emit_intent(sort.clone());

        assert_eq!(view.pending_intents.as_slice(), std::slice::from_ref(&sort));
        assert_eq!(
            notifications.lock().unwrap().as_slice(),
            std::slice::from_ref(&sort)
        );
        assert!(view.is_editing());
        assert_eq!(view.sort_state().version(), sort_version);
        assert!(!view.sort_state().is_active());

        view.process_pending_intents();
        assert_eq!(*view.edit_state(), EditState::Idle);
        assert_eq!(view.focus_state().focused(), pre_edit_focus);
        assert_eq!(view.sort_state().active_column(), Some(cols[0]));
        assert_eq!(
            view.sort_state().direction(),
            Some(intents::SortDirection::Ascending)
        );
        assert_eq!(view.sort_state().version(), sort_version + 1);
        let pending_after_processing = view.pending_intents.len();

        // A replay would toggle the same column to Descending.
        view.process_pending_intents();
        assert_eq!(
            view.sort_state().direction(),
            Some(intents::SortDirection::Ascending)
        );
        assert_eq!(view.sort_state().version(), sort_version + 1);
        assert_eq!(pending_after_processing, 0);
        assert!(view.take_intents().is_empty());
        assert_eq!(
            notifications.lock().unwrap().as_slice(),
            std::slice::from_ref(&sort)
        );
    }

    #[test]
    fn test_pending_intents_process_in_fifo_order() {
        let (mut view, cols, _) = setup_view();
        let sort_version = view.sort_state().version();
        for column_id in [cols[0], cols[1], cols[1]] {
            view.emit_intent(TabularIntent::Sort(intents::SortIntent::RequestSort {
                column_id,
                direction: None,
            }));
        }

        view.process_pending_intents();
        assert_eq!(view.sort_state().active_column(), Some(cols[1]));
        assert_eq!(
            view.sort_state().direction(),
            Some(intents::SortDirection::Descending)
        );
        assert_eq!(view.sort_state().version(), sort_version + 3);
        assert!(view.take_intents().is_empty());
    }

    #[test]
    fn test_sort_during_edit_cancels_then_applies() {
        let (mut view, cols, rows) = setup_view();

        // Set pre-edit focus
        view.handle_intent(&TabularIntent::Focus(intents::FocusIntent::RequestFocus {
            row_id: rows[1],
            column_id: cols[1],
        }));
        let pre_focus = view.focus_state().focused();

        // Begin edit
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        // Sort intent → cancels edit, then applies sort
        view.handle_intent(&TabularIntent::Sort(intents::SortIntent::RequestSort {
            column_id: cols[0],
            direction: Some(intents::SortDirection::Ascending),
        }));

        assert!(!view.is_editing());
        assert!(view.sort_state().is_active());
        // Focus restored to pre-edit state (cancel semantics)
        assert_eq!(view.focus_state().focused(), pre_focus);
    }

    #[test]
    fn test_focus_during_edit_cancels_then_applies() {
        let (mut view, cols, rows) = setup_view();

        // Begin edit
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        // Focus intent → cancels edit, then applies new focus
        view.handle_intent(&TabularIntent::Focus(intents::FocusIntent::RequestFocus {
            row_id: rows[2],
            column_id: cols[1],
        }));

        assert!(!view.is_editing());
        // Focus is now the target of the focus intent (applied after cancel)
        let focused = view.focus_state().focused().unwrap();
        assert_eq!(focused.row_id, rows[2]);
        assert_eq!(focused.column_id, cols[1]);
    }

    #[test]
    fn test_resize_during_edit_does_not_cancel() {
        let (mut view, cols, rows) = setup_view();

        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        // Resize passes through without affecting edit
        view.handle_intent(&TabularIntent::Resize(intents::ResizeIntent::Begin {
            column_id: cols[0],
            initial_width: 100.0,
        }));

        assert!(view.is_editing());
    }

    #[test]
    fn test_scroll_during_edit_cancels() {
        let (mut view, cols, rows) = setup_view();

        // Set pre-edit focus
        view.handle_intent(&TabularIntent::Focus(intents::FocusIntent::RequestFocus {
            row_id: rows[1],
            column_id: cols[0],
        }));

        // Begin edit
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        // Scroll change → destabilization cancel
        view.set_scroll_y(50.0);

        assert!(!view.is_editing());
        // Focus restored to pre-edit (rows[1], cols[0])
        let focused = view.focus_state().focused().unwrap();
        assert_eq!(focused.row_id, rows[1]);
    }

    #[test]
    fn test_column_hide_during_edit_cancels() {
        let (mut view, cols, rows) = setup_view();

        // Begin edit on cols[0]
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        // Hide the editing column → destabilization cancel
        view.set_column_visible(cols[0], false);

        assert!(!view.is_editing());
    }

    #[test]
    fn test_column_hide_other_column_no_cancel() {
        let (mut view, cols, rows) = setup_view();

        // Begin edit on cols[0]
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(view.is_editing());

        // Hide a different column — no cancel
        view.set_column_visible(cols[1], false);

        assert!(view.is_editing());
    }

    #[test]
    fn test_edit_state_lifecycle() {
        let (mut view, cols, rows) = setup_view();

        // Initially Idle
        assert_eq!(*view.edit_state(), EditState::Idle);

        // Begin → Editing
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        assert!(matches!(*view.edit_state(), EditState::Editing(_)));
        assert_eq!(view.edit_state().editing_cell().unwrap().row_id, rows[0]);

        // Commit → Idle
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestCommitEdit {
                row_id: rows[0],
                column_id: cols[0],
                payload: intents::EditPayload::None,
            },
        ));
        assert_eq!(*view.edit_state(), EditState::Idle);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn test_nested_begin_edit_rejected() {
        let (mut view, cols, rows) = setup_view();

        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));

        // Nested begin — rejected, still editing original cell
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[1],
                column_id: cols[1],
            },
        ));

        assert_eq!(view.edit_state().editing_cell().unwrap().row_id, rows[0]);
        assert_eq!(view.edit_state().editing_cell().unwrap().column_id, cols[0]);
    }

    #[test]
    fn test_cancel_restores_pre_edit_focus() {
        let (mut view, cols, rows) = setup_view();

        // Set initial focus
        view.handle_intent(&TabularIntent::Focus(intents::FocusIntent::RequestFocus {
            row_id: rows[2],
            column_id: cols[1],
        }));

        // Begin edit on different cell
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));
        // Focus moved to editing cell
        assert_eq!(view.focus_state().focused().unwrap().row_id, rows[0]);

        // Cancel → restores pre-edit focus
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestCancelEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));

        let focused = view.focus_state().focused().unwrap();
        assert_eq!(focused.row_id, rows[2]);
        assert_eq!(focused.column_id, cols[1]);
    }

    #[test]
    fn test_commit_keeps_focus_on_edited_cell() {
        let (mut view, cols, rows) = setup_view();

        // Set initial focus
        view.handle_intent(&TabularIntent::Focus(intents::FocusIntent::RequestFocus {
            row_id: rows[2],
            column_id: cols[1],
        }));

        // Begin edit
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestBeginEdit {
                row_id: rows[0],
                column_id: cols[0],
            },
        ));

        // Commit → focus remains on edited cell
        view.handle_intent(&TabularIntent::Edit(
            intents::EditIntent::RequestCommitEdit {
                row_id: rows[0],
                column_id: cols[0],
                payload: intents::EditPayload::None,
            },
        ));

        let focused = view.focus_state().focused().unwrap();
        assert_eq!(focused.row_id, rows[0]);
        assert_eq!(focused.column_id, cols[0]);
    }

    // =========================================================================
    // Scroll + Virtualization Tests (regression for hit-testing after scroll)
    // =========================================================================

    #[test]
    fn test_update_visible_range_from_scroll_returns_changed() {
        // Setup: Create view with many rows
        let mut view = TabularSubstrate::new();
        view.add_column(Column::new(100.0));

        let row_ids: Vec<row::RowId> = (0..100).map(|_| row::RowId::new()).collect();
        for &id in &row_ids {
            view.add_row_with_height(id, 30.0); // 30px per row, 3000px total
        }

        // Simulate first layout to set viewport and sorted_indices
        // (In real usage, layout() does this)
        view.body.set_viewport(RectF::new(0.0, 0.0, 100.0, 300.0)); // 300px viewport = ~10 rows
        view.set_sorted_indices((0..100).collect());

        // First call should return true (initial state change)
        let changed1 = view.update_visible_range_from_scroll(0.0);
        // Note: Initial state is (0,0), scrolling to 0.0 computes a non-empty range
        // so this should return true
        assert!(
            changed1,
            "First scroll sync should detect change from initial state"
        );

        // Consume the token before next update (INV-VZ-34)
        view.visible_range.take();

        // Same scroll offset should return false
        let changed2 = view.update_visible_range_from_scroll(0.0);
        assert!(
            !changed2,
            "Same scroll offset should not change visible range"
        );

        // Consume the token before next update (INV-VZ-34)
        view.visible_range.take();

        // Different scroll offset should return true
        let changed3 = view.update_visible_range_from_scroll(300.0); // Scroll down 10 rows
        assert!(
            changed3,
            "Different scroll offset should change visible range"
        );
    }

    #[test]
    fn test_hit_test_after_scroll_targets_correct_row() {
        // Setup: Create view with 100 rows of 30px height
        let mut view = TabularSubstrate::new();
        view.add_column(Column::new(100.0));
        view.set_fixed_row_height(30.0);

        let row_ids: Vec<row::RowId> = (0..100).map(|_| row::RowId::new()).collect();
        for &id in &row_ids {
            view.add_row_with_height(id, 30.0);
        }

        // Simulate layout: viewport is 300px tall (fits ~10 rows)
        let header_height = 0.0;
        let viewport_height = 300.0;
        view.body
            .set_viewport(RectF::new(0.0, header_height, 100.0, viewport_height));
        view.set_sorted_indices((0..100).collect());
        view.test_begin_evaluation();
        view.test_resolve_geometry(100.0);

        // Scroll to 0, realize viewport via engine
        view.body.set_scroll_y(0.0);
        let sorted = view.row_topology.sorted_indices();
        let version = view.row_topology.topology_version();
        // SPEC-PM-017: f32 boundary shims — test mirrors the production
        // call shape; rung-5 audit.
        let (first, last, y_off) = view.virtualization.compute_visible_range(
            0.0,
            viewport_height as f64,
            view.body.row_model().heights(),
            sorted,
            version,
        );
        let y_off = y_off as f32;
        let sorted = view.row_topology.sorted_indices();
        view.body.realize_with_range(
            geometry::GeometryView::new(
                &view.geometry_table,
                view.evaluation_guard.evaluation_epoch(),
            ),
            sorted,
            first,
            last,
            y_off,
        );

        // Hit test at y=15 (middle of first row) should hit row 0
        // Note: hit_test expects TabularSubstrate-local coords, so y=15 means y=15 in body space
        let hit_at_scroll_0 = view.body.hit_test(50.0, 15.0);
        if let Some(cell_id) = hit_at_scroll_0 {
            assert_eq!(
                cell_id.row_id, row_ids[0],
                "At scroll=0, y=15 should hit row 0"
            );
        }

        // Now scroll down by 300px (10 rows worth)
        view.body.set_scroll_y(300.0);
        let sorted = view.row_topology.sorted_indices();
        let version = view.row_topology.topology_version();
        // SPEC-PM-017: f32 boundary shims — test mirrors the production
        // call shape; rung-5 audit.
        let (first2, last2, y_off2) = view.virtualization.compute_visible_range(
            300.0,
            (300.0 + viewport_height) as f64,
            view.body.row_model().heights(),
            sorted,
            version,
        );
        let y_off2 = y_off2 as f32;
        let sorted = view.row_topology.sorted_indices();
        view.body.realize_with_range(
            geometry::GeometryView::new(
                &view.geometry_table,
                view.evaluation_guard.evaluation_epoch(),
            ),
            sorted,
            first2,
            last2,
            y_off2,
        );

        // Hit test at y=15 (same screen position) should now hit row 10
        // Because scroll_y=300 means the viewport shows rows 10-19
        // Point y=15 in body space with scroll_y=300 maps to content y=315
        // Row 10 starts at y=300, so y=315 is in row 10
        let hit_at_scroll_300 = view.body.hit_test(50.0, 15.0);
        if let Some(cell_id) = hit_at_scroll_300 {
            assert_eq!(
                cell_id.row_id, row_ids[10],
                "At scroll=300, y=15 should hit row 10 (not row 0)"
            );
        }
    }

    #[test]
    fn test_hit_test_after_scroll_correct_row() {
        // Verify hit_test coordinate transformation with non-zero scroll.
        let mut view = TabularSubstrate::new();
        view.add_column(Column::new(100.0));

        let row_ids: Vec<row::RowId> = (0..50).map(|_| row::RowId::new()).collect();
        for &id in &row_ids {
            view.add_row(id);
        }
        let row_height = 24.0;

        let header_height = 30.0;
        let viewport_height = 240.0;
        view.body
            .set_viewport(RectF::new(0.0, header_height, 100.0, viewport_height));
        view.set_sorted_indices((0..50).collect());

        let scroll_y = 120.0;
        view.body.set_scroll_y(scroll_y);

        view.evaluation_guard.begin_evaluation();
        view.resolve_geometry(100.0, 1.0);
        let sorted = view.row_topology.sorted_indices();
        let version = view.row_topology.topology_version();
        // SPEC-PM-017: f32 boundary shims — test mirrors the production
        // call shape; rung-5 audit.
        let (first, last, y_off) = view.virtualization.compute_visible_range(
            scroll_y as f64,
            (scroll_y + viewport_height) as f64,
            view.body.row_model().heights(),
            sorted,
            version,
        );
        let y_off = y_off as f32;
        let sorted = view.row_topology.sorted_indices();
        view.body.realize_with_range(
            geometry::GeometryView::new(
                &view.geometry_table,
                view.evaluation_guard.evaluation_epoch(),
            ),
            sorted,
            first,
            last,
            y_off,
        );

        let test_x = 50.0;
        let test_y = header_height + 12.0;
        let expected_row_index = 5_usize;
        let expected_content_y = test_y - header_height + scroll_y;
        assert_eq!(
            (expected_content_y / row_height).floor() as usize,
            expected_row_index,
            "Sanity check: content y={} should map to row {}",
            expected_content_y,
            expected_row_index
        );

        let hit_test_result = view.body.hit_test(test_x, test_y);
        assert!(
            hit_test_result.is_some(),
            "hit_test() should find a row at y={} with scroll_y={}",
            test_y,
            scroll_y
        );

        let cell_id = hit_test_result.unwrap();
        assert_eq!(
            cell_id.row_id, row_ids[expected_row_index],
            "hit_test() at y={} with scroll_y={} should hit row {} (content y={})",
            test_y, scroll_y, expected_row_index, expected_content_y
        );
    }

    #[test]
    fn test_visible_range_computation_matches_list_pattern() {
        // This test verifies TabularSubstrate visible range follows the List pattern:
        // visible_range = pure function of (scroll_y, viewport_height, row_heights)
        // INV-VZ-6: Uses VirtualizationEngine as the single visible-range authority.

        let mut view = TabularSubstrate::new();
        view.add_column(Column::new(100.0));

        // Set fixed height to 20px BEFORE adding rows (RowModel defaults to 24px)
        view.body.row_model_mut().set_fixed_height(20.0);

        // 50 rows of 20px each = 1000px total content
        let row_ids: Vec<row::RowId> = (0..50).map(|_| row::RowId::new()).collect();
        for &id in &row_ids {
            view.add_row_with_height(id, 20.0);
        }

        // Viewport is 100px (fits 5 rows exactly)
        view.body.set_viewport(RectF::new(0.0, 0.0, 100.0, 100.0));
        view.set_sorted_indices((0..50).collect());

        // Test 1: scroll=0 → should see rows 0-4 (first 5 rows)
        let (first0, last0, _) = view.test_compute_visible_range(0.0, 100.0);
        assert_eq!(first0, 0, "scroll=0: first visible should be 0");
        assert!(last0 >= 5, "scroll=0: should see at least 5 rows");

        // Test 2: scroll=100 (5 rows down) → should see rows 5-9
        let (first100, _, _) = view.test_compute_visible_range(100.0, 200.0);
        assert_eq!(first100, 5, "scroll=100: first visible should be 5");

        // Test 3: scroll=900 (near bottom) → should see rows 45-49
        let (first900, _, _) = view.test_compute_visible_range(900.0, 1000.0);
        assert_eq!(first900, 45, "scroll=900: first visible should be 45");
    }

    // =========================================================================
    // SPEC-HAL-257: Virtualization stress test — multi-mutation single-frame integrity
    // =========================================================================

    #[test]
    fn test_virtualization_multi_mutation_single_frame_integrity() {
        // Setup: 200 rows, fixed 24px height, 2 columns, 240px viewport (~10 visible).
        let mut view = TabularSubstrate::new();
        view.add_column(Column::new(100.0));
        view.add_column(Column::new(100.0));

        let mut row_ids: Vec<row::RowId> = (0..200).map(|_| row::RowId::new()).collect();
        for &id in &row_ids {
            view.add_row(id);
        }

        // Set viewport: 240px tall body
        view.body.set_viewport(RectF::new(0.0, 0.0, 200.0, 240.0));
        view.set_sorted_indices((0..200).collect());

        // --- Phase 1: Initial frame at scroll_y=0 ---
        view.test_begin_evaluation();
        view.test_resolve_geometry(200.0);
        let changed = view.update_visible_range_from_scroll(0.0);
        assert!(changed, "Initial scroll sync should detect change");

        // Manually set topology clean + virtualization dirty so prepare_evaluation realizes
        view.test_set_topology_dirty(false);
        view.engine_invalidation.virtualization_dirty = true;
        view.prepare_evaluation(200.0, 1.0);

        // Assert: visible rows start at row 0
        let realized = view.body.realized_rows();
        assert!(!realized.is_empty(), "Must realize at least one row");
        assert_eq!(
            realized[0].row_id, row_ids[0],
            "Initial frame: first realized row should be row_ids[0]"
        );

        // --- Phase 2: Multi-mutation burst (NO intermediate layout) ---

        // 2a. Scroll jump to 480.0 (row ~20 at 24px/row)
        view.set_scroll_y(480.0);

        // 2b. Reverse sorted_indices (reorder)
        {
            let mut idx = view.sorted_indices().to_vec();
            idx.reverse();
            view.set_sorted_indices(idx);
        }

        // 2c. Remove 10 rows from the middle (indices 95..105 in original)
        let removed_ids: Vec<row::RowId> = row_ids[95..105].to_vec();
        // Remove from RowModel (highest index first to preserve indices)
        for i in (95..105).rev() {
            view.body.row_model_mut().remove(i);
        }
        view.bump_base_row_order_epoch();

        // Remove from row_ids tracking
        row_ids.drain(95..105);

        // 2d. Insert 5 new rows
        let new_ids: Vec<row::RowId> = (0..5).map(|_| row::RowId::new()).collect();
        for &id in &new_ids {
            view.add_row(id);
        }
        row_ids.extend_from_slice(&new_ids);

        // 2e. Rebuild sorted_indices for new row count (reversed)
        let row_count = view.body.row_model().len();
        assert_eq!(row_count, 195, "200 - 10 + 5 = 195 rows");
        view.set_sorted_indices((0..row_count).rev().collect());

        // --- Phase 3: Compute + consume through token path ---
        view.test_begin_evaluation();
        view.test_resolve_geometry(200.0);

        // Token must not exist yet (consumed by phase 1, no new production)
        assert!(
            !view.test_has_visible_range_token(),
            "Token must be None before update_visible_range_from_scroll"
        );

        let changed2 = view.update_visible_range_from_scroll(480.0);
        // Range changed because topology and scroll both mutated
        assert!(changed2, "Multi-mutation scroll sync should detect change");

        // Token must exist now
        assert!(
            view.test_has_visible_range_token(),
            "Token must be Some after update_visible_range_from_scroll"
        );

        // Realize
        view.test_set_topology_dirty(false);
        view.engine_invalidation.virtualization_dirty = true;
        view.prepare_evaluation(200.0, 1.0);

        // Token must be consumed (None after prepare_evaluation)
        assert!(
            !view.test_has_visible_range_token(),
            "Token must be consumed (None) after prepare_evaluation"
        );

        // --- Assertions ---
        let realized = view.body.realized_rows();

        // A) No panic (implicit — we got here)

        // B) Token consumed exactly once (asserted above: Some → None)

        // C) Realized rows within sorted_indices bounds, stable RowId mapping
        for rp in realized {
            let row_id = rp.row_id;
            assert!(
                row_ids.contains(&row_id),
                "Realized row {:?} not found in row_ids — mismatched identity",
                row_id
            );
            // Must not be one of the removed rows
            assert!(
                !removed_ids.contains(&row_id),
                "Realized row {:?} was removed — stale identity leak",
                row_id
            );
        }

        // D) First realized row corresponds to expected sorted order after reversal.
        // sorted_indices is reversed: [194, 193, 192, ...].
        // scroll_y=480, row_height=24 → first visible sorted index = 480/24 = 20.
        // sorted_indices[20] = 194 - 20 = 174 → row_ids[174].
        let expected_first_sorted_idx = (480.0_f32 / 24.0) as usize; // = 20
        let expected_row_model_idx = view.sorted_indices()[expected_first_sorted_idx];
        let expected_row_id = view.body.row_model().get(expected_row_model_idx).unwrap();
        assert_eq!(
            realized[0].row_id, expected_row_id,
            "First realized row should match expected sorted position after reversal"
        );

        // E) Hit test at middle of viewport returns consistent row
        // Viewport: scroll_y=480, height=240. Middle = 120px into viewport.
        // Middle screen y = 120 (relative to viewport origin).
        // Content y = 120 + 480 = 600. Row = 600/24 = 25 in sorted space.
        let hit = view.body.hit_test(100.0, 120.0);
        if let Some(cell_id) = hit {
            // Verify hit row is one of the realized rows
            assert!(
                realized.iter().any(|rp| rp.row_id == cell_id.row_id),
                "Hit test row must be among realized rows"
            );
            // Verify it's a valid row
            assert!(
                row_ids.contains(&cell_id.row_id),
                "Hit test row must exist in row_ids"
            );
        }

        // F) No duplicate realized rows
        let mut seen_ids = std::collections::HashSet::new();
        for rp in realized {
            assert!(
                seen_ids.insert(rp.row_id),
                "Duplicate realized row: {:?}",
                rp.row_id
            );
        }

        // G) Realized count ≤ visible capacity + 1
        let visible_capacity = (240.0_f32 / 24.0).ceil() as usize; // = 10
        assert!(
            realized.len() <= visible_capacity + 1,
            "Realized {} rows, expected at most {} (visible_capacity={} + 1 buffer)",
            realized.len(),
            visible_capacity + 1,
            visible_capacity
        );
    }
}
