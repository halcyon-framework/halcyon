// =============================================================================
// SPEC-HAL-169, 171, 174, 175: TabularSubstrate Intent Directives
// =============================================================================
//
// All interactions in TabularSubstrate are expressed as intent directives.
// No geometry mutation occurs inside interaction handlers.
// Upstream resolution decides actual mutations.
// =============================================================================

use super::column::ColumnId;
use super::row::RowId;
use serde::{Deserialize, Serialize};

// =============================================================================
// SPEC-HAL-169: Column Resize Intents
// =============================================================================

/// Column resize intent lifecycle phase.
///
/// SPEC-HAL-169: Resize is modeled as a three-phase intent lifecycle.
/// All phases emit intent directives only. No geometry mutation occurs
/// inside interaction handlers.
#[derive(Debug, Clone, PartialEq)]
pub enum ResizeIntent {
    /// Begin resize operation.
    ///
    /// SPEC-HAL-169 INVARIANT-2: Must reference exactly one column_id.
    Begin {
        column_id: ColumnId,
        /// Initial column width at drag start (snapshot).
        initial_width: f32,
    },

    /// Update resize with delta.
    ///
    /// SPEC-HAL-169 INVARIANT-4: Must not read or write column geometry.
    /// May only reference initial_width (snapshot) and pointer delta.
    Update {
        column_id: ColumnId,
        /// Proposed new width (initial_width + delta).
        proposed_width: f32,
    },

    /// Commit resize operation.
    ///
    /// SPEC-HAL-169: On commit, a structural mutation request is emitted,
    /// layout invalidation is triggered upstream.
    /// SPEC-HAL-169 INVARIANT-5: Interaction code emits proposed_width only.
    /// TabularSubstrate decides final clamped width.
    Commit {
        column_id: ColumnId,
        /// Proposed width — substrate applies clamping and two-column logic.
        proposed_width: f32,
    },

    /// Cancel resize operation.
    ///
    /// SPEC-HAL-169: On cancel, no mutation occurs.
    Cancel { column_id: ColumnId },
}

impl ResizeIntent {
    /// Get the column ID this intent targets.
    pub fn column_id(&self) -> ColumnId {
        match self {
            ResizeIntent::Begin { column_id, .. } => *column_id,
            ResizeIntent::Update { column_id, .. } => *column_id,
            ResizeIntent::Commit { column_id, .. } => *column_id,
            ResizeIntent::Cancel { column_id } => *column_id,
        }
    }
}

// =============================================================================
// SPEC-HAL-164 INVARIANT-A1: Visibility Intent
// =============================================================================

/// Column visibility intent directive.
///
/// SPEC-HAL-164 INVARIANT-A1: All semantic mutations must flow through handle_intent.
/// Visibility changes are no exception — they route through the intent pipeline
/// so that edit-session destabilization and guard checks are centralized.
#[derive(Debug, Clone, PartialEq)]
pub enum VisibilityIntent {
    /// Set column visibility.
    SetVisible { column_id: ColumnId, visible: bool },
}

// =============================================================================
// SPEC-HAL-164 INVARIANT-A1: Width Intent
// =============================================================================

/// Column width intent directive.
///
/// SPEC-HAL-164 INVARIANT-A1: Programmatic width changes route through handle_intent
/// so that guard checks are centralized.
#[derive(Debug, Clone, PartialEq)]
pub enum WidthIntent {
    /// Set column preferred width.
    SetWidth { column_id: ColumnId, width: f32 },
}

// =============================================================================
// SPEC-HAL-171: Sort Intents
// =============================================================================

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    /// Toggle to the opposite direction.
    pub fn toggle(self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }
}

/// Sort intent directive.
///
/// SPEC-HAL-171: Sort is expressed via intent directives only.
/// No mutation occurs in the Header Domain. Upstream resolution decides
/// final direction, multi-column behavior, data reordering, persistence.
#[derive(Debug, Clone, PartialEq)]
pub enum SortIntent {
    /// Request sort on a column.
    ///
    /// SPEC-HAL-171 INVARIANT-1: Must reference exactly one column_id.
    RequestSort {
        column_id: ColumnId,
        /// Explicit direction or None for toggle.
        direction: Option<SortDirection>,
    },

    /// Request to clear sort on a column.
    RequestClearSort { column_id: ColumnId },
}

impl SortIntent {
    /// Get the column ID this intent targets.
    pub fn column_id(&self) -> ColumnId {
        match self {
            SortIntent::RequestSort { column_id, .. } => *column_id,
            SortIntent::RequestClearSort { column_id } => *column_id,
        }
    }
}

// =============================================================================
// SPEC-HAL-174: Inline Edit Intents
// =============================================================================
//
// INTENDED EXCLUSIVE AUTHORITY RULES (not yet mechanically enforced):
//
// These rules describe the intended behavior when edit-session state is
// implemented. Currently they are design constraints, not runtime invariants.
//
// RULE-1: Inline editor should capture focus and pointer until commit/cancel.
//         No other cell should receive focus while editing is active.
//         Pointer events outside the editor should trigger cancel.
//
// RULE-2: Column resize during active edit should be forbidden or deferred.
//         When EditSession state exists, resize should be blocked until
//         editor teardown.
//
// RULE-3: Scroll during active edit should not invalidate the cell being
//         edited. When EditSession state exists, scroll should either
//         preserve visibility or cancel editing.
//
// These rules exist to prevent the "text field inside list" bug class.
// Enforcement requires EditSession state, which is not yet implemented.
// =============================================================================

/// Edit intent directive.
///
/// SPEC-HAL-174: All edit actions are expressed as intent directives.
/// TabularSubstrate does not own edit state. Editing is a mode, not a structure.
#[derive(Debug, Clone, PartialEq)]
pub enum EditIntent {
    /// Request to begin editing a cell.
    ///
    /// SPEC-HAL-174 INVARIANT-4: Must not mutate data, selection, geometry, or focus.
    RequestBeginEdit { row_id: RowId, column_id: ColumnId },

    /// Request to commit edit with payload.
    ///
    /// SPEC-HAL-174: TabularSubstrate does not interpret payloads.
    /// Upstream state decides validation, data mutation, persistence.
    RequestCommitEdit {
        row_id: RowId,
        column_id: ColumnId,
        /// Opaque payload - interpretation is upstream.
        payload: EditPayload,
    },

    /// Request to cancel edit.
    RequestCancelEdit { row_id: RowId, column_id: ColumnId },
}

impl EditIntent {
    /// Get the row ID this intent targets.
    pub fn row_id(&self) -> RowId {
        match self {
            EditIntent::RequestBeginEdit { row_id, .. } => *row_id,
            EditIntent::RequestCommitEdit { row_id, .. } => *row_id,
            EditIntent::RequestCancelEdit { row_id, .. } => *row_id,
        }
    }

    /// Get the column ID this intent targets.
    pub fn column_id(&self) -> ColumnId {
        match self {
            EditIntent::RequestBeginEdit { column_id, .. } => *column_id,
            EditIntent::RequestCommitEdit { column_id, .. } => *column_id,
            EditIntent::RequestCancelEdit { column_id, .. } => *column_id,
        }
    }
}

/// Edit payload - opaque value container.
///
/// SPEC-HAL-174: TabularSubstrate does not interpret payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum EditPayload {
    /// String value.
    String(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Bool(bool),
    /// No value (for custom handling).
    None,
}

impl Default for EditPayload {
    fn default() -> Self {
        EditPayload::None
    }
}

// =============================================================================
// SPEC-HAL-175: Focus and Navigation Intents
// =============================================================================

/// Navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDirection {
    /// Move left (previous column).
    Left,
    /// Move right (next column).
    Right,
    /// Move up (previous row).
    Up,
    /// Move down (next row).
    Down,
    /// Move to first cell in row.
    Home,
    /// Move to last cell in row.
    End,
    /// Move up by viewport height.
    PageUp,
    /// Move down by viewport height.
    PageDown,
}

/// Focus/navigation intent directive.
///
/// SPEC-HAL-175: Navigation is expressed via intent directives only.
/// Upstream logic decides wrapping behavior, column skipping rules,
/// tree-aware movement, interaction with selection policies.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusIntent {
    /// Request to focus a specific cell.
    RequestFocus { row_id: RowId, column_id: ColumnId },

    /// Request to navigate in a direction.
    ///
    /// SPEC-HAL-175: Emitted intents reference target cell coordinates, not widgets.
    RequestNavigate {
        /// Current focused cell (if any).
        from: Option<(RowId, ColumnId)>,
        /// Direction to navigate.
        direction: NavigationDirection,
    },

    /// Request to clear focus.
    RequestClearFocus,
}

// =============================================================================
// SPEC-HAL-172: Tree Intents
// =============================================================================

/// Tree intent directive.
///
/// SPEC-HAL-172: Tree expansion/collapse is expressed via intent directives only.
/// Hit-test paths must NOT directly mutate tree state.
/// Intent carries `row_index` (RowModel index), not RowId.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeIntent {
    /// Toggle expansion state of a node.
    Toggle { row_index: usize },
    /// Expand a node (make children visible).
    Expand { row_index: usize },
    /// Collapse a node (hide children).
    Collapse { row_index: usize },
}

impl TreeIntent {
    /// Get the row index this intent targets.
    pub fn row_index(&self) -> usize {
        match self {
            TreeIntent::Toggle { row_index } => *row_index,
            TreeIntent::Expand { row_index } => *row_index,
            TreeIntent::Collapse { row_index } => *row_index,
        }
    }
}

// =============================================================================
// SPEC-HAL-178: Selection Intents
// =============================================================================

/// Selection intent directive.
///
/// SPEC-HAL-178: Selection is expressed via intent directives only.
/// Selection operates on RowId identity (INV-178-1).
/// Selection never mutates FocusState (INV-178-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionIntent {
    /// Select a single row. Clears previous selection, sets anchor.
    SelectRow { row_id: RowId },
    /// Extend range from anchor to target row (Shift+click / Shift+Nav).
    /// If no anchor, sets anchor = row_id and selects that row.
    ExtendToRow { row_id: RowId },
    /// Toggle a single row in selection (Ctrl+click semantics).
    /// Adds if absent, removes if present. Sets anchor if none.
    ToggleRow { row_id: RowId },
    /// Clear all selection and anchor.
    ClearSelection,
    /// Set selection to an exact set of rows with specified anchor.
    ///
    /// Generic batch-set capability for widgets that need to compute
    /// selection externally (e.g., base ∪ range unions, multi-select patterns).
    ///
    /// Semantics:
    /// - Replaces `selected_rows` with `rows` (deterministic, order preserved).
    /// - Sets `anchor` to provided value (or None to clear anchor).
    /// - Updates `version` exactly once.
    /// - No List-specific or policy-specific behavior.
    SetSelection {
        rows: Vec<RowId>,
        anchor: Option<RowId>,
    },
}

// =============================================================================
// SPEC-HAL-200: Activation Intents
// =============================================================================

/// Activation intent directive.
///
/// SPEC-HAL-200: Activation is semantically distinct from selection.
/// Selection marks; activation opens, runs, or invokes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationIntent {
    /// Activate the target row (open, run, invoke).
    Activate { row_id: RowId },
}

// =============================================================================
// SPEC-HAL-313: Row Expansion Intents
// =============================================================================

/// Row expansion intent directive.
///
/// SPEC-HAL-313 §3.3: Row expansion is distinct from tree expansion (TreeIntent).
/// TreeIntent controls node child visibility in a hierarchy.
/// ExpansionIntent controls whether a detail widget subtree is shown for a row.
/// A TreeView node can be both tree-expanded and row-expanded independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpansionIntent {
    /// Expand the target row (show detail subtree).
    Expand { row_id: RowId },
    /// Collapse the target row (hide detail subtree).
    Collapse { row_id: RowId },
    /// Toggle expansion state of the target row.
    Toggle { row_id: RowId },
    /// Toggle expansion and enter focus capture into the detail subtree.
    /// Phase A stub: behaves as Toggle until SPEC-HAL-176 Phase 3 focus scopes land.
    ToggleWithCapture { row_id: RowId },
}

// =============================================================================
// SPEC-HAL-182: Column Reorder Intents
// =============================================================================

/// Column reorder intent directive.
///
/// SPEC-HAL-182: Reorder operations are expressed via intent directives.
/// Upstream resolution applies the new column order via ColumnModel::reorder().
#[derive(Debug, Clone, PartialEq)]
pub enum ReorderIntent {
    /// Apply a new column order.
    Apply { new_order: Vec<ColumnId> },
}

// =============================================================================
// Unified Intent Enum
// =============================================================================

/// All TabularSubstrate intent types.
///
/// This unified enum allows routing all intents through a single channel.
#[derive(Debug, Clone, PartialEq)]
pub enum TabularIntent {
    Resize(ResizeIntent),
    Sort(SortIntent),
    Edit(EditIntent),
    Focus(FocusIntent),
    Tree(TreeIntent),
    Selection(SelectionIntent),
    /// SPEC-HAL-200: Activation intent (open, run, invoke).
    Activation(ActivationIntent),
    /// SPEC-HAL-164 INVARIANT-A1: Column visibility mutation via intent pipeline.
    Visibility(VisibilityIntent),
    /// SPEC-HAL-164 INVARIANT-A1: Column width mutation via intent pipeline.
    Width(WidthIntent),
    /// SPEC-HAL-182: Column reorder via intent pipeline.
    Reorder(ReorderIntent),
    /// SPEC-HAL-313: Row expansion intent (detail subtree show/hide).
    Expansion(ExpansionIntent),
}

impl From<ResizeIntent> for TabularIntent {
    fn from(intent: ResizeIntent) -> Self {
        TabularIntent::Resize(intent)
    }
}

impl From<SortIntent> for TabularIntent {
    fn from(intent: SortIntent) -> Self {
        TabularIntent::Sort(intent)
    }
}

impl From<EditIntent> for TabularIntent {
    fn from(intent: EditIntent) -> Self {
        TabularIntent::Edit(intent)
    }
}

impl From<FocusIntent> for TabularIntent {
    fn from(intent: FocusIntent) -> Self {
        TabularIntent::Focus(intent)
    }
}

impl From<TreeIntent> for TabularIntent {
    fn from(intent: TreeIntent) -> Self {
        TabularIntent::Tree(intent)
    }
}

impl From<SelectionIntent> for TabularIntent {
    fn from(intent: SelectionIntent) -> Self {
        TabularIntent::Selection(intent)
    }
}

impl From<ActivationIntent> for TabularIntent {
    fn from(intent: ActivationIntent) -> Self {
        TabularIntent::Activation(intent)
    }
}

impl From<VisibilityIntent> for TabularIntent {
    fn from(intent: VisibilityIntent) -> Self {
        TabularIntent::Visibility(intent)
    }
}

impl From<WidthIntent> for TabularIntent {
    fn from(intent: WidthIntent) -> Self {
        TabularIntent::Width(intent)
    }
}

impl From<ReorderIntent> for TabularIntent {
    fn from(intent: ReorderIntent) -> Self {
        TabularIntent::Reorder(intent)
    }
}

impl From<ExpansionIntent> for TabularIntent {
    fn from(intent: ExpansionIntent) -> Self {
        TabularIntent::Expansion(intent)
    }
}
