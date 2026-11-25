// =============================================================================
// SPEC-HAL-260A §8, §12, §13: PropertyEditorCore + Intent Dispatch
// =============================================================================
//
// PropertyEditorCore owns the runtime state shared by PE Outer, Body, and
// Footer through `Rc<RefCell<PropertyEditorCore>>`. 22 fields per §8.1.
//
// dispatch_intent (§12.2) is the single funnel for mutating `selection`,
// `focused_column`, `edit_session`, `hts` topology, and `validation_error`.
// INV-PE-INTENT-FUNNEL: no public mutator routes around it.
//
// Sub-intent variants (ColumnIntent, EditIntent, TreeIntent) are minimal
// here — just enough to satisfy §13.2's accessor calls and the edit /
// column / tree cycle skeleton. They EXTEND in Phase B (Body event
// handlers) and Phase O (Outer keyboard handler) as call sites emit them.
// SPEC §12.1 does not enumerate variants; the accessors in §13.2 imply
// the minimum shape.
//
// `apply_*_intent` helpers are stubs in this task; bodies land with their
// call sites in B/O.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::core::ordered_selection::{
    OrderedIndex, OrderedSelectionAction, OrderedSelectionState, reduce_ordered_selection,
};
use crate::core::scroll_container::HorizontalScrollLink;
use crate::engines::cell_mechanics::{CellMechanicsSubstrate, PeFocus};
use crate::engines::cell_source::PropertyValue;
use crate::engines::hierarchical::NodeMeta;
use crate::utils::id::TreeWidgetId;
use crate::widgets::tabular_view::column::ColumnId;

use super::policy::PropertyEditorPolicy;
use crate::engines::cell_source::{PermissionAction, PermissionQuery, PermissionResult};

use super::types::{ColumnKind, PropertyDataSource, PropertyNode, PropertyNodeKind};
use crate::engines::cell_source::AdvanceDirection;

// ----------------------------------------------------------------------------
// §12.1 PEIntent + sub-intents
// ----------------------------------------------------------------------------

/// SPEC-HAL-260A §12.1: top-level intent. Every state-changing event
/// handler constructs one of these and calls
/// `PropertyEditorCore::dispatch_intent`.
pub enum PEIntent {
    Selection(OrderedSelectionAction<u64>),
    Column(ColumnIntent),
    Edit(EditIntent),
    Tree(TreeIntent),
    Activate { row_id: u64 },
    DataLayer(DataLayerIntent),
}

/// SPEC-HAL-260A §10.2 / §12: column-axis intents. Variant set is the
/// minimum needed for §13.2's `target_column()` accessor; Phase O may add
/// `Next`, `Previous`, `First`, `Last` (corresponding to the §10.2
/// `focus_column_*` ops) as keyboard handlers emit them.
pub enum ColumnIntent {
    To { column_id: ColumnId },
}

impl ColumnIntent {
    pub fn target_column(&self) -> ColumnId {
        match self {
            ColumnIntent::To { column_id } => *column_id,
        }
    }
}

/// SPEC-HAL-260A §6, §12: edit-cycle intents. Variant set is the minimum
/// needed for §13.2's `row_id()` and `column_id()` accessors plus the
/// edit-cycle phases (begin / commit / cancel / commit-and-advance).
pub enum EditIntent {
    Begin {
        row_id: u64,
        column_id: ColumnId,
    },
    Commit {
        row_id: u64,
        column_id: ColumnId,
        new_value: PropertyValue,
    },
    Cancel {
        row_id: u64,
        column_id: ColumnId,
    },
    CommitAndAdvance {
        row_id: u64,
        column_id: ColumnId,
        new_value: PropertyValue,
        direction: AdvanceDirection,
    },
}

impl EditIntent {
    pub fn row_id(&self) -> u64 {
        match self {
            EditIntent::Begin { row_id, .. }
            | EditIntent::Commit { row_id, .. }
            | EditIntent::Cancel { row_id, .. }
            | EditIntent::CommitAndAdvance { row_id, .. } => *row_id,
        }
    }

    pub fn column_id(&self) -> ColumnId {
        match self {
            EditIntent::Begin { column_id, .. }
            | EditIntent::Commit { column_id, .. }
            | EditIntent::Cancel { column_id, .. }
            | EditIntent::CommitAndAdvance { column_id, .. } => *column_id,
        }
    }
}

/// SPEC-HAL-260A §12: tree-structural intents. Variant set is the minimum
/// for §13.2's `row_index()` accessor; the disclosure-toggle call site in
/// Body (B4) emits `ToggleExpand`.
pub enum TreeIntent {
    ToggleExpand { row_index: usize },
}

impl TreeIntent {
    pub fn row_index(&self) -> usize {
        match self {
            TreeIntent::ToggleExpand { row_index } => *row_index,
        }
    }
}

/// SPEC-HAL-260A §12.1: reserved for future filter / sort / search /
/// grouping intents. No variants in this SPEC's scope.
pub enum DataLayerIntent {}

// ----------------------------------------------------------------------------
// §8.1 PropertyEditorCore
// ----------------------------------------------------------------------------

/// SPEC-HAL-260A §8.1: runtime state shared by Outer, Body, and Footer
/// through `Rc<RefCell<PropertyEditorCore>>` (§8.3). Field ownership is
/// declared in §8.2.
pub(crate) struct PropertyEditorCore {
    // --- Topology + geometry (HTS owned by CMS) ---
    pub(crate) cms: CellMechanicsSubstrate,
    pub(crate) column_kinds: HashMap<ColumnId, ColumnKind>,
    pub(crate) name_column_id: Option<ColumnId>,
    pub(crate) tree_column_id: Option<ColumnId>,
    pub(crate) value_column_ids: Vec<ColumnId>,
    pub(crate) column_titles: HashMap<ColumnId, String>,

    // --- Property data (consumer-provided) ---
    pub(crate) data_source: Option<Rc<RefCell<dyn PropertyDataSource>>>,
    pub(crate) property_nodes: HashMap<u64, PropertyNode>,
    pub(crate) data_revision: u64,

    // --- Row-axis selection / focus (reducer-managed) ---
    pub(crate) selection: OrderedSelectionState<u64>,

    // --- Drag selection (TreeView pattern) ---
    pub(crate) selection_drag_anchor: Option<u64>,
    pub(crate) selection_drag_active: bool,
    /// Last node during drag (for change detection).
    pub(crate) selection_drag_last: Option<u64>,

    // --- Column resize (outer-managed; TreeView pattern) ---
    pub(crate) hovered_separator: Option<ColumnId>,
    pub(crate) active_resize_separator: Option<ColumnId>,

    // --- Hover (paint affordance) ---
    /// Per-cell hover signal for Data rows. Set by PointerMove when the
    /// pointer is over a Data-row cell; cleared when the pointer leaves
    /// (on_hover_leave) or moves to a band row.
    pub(crate) hovered_cell: Option<(u64, ColumnId)>,
    /// Whole-row hover signal for band rows (Section / SubSection). Set
    /// by PointerMove when the pointer is over a band row anywhere
    /// across its width; cleared when the pointer leaves or moves to a
    /// Data row. Mutually exclusive with `hovered_cell` — the
    /// PointerMove seam guarantees at most one of the two is `Some`
    /// at a time.
    pub(crate) hovered_row: Option<u64>,

    // --- Body ↔ outer signaling ---
    pub(crate) body_widget_id: Option<TreeWidgetId>,
    pub(crate) requested_scroll_y: Option<f32>,
    pub(crate) focused: bool,
    pub(crate) scale_factor: f32,
    pub(crate) horizontal_scroll_link: Option<HorizontalScrollLink>,

    // --- Interaction policy ---
    pub(crate) policy: PropertyEditorPolicy,

    // --- Validation feedback (display cache only; §3.5 FORBID-DLS-1) ---
    pub(crate) validation_error: Option<String>,

    // --- Deferred topology (SPEC-HAL-337 §4.1 / INV-337-M1) ---
    /// Node ids whose expansion toggles were dispatched during event
    /// routing; applied by `PropertyEditor::update` before layout.
    /// Identity-resolved at dispatch time (frame-local `row_index` →
    /// `node_id`), immune to same-batch projection shifts.
    pub(crate) pending_topology_toggles: Vec<u64>,
}

// ----------------------------------------------------------------------------
// §12.2 dispatch_intent — the single funnel
// §13.2 build_permission_query — permission gate inside the funnel
// ----------------------------------------------------------------------------

impl PropertyEditorCore {
    /// SPEC-HAL-260A §12.2: single dispatch funnel. INV-PE-INTENT-FUNNEL.
    ///
    /// Returns `true` if the intent was applied, `false` if the permission
    /// gate denied it.
    pub(crate) fn dispatch_intent(&mut self, intent: PEIntent) -> bool {
        // §13.1: permission gate. Default `Permit`; consumer-provided
        // `permission_check` may deny. `matches!` instead of `==` because
        // PermissionResult does not derive PartialEq (SPEC §5.6) — semantics
        // are identical.
        if let Some(ds) = &self.data_source {
            let query = self.build_permission_query(&intent);
            if matches!(ds.borrow().permission_check(query), PermissionResult::Deny) {
                return false;
            }
        }

        match intent {
            PEIntent::Selection(action) => {
                // SPEC-HAL-260A §10.3: row-axis transitions may cross
                // archetype boundaries (Data ↔ Section ↔ SubSection).
                // After the reducer mutates `focus_key`, project the new
                // focus_key through `set_focus_row` — the single
                // constructor seam for row-axis focus mutation. It reads
                // the new row's archetype and produces the matching
                // PeFocus variant, applying §10.2's column-preservation
                // rules in the Data branch.
                //
                // Snapshot `prev_focus_key` BEFORE the reducer; skip
                // reprojection when the action mutated selection without
                // moving focus (ToggleFocused on already-focused row,
                // ExtendNext that only grows the range, reducer-rejected
                // actions). set_focus_row is idempotent, but avoiding the
                // call also avoids touching last_data_column on no-op
                // transitions.
                let prev_focus_key = self.selection.focus_key;
                let _ = reduce_ordered_selection(&mut self.selection, action, &*self.cms.hts());
                if self.selection.focus_key != prev_focus_key {
                    self.set_focus_row(self.selection.focus_key);
                }
            }
            PEIntent::Column(c) => self.apply_column_intent(c),
            PEIntent::Edit(e) => self.apply_edit_intent(e),
            PEIntent::Tree(t) => self.enqueue_tree_intent(t),
            PEIntent::Activate { row_id } => self.activate_row(row_id),
            PEIntent::DataLayer(d) => self.forward_to_data_layer(d),
        }
        true
    }

    /// SPEC-HAL-260A §13.2: construct the permission query for a given
    /// intent. Used inside `dispatch_intent` above.
    fn build_permission_query(&self, intent: &PEIntent) -> PermissionQuery {
        match intent {
            PEIntent::Selection(action) => PermissionQuery {
                row_id: extract_row_from_action(action).unwrap_or(0),
                column_index: None,
                action: match action {
                    OrderedSelectionAction::DragSelect { .. } => PermissionAction::DragExtend,
                    _ => PermissionAction::Select,
                },
            },
            PEIntent::Column(c) => PermissionQuery {
                row_id: self.selection.focus_key.unwrap_or(0),
                column_index: Some(self.column_index_of(c.target_column())),
                action: PermissionAction::Focus,
            },
            PEIntent::Edit(e) => PermissionQuery {
                row_id: e.row_id(),
                column_index: Some(self.column_index_of(e.column_id())),
                action: PermissionAction::Edit,
            },
            PEIntent::Tree(t) => PermissionQuery {
                row_id: self.node_id_for_index(t.row_index()),
                column_index: None,
                action: PermissionAction::Tree,
            },
            PEIntent::Activate { row_id } => PermissionQuery {
                row_id: *row_id,
                column_index: None,
                action: PermissionAction::Activate,
            },
            PEIntent::DataLayer(_) => PermissionQuery {
                // Future port supplies query shape per variant.
                row_id: 0,
                column_index: None,
                action: PermissionAction::Select,
            },
        }
    }

    // ------------------------------------------------------------------------
    // Helpers — stubs in T4; full bodies land with their call sites.
    // ------------------------------------------------------------------------

    /// Map ColumnId → display column index (position in `value_column_ids`).
    /// Used by `build_permission_query` for the gate's `column_index` and by
    /// Body to encode `cell_widget_key` (SPEC §7.3). Callers must pass Data
    /// column IDs; NonData / unregistered IDs return `usize::MAX` as a "no
    /// valid index" sentinel — gate consumers should treat that as
    /// "not cell-specific" and degrade gracefully.
    fn column_index_of(&self, column_id: ColumnId) -> usize {
        self.value_column_ids
            .iter()
            .position(|&id| id == column_id)
            .unwrap_or(usize::MAX)
    }

    /// Map a sorted-enumeration row index → node_id for the gate's
    /// `PEIntent::Tree` query (SPEC-HAL-260A §13.2). Uses
    /// `OrderedIndex::key_at` — the same resolver `enqueue_tree_intent`
    /// uses. Out-of-bounds (stale caller-supplied index) returns the
    /// `0` sentinel rather than panicking, matching the
    /// `extract_row_from_action(...).unwrap_or(0)` and
    /// `selection.focus_key.unwrap_or(0)` conventions already used in
    /// `build_permission_query` for unresolvable row identities.
    /// Consumer `permission_check` implementations are expected to
    /// deny or no-op on row_id=0 if they enforce a non-trivial access
    /// policy; the matched `enqueue_tree_intent` call additionally
    /// bound-checks and silently no-ops on the same stale index.
    fn node_id_for_index(&self, row_index: usize) -> u64 {
        if row_index < self.cms.hts().len() {
            self.cms.hts().key_at(row_index)
        } else {
            0
        }
    }

    /// SPEC-HAL-260A §5.5.1: predicate `true` when the row is a band
    /// archetype (Section or SubSection). Used by paint, hit-test,
    /// navigation, and selection gates that need to distinguish bands
    /// from Data rows before T5/T6/T7 fork their paths fully on the
    /// archetype.
    ///
    /// Returns `false` for unknown row ids (data not yet loaded, stale
    /// id from a prior revision). Treated as a Data row in that case;
    /// callers using `navigable_columns_for_row` will see the full
    /// column list, which is the safe default.
    pub(crate) fn is_section_row(&self, row_id: u64) -> bool {
        self.property_nodes.get(&row_id).map_or(false, |n| {
            matches!(
                n.kind,
                PropertyNodeKind::Section | PropertyNodeKind::SubSection
            )
        })
    }

    /// SPEC-HAL-260A §5.5: look up the archetype of a row by id. Returns
    /// `None` for unknown row ids (data not yet loaded, stale id from a
    /// prior revision). Used by `set_focus_row` to branch on the new
    /// row's archetype.
    pub(crate) fn property_node_kind(&self, row_id: u64) -> Option<PropertyNodeKind> {
        self.property_nodes.get(&row_id).map(|n| n.kind)
    }

    /// SPEC-HAL-260A §10.2: predicate `true` when the column is a Data
    /// column (carries property values). Used by `set_focus_row` and
    /// `apply_column_intent` to decide whether to update the
    /// `last_data_column` cache on a column transition.
    pub(crate) fn is_data_column(&self, column_id: ColumnId) -> bool {
        self.column_kinds.get(&column_id) == Some(&ColumnKind::Data)
    }

    /// SPEC-HAL-260A §10.2 / §14.3: predicate used by PE Outer's
    /// `hit_test_separator` to suppress column-resize hits when the
    /// pointer is over a section row. Replaces the prior
    /// `visible_section_y_bands` paint-published cache — the predicate
    /// derives on demand from `hts` + `is_section_row`, so paint and
    /// hit-test cannot disagree by construction.
    ///
    /// `body_content_y` is the pointer Y in body-content space:
    ///   body_content_y = local_y - header_height + hts.scroll_offset()
    /// where `local_y` is PE-Outer-local and `header_height` is the
    /// outer's header strip. Returns `false` when `row_height` is zero,
    /// the body Y is negative (pointer in header strip), or the
    /// computed row index falls outside the sorted enumeration.
    pub(crate) fn is_section_at_body_y(&self, body_content_y: f32) -> bool {
        let row_height = self.cms.hts().row_height();
        if row_height <= 0.0 || body_content_y < 0.0 {
            return false;
        }
        let row_index = (body_content_y / row_height) as usize;
        if row_index >= self.cms.hts().len() {
            return false;
        }
        let node_id = self.cms.hts().key_at(row_index);
        self.is_section_row(node_id)
    }

    /// SPEC-HAL-260A §10.2: ordered list of columns navigable by the
    /// cursor on the given row.
    ///
    /// - Section rows: only the tree column. FORBID-PE-SECTION-DATA-CURSOR
    ///   (Step 1 §12) forbids Data-column cursor placement on Section
    ///   rows, so cell-mode navigation must never propose one.
    /// - Data rows: every registered column in HTS display order
    ///   (`column_layout().header_slots()`: pinned-left → scrollable →
    ///   pinned-right). Matches the slot order O3b's `next_column`
    ///   walks for Left / Right arrow navigation.
    ///
    /// Allocates `Vec<ColumnId>` per call. Acceptable at keyboard rate.
    /// SPEC §10.2 sketches the signature as `&[ColumnId]` over a
    /// cached `all_columns_in_order` field; that cache is a future
    /// optimization, not part of this task.
    pub(crate) fn navigable_columns_for_row(&self, row_id: u64) -> Vec<ColumnId> {
        // SPEC-HAL-260A §10.2: band-archetype rows (Section / SubSection)
        // have an EMPTY navigable set — they do not host cell focus. The
        // band is hit-tested and focused as a whole-row unit via
        // `PeFocus::Band` (§10.1); per-column cell-mode operations do not
        // apply on it. Tab navigation (property_editor.rs:1005) treats
        // empty nav_cols as "no movement"; Left/Right arrow emitters
        // (next_column at property_editor.rs:128) consult this list and
        // produce `None` on band rows, so no column intent is emitted.
        //
        // Data rows return every registered column in HTS display order
        // (pinned-left → scrollable → pinned-right).
        if self.is_section_row(row_id) {
            return Vec::new();
        }
        // Bind the GeometryView to a local; otherwise the temporary
        // returned by `column_layout()` drops at end-of-expression while
        // `header_slots()`'s iterator still borrows it (E0716).
        let hts = self.cms.hts();
        let layout = hts.column_layout();
        layout.header_slots().map(|s| s.column_id).collect()
    }

    /// Apply a column-axis intent. SPEC-HAL-260A §10.2 / §10.3:
    /// `focused_column` updates unconditionally for any intent target;
    /// `last_data_column` tracks the most recently focused Data column.
    /// NonData column focus leaves `last_data_column` untouched —
    /// preserves the "fall back to last data column when row changes"
    /// semantics §10.3 specifies.
    ///
    /// O3b emits `ColumnIntent::To` for Left/Right arrow keys; future
    /// phases (column reordering, programmatic focus) emit the same
    /// variant.
    fn apply_column_intent(&mut self, intent: ColumnIntent) {
        match intent {
            ColumnIntent::To { column_id } => {
                // SPEC-HAL-260A §10.2. Column intents only act on cell
                // focus. On `PeFocus::Band` and `PeFocus::None` there is
                // no column to set — silent no-op. INV-CELL-FOCUS-WELLFORMED
                // guard rejects non-navigable targets (emitters may walk
                // raw slot order and propose columns outside the row's
                // navigable set; the funnel is the final enforcer).
                let PeFocus::Cell { row_id, .. } = self.cms.focus() else {
                    return;
                };
                if !self.navigable_columns_for_row(row_id).contains(&column_id) {
                    return;
                }
                self.cms.set_focus(PeFocus::Cell {
                    row_id,
                    col_id: column_id,
                });
                if self.is_data_column(column_id) {
                    self.cms.set_last_data_column(Some(column_id));
                }
            }
        }
    }

    /// SPEC-HAL-260A §10.1: project a `focus_key` onto `core.focus`. The
    /// single constructor seam for row-axis focus mutation. Reads the
    /// target row's archetype and produces the matching `PeFocus`
    /// variant. Idempotent and total — handles `None`, unknown rows,
    /// archetype lookup, column resolution, and §10.2's
    /// column-preservation rules in one place.
    ///
    /// - `None` input → `PeFocus::None`.
    /// - Unknown row → `PeFocus::None`.
    /// - Section / SubSection → `PeFocus::Band { row_id }`. Before
    ///   transitioning, caches the prior `Cell.col_id` into
    ///   `last_data_column` when that column is a Data column.
    /// - Data → `PeFocus::Cell { row_id, col_id }` with `col_id`
    ///   resolved by `resolve_cell_focus_column`.
    ///
    /// Called from `dispatch_intent`'s Selection branch after the
    /// reducer mutates `selection.focus_key`; called from
    /// programmatic row-focus mutators when those land. Direct mutation
    /// of `self.focus` outside this seam and `apply_column_intent` is
    /// a funnel bypass (INV-PE-INTENT-FUNNEL).
    fn set_focus_row(&mut self, new_focus_key: Option<u64>) {
        let prior = self.cms.focus();
        let next = match new_focus_key {
            None => PeFocus::None,
            Some(row_id) => match self.property_node_kind(row_id) {
                None => PeFocus::None,
                Some(PropertyNodeKind::Section) | Some(PropertyNodeKind::SubSection) => {
                    // Cache out the prior Data column before transitioning
                    // into a band (§10.2's column-preservation rule).
                    if let PeFocus::Cell { col_id, .. } = prior {
                        if self.is_data_column(col_id) {
                            self.cms.set_last_data_column(Some(col_id));
                        }
                    }
                    PeFocus::Band { row_id }
                }
                Some(PropertyNodeKind::Data) => {
                    match self.resolve_cell_focus_column(row_id, prior) {
                        Some(col_id) => PeFocus::Cell { row_id, col_id },
                        None => PeFocus::None,
                    }
                }
            },
        };
        self.cms.set_focus(next);
    }

    /// SPEC-HAL-260A §10.2: choose the column to use when entering a
    /// Data row. Resolution order, first match wins:
    ///   1. Prior `Cell.col_id` if still navigable on the new row.
    ///   2. `last_data_column` if navigable on the new row.
    ///   3. First Data column (`value_column_ids.first()`).
    ///   4. Tree column (`tree_column_id`).
    /// Returns `None` only when the column set is entirely empty (no
    /// data columns and no tree column). In that case `set_focus_row`
    /// collapses to `PeFocus::None` — INV-CELL-FOCUS-WELLFORMED holds
    /// vacuously.
    fn resolve_cell_focus_column(&self, row_id: u64, prior: PeFocus) -> Option<ColumnId> {
        let navigable = self.navigable_columns_for_row(row_id);
        if let PeFocus::Cell { col_id, .. } = prior {
            if navigable.contains(&col_id) {
                return Some(col_id);
            }
        }
        if let Some(cc) = self.cms.last_data_column() {
            if navigable.contains(&cc) {
                return Some(cc);
            }
        }
        self.value_column_ids
            .first()
            .copied()
            .or(self.tree_column_id)
    }

    /// Apply an edit-cycle intent. B5 fills the Commit case (drives the
    /// always-mounted Bool Checkbox commit path and any other Commit
    /// emitter). Begin / Cancel / CommitAndAdvance stay stubbed until
    /// their call sites land (B6 / O3); INV-PE-SINGLE-EDIT will be
    /// enforced once Begin lands.
    fn apply_edit_intent(&mut self, intent: EditIntent) {
        match intent {
            EditIntent::Commit {
                row_id,
                column_id,
                new_value,
            } => {
                if let Some(ds) = &self.data_source {
                    let mut ds_mut = ds.borrow_mut();
                    let accepted = ds_mut.commit_cell(row_id, column_id, new_value.clone());
                    if accepted {
                        // Clear validation error and the edit session if
                        // this commit ended one. Bool's always-mounted
                        // Checkbox does not open a session, so the check
                        // is a no-op for Bool but active for inline-editor
                        // commits.
                        self.validation_error = None;
                        if self.cms.edit_session().map(|s| (s.row_id, s.column_id))
                            == Some((row_id, column_id))
                        {
                            self.cms.set_edit_session(None);
                        }
                    } else {
                        // Commit rejected per SPEC §5.3 — capture the
                        // `validate` error message.
                        self.validation_error =
                            ds_mut.validate(row_id, column_id, &new_value).err();
                    }
                }
            }
            // Edit entry routes through the shared substrate mechanism.
            // `CellMechanicsSubstrate::try_enter_edit` enforces the
            // on-demand-editor eligibility rule (editable; not Bool /
            // ReadOnly / Badge) and INV-PE-SINGLE-EDIT (reject when a
            // session is already active), then creates the session
            // (child_widget_id / editor_node_id as None; the reconcile pass
            // mounts the editor next layout). The permission gate has already
            // run in `dispatch_intent` before this arm. The cell is fetched
            // from the data source for the eligibility check; `validation_error`
            // clears only on a successful entry.
            EditIntent::Begin { row_id, column_id } => {
                let cell = {
                    let ds = self.data_source.clone();
                    ds.as_ref()
                        .and_then(|ds| ds.borrow().cell(row_id, column_id))
                };
                if let Some(cell) = cell {
                    if self
                        .cms
                        .try_enter_edit(row_id, column_id, &cell.value, cell.editable)
                    {
                        self.validation_error = None;
                    }
                }
            }
            // Clear the session only if it matches the cell. Stale Cancel
            // intents (already-ended or different-cell session) are
            // silent no-ops.
            EditIntent::Cancel { row_id, column_id } => {
                if self.cms.edit_session().map(|s| (s.row_id, s.column_id))
                    == Some((row_id, column_id))
                {
                    self.cms.set_edit_session(None);
                    self.validation_error = None;
                }
            }
            // CommitAndAdvance → keyboard advance handler (step 7).
            EditIntent::CommitAndAdvance { .. } => {}
        }
    }

    /// Enqueue a tree-structural intent (SPEC-HAL-260A §12.2,
    /// SPEC-HAL-337 §4.1 / INV-337-M1). The permission gate has already
    /// run for the inbound `PEIntent::Tree` before this helper is
    /// invoked. Internal callers (e.g. `activate_row`'s NonData branch)
    /// are inside an outer gated `dispatch_intent` and call this helper
    /// directly — that is consistent with the `apply_edit_intent`
    /// pattern (SPEC §13.2 row-level activation gate) and is not a
    /// funnel bypass.
    ///
    /// Application is deferred: `PropertyEditor::update` (the per-frame
    /// hook) drains `pending_topology_toggles` before layout. Identity
    /// is resolved HERE, while the frame-local `row_index` is still
    /// valid against the current projection — the queue carries node
    /// ids, never indices.
    fn enqueue_tree_intent(&mut self, intent: TreeIntent) {
        match intent {
            TreeIntent::ToggleExpand { row_index } => {
                // Bound-check before `key_at`: the trait docstring at
                // `ordered_selection/mod.rs:77-78` mandates panic on
                // out-of-bounds. A stale row_index — caller computed
                // it before another intent shrank the sorted set —
                // becomes a silent no-op rather than a panic.
                if row_index < self.cms.hts().len() {
                    let node_id = self.cms.hts().key_at(row_index);
                    self.pending_topology_toggles.push(node_id);
                }
            }
        }
    }

    /// Activation behavior on the focused row. SPEC-HAL-260A §10.5 +
    /// Step 1 §7.4: column-kind-aware dispatch.
    ///
    /// - `focused_column == None` → no-op.
    /// - NonData column on a row with children → `hts.toggle_by_id`
    ///   (B4 / B7c META bypass of the intent funnel; same shape as
    ///   the pointer-side disclosure / double-click path).
    /// - NonData column on a leaf row → no-op.
    /// - Data column, cell `Bool(b)` + editable → emit
    ///   `EditIntent::Commit { row, col, Bool(!b) }` via
    ///   `apply_edit_intent`. Matches the always-mounted Checkbox's
    ///   `on_change` commit path (SPEC §6.2).
    /// - Data column, cell editable, non-Bool, non-ReadOnly → emit
    ///   `EditIntent::Begin` via `apply_edit_intent` (INV-PE-SINGLE-EDIT
    ///   enforced inside that helper at core.rs:378–389).
    /// - Otherwise (cell missing, ReadOnly, not editable, unregistered
    ///   column) → no-op.
    ///
    /// The Activate permission gate has already run at the top of
    /// `dispatch_intent` (core.rs:229–234, query at :276–280). The
    /// internal call to `apply_edit_intent` is not a new intent
    /// emission and does not re-run the gate; this is consistent
    /// with the row-level shape of `PEIntent::Activate` (no
    /// `column_index` in the gate query — SPEC §13.2).
    fn activate_row(&mut self, row_id: u64) {
        let Some(column_id) = self.cms.focus().col_id() else {
            return;
        };

        let column_kind = self.column_kinds.get(&column_id).copied();
        match column_kind {
            Some(ColumnKind::NonData) => {
                // Resolve the visible row index for the focused row
                // (HTS-sorted enumeration). Used both for the
                // has_children check and as the `enqueue_tree_intent`
                // argument. Borrow of `topology()` is scoped — released
                // before the `&mut self` call to `enqueue_tree_intent`.
                let row_index_opt = self.cms.hts().index_of(row_id);
                let has_children = row_index_opt
                    .map(|idx| self.cms.hts().topology().has_children(idx))
                    .unwrap_or(false);
                if !has_children {
                    return;
                }
                if let Some(row_index) = row_index_opt {
                    // Already inside a gated `dispatch_intent`
                    // (Activate). The internal call to
                    // `enqueue_tree_intent` is a sub-step of that gated
                    // intent, not a new emission — matches the
                    // `apply_edit_intent` pattern used elsewhere in
                    // this method.
                    self.enqueue_tree_intent(TreeIntent::ToggleExpand { row_index });
                }
            }
            Some(ColumnKind::Data) => {
                let cell = {
                    let ds = self.data_source.clone();
                    ds.as_ref()
                        .and_then(|ds| ds.borrow().cell(row_id, column_id))
                };
                let Some(cell) = cell else {
                    return;
                };
                if !cell.editable {
                    return;
                }
                match cell.value {
                    PropertyValue::Bool(b) => {
                        self.apply_edit_intent(EditIntent::Commit {
                            row_id,
                            column_id,
                            new_value: PropertyValue::Bool(!b),
                        });
                    }
                    PropertyValue::ReadOnly(_) => {
                        // Structurally non-editable.
                    }
                    _ => {
                        self.apply_edit_intent(EditIntent::Begin { row_id, column_id });
                    }
                }
            }
            None => {
                // Unregistered column.
            }
        }
    }

    /// Forward a data-layer intent to the consumer. Future feature port.
    fn forward_to_data_layer(&mut self, _intent: DataLayerIntent) {
        // Empty enum has no variants; matched here for completeness.
    }

    /// Rebuild HTS tree topology from the registered data source.
    ///
    /// Reads `data_source.nodes()` and `data_source.revision()`, converts
    /// the `PropertyNode` list to `NodeMeta` topology, pushes the topology
    /// to HTS via `set_tree_data`, repopulates `property_nodes`, and
    /// caches the new `data_revision`.
    ///
    /// Idempotent w.r.t. revision: calling this when the source has not
    /// advanced its `revision()` still rebuilds verbatim. Callers that
    /// want revision-gated rebuild check `data_revision` against
    /// `source.revision()` themselves and skip this call when equal.
    ///
    /// No-op when `data_source` is `None`.
    ///
    /// SPEC-HAL-260A §8.2: writer attribution for `property_nodes` and
    /// `data_revision`. SPEC-HAL-260A §3.4 INV-DLS-1: PE re-reads the
    /// data source; PE does not mutate the data source.
    ///
    /// SPEC-HAL-259 INV-HTS-PREORDER: the consumer's `nodes()` must
    /// return nodes in strict pre-order traversal. `build_node_metas`
    /// preserves the ordering; `hts.set_tree_data` debug-asserts pre-order
    /// on receipt.
    pub(crate) fn rebuild_hts_data(&mut self) {
        let Some(source) = self.data_source.clone() else {
            return;
        };
        let (nodes, revision) = {
            let ds = source.borrow();
            (ds.nodes(), ds.revision())
        };

        let node_metas = build_node_metas(&nodes);
        self.cms.hts_mut().set_tree_data(node_metas);

        self.property_nodes.clear();
        self.property_nodes.reserve(nodes.len());
        for node in nodes {
            self.property_nodes.insert(node.node_id, node);
        }

        self.data_revision = revision;
    }
}

// ----------------------------------------------------------------------------
// Free helpers
// ----------------------------------------------------------------------------

/// Convert a `PropertyNode` list (in consumer-supplied pre-order) to a
/// `NodeMeta` list suitable for `HierarchicalTabularSubstrate::set_tree_data`.
///
/// Builds the parent-index map from `parent_id` lookups, computes per-row
/// depth from parent depth, and computes `has_children` by scanning which
/// `parent_id` values are referenced. Preserves input ordering.
///
/// SPEC-HAL-259 INV-HTS-PREORDER: the input must be in pre-order. This
/// function does not reorder. If the consumer violates pre-order, HTS's
/// `set_tree_data` debug-asserts on receipt.
fn build_node_metas(props: &[PropertyNode]) -> Vec<NodeMeta> {
    // Collect the set of parent_ids referenced — gives has_children
    // for each node in O(N) after the scan.
    let referenced_parents: HashSet<u64> = props.iter().filter_map(|p| p.parent_id).collect();

    // Map node_id -> index in the input vec. Lets parent_index resolve
    // in O(1) per node.
    let id_to_index: HashMap<u64, usize> = props
        .iter()
        .enumerate()
        .map(|(i, p)| (p.node_id, i))
        .collect();

    // SPEC-HAL-260A §5.5.1 INV-ARCHETYPE-HIERARCHY: every SubSection's
    // parent_id must resolve to a Section node. The combined invariant
    // (no SubSection at root, no SubSection under Data, no SubSection
    // nested under another SubSection) reduces to: SubSection's parent
    // exists, resolves, and has `kind == Section`. Debug-only — release
    // builds render whatever topology the consumer supplies, per §3
    // Data-Layer Sovereignty (PE does not silently restructure data).
    #[cfg(debug_assertions)]
    for prop in props {
        if prop.kind == PropertyNodeKind::SubSection {
            let parent_id = prop.parent_id.unwrap_or_else(|| {
                panic!(
                    "INV-ARCHETYPE-HIERARCHY: SubSection node {} has no parent (would be at root)",
                    prop.node_id
                )
            });
            let parent_index = id_to_index.get(&parent_id).copied().unwrap_or_else(|| {
                panic!(
                    "INV-ARCHETYPE-HIERARCHY: SubSection node {} parent_id {} does not resolve in the nodes() vector",
                    prop.node_id, parent_id
                )
            });
            assert_eq!(
                props[parent_index].kind,
                PropertyNodeKind::Section,
                "INV-ARCHETYPE-HIERARCHY: SubSection node {} parent (node {}) has kind {:?}, expected Section",
                prop.node_id,
                parent_id,
                props[parent_index].kind,
            );
        }
    }

    let mut metas: Vec<NodeMeta> = Vec::with_capacity(props.len());
    for prop in props {
        let parent_index = prop
            .parent_id
            .and_then(|pid| id_to_index.get(&pid).copied());
        let depth = parent_index.map_or(0, |pi| metas[pi].depth + 1);
        let has_children = referenced_parents.contains(&prop.node_id);

        metas.push(NodeMeta {
            node_id: prop.node_id,
            parent_index,
            depth,
            has_children,
            label: prop.label.clone(),
        });
    }

    metas
}

/// SPEC-HAL-260A §13.2: extract the row key from a selection action, if the
/// variant carries one. Returns `None` for actions that operate on the
/// current focus (FocusPrevious, FocusNext, etc.); caller passes
/// `.unwrap_or(0)` per the SPEC.
fn extract_row_from_action(action: &OrderedSelectionAction<u64>) -> Option<u64> {
    use OrderedSelectionAction::*;
    match action {
        Select { key } | Toggle { key } | ExtendTo { key } | FocusTo { key } => Some(*key),
        DragSelect { current, .. } => Some(*current),
        // Focus*, Extend*, MoveFocus*, ToggleFocused, ClearSelection,
        // SelectAll, DragEnd, FocusPage, ExtendPage, MoveFocusPage —
        // no row key carried in the action; the gate sees row 0.
        _ => None,
    }
}
