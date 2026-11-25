// =============================================================================
// SPEC-HAL-260A §4.1 / §8.2: PropertyEditor (outer composite)
// =============================================================================
//
// Outer widget for PE v2. Owns `PropertyEditorCore` via `Rc<RefCell<...>>`
// (§8.3), composes the body inside a ScrollArea (typically wrapped by the
// consumer or a future I-phase helper), and (in subsequent O-phase tasks)
// will paint the header, route keyboard events, and handle column resize.
// Mirrors `TreeView`'s structural pattern.
//
// O1 scope: skeleton — constructor with default Name + Value columns,
// `create_body_widget`, minimal Widget impl that resolves column geometry
// once at composite level and positions a single body child below a
// reserved header strip. Header paint, keyboard handling, column resize,
// and any filter/footer come in O2/O3/O4.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::event::{CursorIcon, Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::ordered_selection::{
    OrderedIndex, OrderedSelectionAction, OrderedSelectionState, SelectionMode,
};
use crate::core::scene::LAYER_CONTENT;
use crate::core::scroll_container::HorizontalScrollLink;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::engines::cell_mechanics::CellMechanicsSubstrate;
use crate::engines::cell_source::CellSource;
use crate::engines::hierarchical::{HierarchicalTabularSubstrate, HtsOwnership, SharedHts};
use crate::model::tabular::column::{Column, ColumnScrollDomain};
use crate::utils::id::WidgetInternalId;
use crate::widgets::cell_editor_factory::DefaultCellEditorFactory;
use crate::widgets::tabular_view::column::ColumnId;

use super::body::PropertyEditorBody;
use super::core::{ColumnIntent, PEIntent, PropertyEditorCore};
use super::footer::PropertyEditorFooter;
use super::policy::{PropertyEditorPolicy, TabBoundary, TabRowEnd};
use super::types::{ColumnKind, PropertyDataSource};

/// Default row height for PE rows (pixels). Matches the old PE baseline.
const DEFAULT_ROW_HEIGHT: f32 = 24.0;

/// Reserved height for the header strip (pixels). O2 paints the header
/// background, column titles, and column separators into this region.
const DEFAULT_HEADER_HEIGHT: f32 = 24.0;

/// Hit-test slop for column separator detection (pixels, each side).
/// Mirrors `tree_view::SEPARATOR_HIT_HALF_WIDTH` at tree_view.rs:127.
const SEPARATOR_HIT_HALF_WIDTH: f32 = 3.0;

/// Column resize state machine. Mirrors TreeView's `ResizeState`
/// (tree_view.rs:133–145). States: `Idle → Armed → Dragging → Idle`.
///
/// - `Armed`: PointerDown landed on a separator. `start_x` and
///   `initial_width` are captured; no drag has started yet.
/// - `Dragging`: first PointerMove after Arm transitions here, the
///   pointer is captured by PE Outer, and subsequent Moves stream
///   width updates to HTS until PointerUp.
///
/// SPEC §10.5 forbids PE Escape handling; consequently there is no
/// `cancel_resize` path that rolls back to `initial_width` — the
/// `initial_width` field is kept for shape parity with TreeView but is
/// unread today. (If a future SPEC amendment ever permits PE-side
/// Escape, the rollback restores trivially.)
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResizeState {
    Idle,
    Armed {
        column_id: ColumnId,
        start_x: f32,
        initial_width: f32,
    },
    Dragging {
        column_id: ColumnId,
        start_x: f32,
        initial_width: f32,
    },
}

impl Default for ResizeState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Compute scroll target to reveal the currently focused row.
///
/// Sets `core.requested_scroll_y` if the focused row is outside the
/// viewport. No-op if no row is focused, row height is zero, or the
/// focused node is no longer in the sorted enumeration. Direct port of
/// TreeView's `request_scroll_to_focus` at `tree_view.rs:51-72`.
fn request_scroll_to_focus(core: &mut PropertyEditorCore) {
    let Some(focus_id) = core.selection.focus_key else {
        return;
    };
    let rh = core.cms.hts().row_height();
    if rh <= 0.0 {
        return;
    }
    let Some(sorted_index) = core.cms.hts().index_of(focus_id) else {
        return;
    };
    let row_top = sorted_index as f32 * rh;
    let row_bottom = row_top + rh;
    // SPEC-PM-017: f32 shim — PE rung.
    let scroll_y = core.cms.hts().scroll_offset() as f32;
    let viewport_h = core.cms.hts().viewport_height();

    if row_top < scroll_y {
        core.requested_scroll_y = Some(row_top);
    } else if row_bottom > scroll_y + viewport_h {
        core.requested_scroll_y = Some(row_bottom - viewport_h);
    }
}

/// O3b helper. Compute the next/previous column from `focused_column`
/// in slot order. Direction is `+1` (Right / next) or `-1` (Left /
/// previous). Returns `None` when there is no target — empty slot
/// list, or already at the edge in the requested direction (no wrap
/// per SPEC §10.2).
///
/// First press with `focused_column = None`: Right starts at the
/// first slot, Left starts at the last slot. Slot order is the
/// visual order from HTS column layout (pinned-left, then
/// scrollable, then pinned-right).
fn next_column(core: &PropertyEditorCore, direction: i32) -> Option<ColumnId> {
    let slots: Vec<_> = core.cms.hts().column_layout().header_slots().collect();
    if slots.is_empty() {
        return None;
    }
    let current_idx = core
        .cms
        .focus()
        .col_id()
        .and_then(|c| slots.iter().position(|s| s.column_id == c));
    let target_idx = match (current_idx, direction) {
        (None, d) if d > 0 => Some(0),
        (None, _) => Some(slots.len() - 1),
        (Some(i), d) if d > 0 && i + 1 < slots.len() => Some(i + 1),
        (Some(i), d) if d < 0 && i > 0 => Some(i - 1),
        _ => None,
    };
    target_idx.map(|i| slots[i].column_id)
}

/// SPEC-HAL-260A §4.1: outer composite widget. Owns the shared
/// `PropertyEditorCore` and composes the body below a reserved header
/// strip.
///
/// Mirrors `TreeView`'s structural pattern: the outer holds the core,
/// `PropertyEditorBody` is a separate widget added as a child (typically
/// inside a `ScrollArea` between this PE and the body, set up by the
/// consumer).
pub struct PropertyEditor {
    id: WidgetInternalId,
    core: Rc<RefCell<PropertyEditorCore>>,
    /// SPEC-HAL-337 §2: who constructed the topology this widget projects.
    ownership: HtsOwnership,
    /// Reserved height for the header strip. O2 paints into this region.
    header_height: f32,
    /// Cached body height from the last layout pass. Used by row-axis
    /// keyboard handling (PageUp/PageDown page size).
    body_height: f32,
    /// O4: column resize state machine. `Idle` between drags; `Armed`
    /// from PointerDown on a separator until first PointerMove;
    /// `Dragging` while the pointer is captured streaming width
    /// updates. See `ResizeState` doc above.
    resize_state: ResizeState,
    /// I1: reserved height for the optional footer strip. `0.0` (the
    /// default) means no footer; the second child slot is ignored by
    /// layout. Set via `set_footer_height` before `create_footer_widget`.
    footer_height: f32,
    /// I1: guard against double-creation of the footer child. Mirrors
    /// `tree_view::TreeView::footer_taken`.
    footer_taken: bool,
}

impl PropertyEditor {
    /// Create a new, empty PropertyEditor (no columns, no data source).
    ///
    /// PE is configured via the builders per SPEC-HAL-260A §7.2:
    /// - `add_name_column(title)` — register the Name / tree column
    ///   (`ColumnKind::NonData`). Required exactly once before PE is
    ///   mounted for meaningful rendering. Calling it more than once is
    ///   a programming error (debug panic).
    /// - `add_value_column(title)` — register zero or more value columns
    ///   (`ColumnKind::Data`).
    /// - `set_data_source(source)` — register a `PropertyDataSource`.
    /// - `with_policy(policy)` — install a non-default interaction policy
    ///   (consuming chained builder).
    ///
    /// Body's layout / paint degrade to empty rendering for the tree-column
    /// slot when `name_column_id` is `None` (consumer hasn't called
    /// `add_name_column` yet); no panic.
    pub fn new() -> Self {
        let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
        hts.borrow_mut().set_row_height(DEFAULT_ROW_HEIGHT);
        Self::with_handle(hts, HtsOwnership::Internal)
    }

    /// Create a PropertyEditor projecting an injected shared topology
    /// substrate (SPEC-HAL-337 §2: `HtsOwnership::External`). Exactly one
    /// substrate instance exists per topology (INV-337-O1): the given
    /// handle is threaded into the CMS unchanged. The injected substrate's
    /// configuration (row height, columns, data) is not modified — only
    /// the CMS attaches and PE's editor factory is installed.
    pub fn with_hts(hts: SharedHts) -> Self {
        Self::with_handle(hts, HtsOwnership::External)
    }

    /// Shared constructor body — one code shape, no dual paths
    /// (SPEC-HAL-337 §2).
    fn with_handle(hts: SharedHts, ownership: HtsOwnership) -> Self {
        let mut cms = CellMechanicsSubstrate::new(hts);
        cms.set_editor_factory(Box::new(DefaultCellEditorFactory::new()));

        Self {
            id: WidgetInternalId::new(),
            core: Rc::new(RefCell::new(PropertyEditorCore {
                // --- Topology + geometry ---
                cms,
                column_kinds: HashMap::new(),
                name_column_id: None,
                tree_column_id: None,
                value_column_ids: Vec::new(),
                column_titles: HashMap::new(),

                // --- Property data ---
                data_source: None,
                property_nodes: HashMap::new(),
                data_revision: 0,

                // --- Row-axis selection / focus ---
                selection: OrderedSelectionState::new(SelectionMode::Multi),

                // --- Drag selection ---
                selection_drag_anchor: None,
                selection_drag_active: false,
                selection_drag_last: None,

                // --- Column resize ---
                hovered_separator: None,
                active_resize_separator: None,

                // --- Hover (paint affordance) ---
                hovered_cell: None,
                hovered_row: None,

                // --- Body ↔ outer signaling ---
                body_widget_id: None,
                requested_scroll_y: None,
                focused: false,
                scale_factor: 1.0,
                horizontal_scroll_link: Some(HorizontalScrollLink::new()),

                // --- Interaction policy ---
                policy: PropertyEditorPolicy::default(),

                // --- Validation feedback ---
                validation_error: None,

                // --- Deferred topology (SPEC-HAL-337 §4.1) ---
                pending_topology_toggles: Vec::new(),
            })),
            ownership,
            header_height: DEFAULT_HEADER_HEIGHT,
            body_height: 0.0,
            resize_state: ResizeState::Idle,
            footer_height: 0.0,
            footer_taken: false,
        }
    }

    // =========================================================================
    // O1: Builder API (SPEC-HAL-260A §7.2 / §8.2)
    // =========================================================================

    /// Install a `PropertyEditorPolicy` at construction time (consuming
    /// builder).
    ///
    /// Chains with `PropertyEditor::new()`:
    /// ```ignore
    /// let pe = PropertyEditor::new().with_policy(my_policy);
    /// ```
    ///
    /// Policy fields cover tab traversal, cursor behavior, wrap policy,
    /// edit triggers, blur disposition, and other configuration knobs
    /// per SPEC-HAL-260A §11. Default policy is installed by `new()`;
    /// this builder replaces it.
    ///
    /// SPEC-HAL-260A §8.2 ownership: `policy` is written by Outer (builder
    /// API). This is the builder entry point.
    pub fn with_policy(self, policy: PropertyEditorPolicy) -> Self {
        self.core.borrow_mut().policy = policy;
        self
    }

    /// Clone of the shared topology handle (SPEC-HAL-337 §2).
    pub fn hts_handle(&self) -> SharedHts {
        self.core.borrow().cms.hts_handle()
    }

    /// Who constructed the topology this widget projects (SPEC-HAL-337 §2).
    pub fn ownership(&self) -> HtsOwnership {
        self.ownership
    }

    /// SPEC-HAL-337 §4.1: widget-level pass-through into the single gated
    /// funnel (SPEC-HAL-260A §12.2). The permission gate runs inside the
    /// core funnel — this adds no second entry path (INV-PE-INTENT-FUNNEL).
    /// Exists for tests and programmatic intent dispatch.
    #[cfg(test)]
    pub(crate) fn dispatch_intent(&mut self, intent: PEIntent) -> bool {
        self.core.borrow_mut().dispatch_intent(intent)
    }

    /// Register the Name column (also the tree column).
    ///
    /// Constructs a `ColumnKind::NonData` column with PE-default geometry
    /// (preferred 200px, min 100px) and forwards to
    /// `HierarchicalTabularSubstrate::add_column` (SPEC-HAL-165 INV-1 /
    /// INV-3). Sets both `name_column_id` and `tree_column_id` to
    /// `Some(id)`; registers PE-side metadata (`column_kinds`,
    /// `column_titles`).
    ///
    /// Returns the allocated `ColumnId`.
    ///
    /// SPEC-HAL-260A §7.2: calling `add_name_column` more than once is a
    /// programming error (debug panic). The Name column is the tree
    /// column; PE supports exactly one tree column per §7.1.
    pub fn add_name_column(&mut self, title: impl Into<String>) -> ColumnId {
        let mut core = self.core.borrow_mut();
        debug_assert!(
            core.name_column_id.is_none(),
            "add_name_column called more than once (SPEC-HAL-260A §7.2)"
        );
        let column = Column::new(200.0).with_min_width(100.0);
        let column_id = core.cms.hts_mut().add_column(column);
        core.column_kinds.insert(column_id, ColumnKind::NonData);
        core.column_titles.insert(column_id, title.into());
        core.name_column_id = Some(column_id);
        core.tree_column_id = Some(column_id);
        column_id
    }

    /// Append a value column to PE.
    ///
    /// Appends a `ColumnKind::Data` column. The **first** value column
    /// registered gets PE-default elastic geometry (preferred 200px, min
    /// 80px, flex 1.0); **subsequent** value columns get fixed geometry
    /// (preferred 200px, min 80px). This honors SPEC-HAL-180
    /// INV-COL-ELASTIC-1 (at most one elastic column per layout, scoped
    /// to scrollable). The first-elastic convention matches Unity
    /// Inspector / VS Properties / typical desktop property panels: the
    /// primary value column fills remaining horizontal space, auxiliary
    /// value columns are fixed-width.
    ///
    /// Forwards to the substrate
    /// `HierarchicalTabularSubstrate::add_column` mutator (SPEC-HAL-165
    /// INV-1 / INV-3) which owns the column model. Updates PE-side
    /// per-column metadata (`column_kinds`, `column_titles`,
    /// `value_column_ids`) per SPEC-HAL-260A §8.1 / §8.2.
    ///
    /// Returns the `ColumnId` allocated for the new column. Consumers
    /// store this to address cells in the column via
    /// `CellSource::cell(row_id, column_id)` and to apply per-cell
    /// editability via `Cell.editable`.
    ///
    /// Multiple calls append additional columns; columns appear in call
    /// order after the default Name + default Value pair created by
    /// `new()`.
    ///
    /// SPEC-HAL-260A §7.2: builder signature
    /// `add_value_column(&mut self, title: impl Into<String>) -> ColumnId`.
    pub fn add_value_column(&mut self, title: impl Into<String>) -> ColumnId {
        let mut core = self.core.borrow_mut();

        // SPEC-HAL-180 INV-COL-ELASTIC-1: at most one elastic column per
        // layout. The first value column registered becomes the elastic
        // one (matches the conventional PE shape: primary value column
        // fills remaining horizontal space, auxiliary value columns are
        // fixed-width). Subsequent value columns get fixed geometry.
        let is_first_value_column = core.value_column_ids.is_empty();
        let mut column = Column::new(200.0).with_min_width(80.0);
        if is_first_value_column {
            column = column.with_flex(1.0);
        }

        let column_id = core.cms.hts_mut().add_column(column);
        core.column_kinds.insert(column_id, ColumnKind::Data);
        core.column_titles.insert(column_id, title.into());
        core.value_column_ids.push(column_id);
        column_id
    }

    /// Override the default elastic-column convention, marking
    /// `column_id` as the layout's elastic column.
    ///
    /// `add_value_column` registers the first value column with
    /// `flex = 1.0` and subsequent columns with no flex, per
    /// SPEC-HAL-260A §7.2's default convention (primary value
    /// column absorbs remaining width). For compositions where a
    /// non-primary column should absorb the leftover instead — e.g.
    /// a wide free-text Notes column following narrow Value /
    /// Default columns — call this method after registration with
    /// the desired column's ID.
    ///
    /// SPEC-HAL-180 INV-COL-ELASTIC-1 (at most one elastic column
    /// per layout) is preserved: this method clears flex on every
    /// PE-registered column (name column + every value column),
    /// then sets `flex = 1.0` on the target. The end state has
    /// exactly one elastic column — the target.
    ///
    /// `column_id` must be a column previously registered via
    /// `add_name_column` or `add_value_column`. Returns `true` on
    /// success, `false` if the ID is not registered to this PE.
    pub fn set_column_elastic(&mut self, column_id: ColumnId) -> bool {
        let core = self.core.borrow_mut();

        let known = core.name_column_id == Some(column_id)
            || core.value_column_ids.iter().any(|&id| id == column_id);
        if !known {
            return false;
        }

        // Collect every registered column ID, then clear flex on each.
        // Vec to avoid aliasing the borrow on `core` across `hts.set_column_flex`.
        let all_ids: Vec<ColumnId> = core
            .name_column_id
            .into_iter()
            .chain(core.value_column_ids.iter().copied())
            .collect();
        for id in &all_ids {
            core.cms.hts_mut().set_column_flex(*id, None);
        }
        core.cms.hts_mut().set_column_flex(column_id, Some(1.0));

        true
    }

    /// Register a `PropertyDataSource` and rebuild the HTS topology
    /// from it.
    ///
    /// Stores the data source in `core.data_source`, then calls
    /// `rebuild_hts_data` to convert `data_source.nodes()` into
    /// `NodeMeta` topology and push to HTS via `hts.set_tree_data`.
    /// Updates `core.property_nodes` (the keyed `PropertyNode` map
    /// per SPEC-HAL-260A §8.1) and caches `core.data_revision`.
    ///
    /// Calling `set_data_source` while an edit session is active is a
    /// programming pattern that may leave the edit session referencing
    /// stale node / column IDs. PE does not clear `edit_session` here;
    /// the reducer and mounting code (§6) observe stale IDs as transient
    /// per INV-DLS-1's read-back-after-revision-bump pattern. Consumers
    /// who swap data sources at runtime should commit or cancel any
    /// active edit first.
    ///
    /// SPEC-HAL-260A §8.2 ownership: `data_source` written by Outer
    /// (`set_data_source`); `property_nodes` and `data_revision` written
    /// by Outer (`rebuild_hts_data` after `revision()` change). This is
    /// the initial-load path; runtime revision-change detection in PE
    /// Outer's layout pass is a separate concern (next task).
    pub fn set_data_source(&mut self, source: Rc<RefCell<dyn PropertyDataSource>>) {
        let mut core = self.core.borrow_mut();
        // Trait upcast `dyn PropertyDataSource` → `dyn CellSource`. Same
        // underlying RefCell; CMS reads cell content via the universal
        // trait. PE keeps the PropertyDataSource handle for `nodes()`.
        let cell_source: Rc<RefCell<dyn CellSource>> = source.clone();
        core.cms.set_cell_source(Some(cell_source));
        core.data_source = Some(source);
        core.rebuild_hts_data();
    }

    // =========================================================================
    // O4: column resize internals (SPEC-HAL-260A §8.2 / §11.2)
    // =========================================================================

    /// Hit-test header separators against a viewport-local X coordinate.
    /// Returns the column whose right-edge separator is within
    /// `SEPARATOR_HIT_HALF_WIDTH` of `local_x`. Skips the rightmost
    /// column (no separator after it). Direct port of
    /// `TreeView::hit_test_separator` (tree_view.rs:352–377).
    ///
    /// Uses `try_column_layout()` (non-panicking) because geometry may
    /// not be resolved during event dispatch — same reason TreeView's
    /// event-path code does (line 358).
    ///
    /// Scrollable-domain slots are tested in content-local space
    /// (`local_x + scroll_x`) to keep hit-bands aligned with their
    /// painted positions after horizontal scroll.
    fn hit_test_separator(&self, local_x: f32, local_y: f32) -> Option<ColumnId> {
        let core = self.core.borrow();

        // SPEC §10.2 / §14.3 symmetry: hit-test skips separators over
        // section rows so column-resize agrees with paint geometry.
        // Header strip (local_y < header_height) is always allowed —
        // sections live in the body only. The predicate derives on
        // demand from `hts` + `is_section_row` via
        // `core.is_section_at_body_y`; no stored intermediate, no
        // paint/hit-test synchronization to drift.
        if local_y >= self.header_height {
            // SPEC-PM-017: f32 shim — PE rung.
            let scroll_y = core.cms.hts().scroll_offset() as f32;
            let body_content_y = local_y - self.header_height + scroll_y;
            if core.is_section_at_body_y(body_content_y) {
                return None;
            }
        }

        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let slots: Vec<_> = core.cms.hts().try_column_layout()?.header_slots().collect();
        drop(core);

        if slots.is_empty() {
            return None;
        }

        for slot in &slots[..slots.len().saturating_sub(1)] {
            let sep_x = slot.x + slot.width - 1.0;
            let test_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                local_x + scroll_x
            } else {
                local_x
            };
            if (test_x - sep_x).abs() <= SEPARATOR_HIT_HALF_WIDTH {
                return Some(slot.column_id);
            }
        }
        None
    }

    /// Resolve a column's current width via the HTS geometry view.
    /// Returns None if geometry is unresolved this evaluation (e.g.
    /// pre-first-layout) or the column id is not registered. Mirrors
    /// `TreeView::get_column_width` (tree_view.rs:408–414).
    fn get_column_width(&self, column_id: ColumnId) -> Option<f32> {
        let core = self.core.borrow();
        core.cms
            .hts()
            .try_column_layout()?
            .get(column_id)
            .map(|s| s.width)
    }

    /// Apply a user-initiated column resize via HTS.
    ///
    /// Routes through `HierarchicalTabularSubstrate::commit_user_resize`, the
    /// substrate authority for user-initiated resize gestures. The substrate
    /// applies SPEC-HAL-165 / SPEC-HAL-169 INVARIANT-5 (clamp, INV-RESIZE-
    /// CLEARS-ELASTIC, set preferred_width); PE proposes the width only.
    ///
    /// `set_column_width` (the raw programmatic setter) is NOT called here —
    /// that path is for programmatic configuration that must not override
    /// elastic behavior.
    fn apply_column_width(&mut self, column_id: ColumnId, width: f32) {
        self.core
            .borrow_mut()
            .cms
            .hts_mut()
            .commit_user_resize(column_id, width);
    }

    /// Commit current resize state: width sticks, all transient
    /// affordance flags clear. Called from PointerUp (both Armed and
    /// Dragging paths). Mirrors `TreeView::finalize_resize`
    /// (tree_view.rs:425–430).
    fn finalize_resize(&mut self) {
        self.resize_state = ResizeState::Idle;
        let mut core = self.core.borrow_mut();
        core.active_resize_separator = None;
        core.hovered_separator = None;
    }

    /// I1: set the footer height. `0.0` (default) disables the footer
    /// band; layout will not reserve space for it and any second child
    /// is ignored. Mirrors `TreeView::set_footer_height`.
    pub fn set_footer_height(&mut self, h: f32) {
        self.footer_height = h;
    }

    /// I1: create the footer widget (`PropertyEditorFooter`).
    ///
    /// Call this before adding PE to the widget tree. Add the returned
    /// footer as the second child of `PropertyEditor` (after the
    /// `ScrollArea` body child). Mirrors `TreeView::create_footer_widget`.
    ///
    /// Panics if called more than once.
    pub fn create_footer_widget(&mut self) -> PropertyEditorFooter {
        assert!(
            !self.footer_taken,
            "create_footer_widget() called more than once"
        );
        self.footer_taken = true;
        PropertyEditorFooter::new(self.core.clone())
    }

    /// Create the body content widget (`PropertyEditorBody`).
    ///
    /// Call this before adding PE to the widget tree. Add the returned
    /// body as the content of a `ScrollArea`, then add the `ScrollArea`
    /// as a child of this PropertyEditor. Mirrors
    /// `TreeView::create_body_widget`.
    pub fn create_body_widget(&self) -> PropertyEditorBody {
        PropertyEditorBody::new(self.core.clone())
    }
}

impl Default for PropertyEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for PropertyEditor {
    fn update(&mut self, _dt: f32, _cx: &mut UpdateContext) {
        // SPEC-HAL-337 §4.1 / INV-337-M1: the per-frame application hook.
        // Tree(ToggleExpand) dispatches enqueued identity-resolved node
        // ids during event routing; apply them here so this frame's
        // layout sees the new topology. No choreography — the synchronous
        // arm this replaces performed none (focus reconciliation in PE is
        // owned by the selection path).
        let mut core = self.core.borrow_mut();
        if core.pending_topology_toggles.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut core.pending_topology_toggles);
        for node_id in ids {
            core.cms.hts_mut().toggle_by_id(node_id);
        }
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[crate::core::widget_tree::NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width;
        let h = constraints.max_height;
        let header_h = self.header_height;
        let footer_h = self.footer_height;
        let body_h = (h - header_h - footer_h).max(0.0);
        self.body_height = body_h;

        // Resolve column geometry once at composite level — before any
        // child layout. Body is a pure consumer of resolved geometry
        // (mirrors TreeView at tree_view.rs:805). CMS owns the
        // content-width measurement cache (SPEC-HAL-180) and the HTS
        // resolve call; prepare_layout consolidates both into a single
        // call gated on data-revision / scale changes.
        let sf = self.core.borrow().scale_factor;
        self.core.borrow_mut().cms.prepare_layout(cx, w, sf);

        // Child 0 — body (typically a consumer-wrapped ScrollArea + Body).
        // Header is painted by PE Outer at the top strip.
        if !children.is_empty() {
            let c = Constraints::new(w, w, body_h, body_h);
            cx.layout_child(children[0], c);
            cx.set_child_rect(children[0], RectF::new(0.0, header_h, w, body_h));
        }

        // I1: child 1 — optional footer (`PropertyEditorFooter`). Laid
        // out below the body band when both (a) `footer_height > 0` and
        // (b) a second child is present. When `footer_height == 0` any
        // second child is left un-laid-out, matching the "no footer"
        // contract of `set_footer_height(0.0)`.
        if footer_h > 0.0 && children.len() >= 2 {
            let c = Constraints::new(w, w, footer_h, footer_h);
            cx.layout_child(children[1], c);
            cx.set_child_rect(children[1], RectF::new(0.0, header_h + body_h, w, footer_h));
        }

        let size = Size::new(w, h);
        LayoutResult::new(size, size)
    }

    fn paint(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _children: &[crate::core::widget_tree::NodeId],
    ) {
        // SPEC-HAL-260A §14.5: header paint — background fill, column
        // titles (vertically centered, per-slot clipped), column
        // separators (1px subtle / 2px hover / 2px focus-colored
        // active), and a 1px header bottom border. Body and any other
        // children paint themselves via framework tree traversal; this
        // method owns only the header strip. Mirror of TreeView at
        // `tree_view.rs:828-936`.
        let theme = cx.theme;
        let header_h = self.header_height;

        // Sync `scale_factor` from paint context. The next layout pass
        // uses it for column geometry resolution. RefCell interior
        // mutability does not violate `&self` — state lives behind
        // Rc<RefCell>, not as widget-owned mutable state.
        self.core.borrow_mut().scale_factor = cx.scene.scale_factor();

        // Header background — full strip width.
        let header_rect = RectF::new(rect.x, rect.y, rect.width, header_h);
        cx.scene.push_rect(
            header_rect,
            theme.colors.list.header_background,
            LAYER_CONTENT,
            None,
        );

        // Collect slots, titles, scroll_x, and separator state in a
        // single core borrow.
        let core = self.core.borrow();
        let slots: Vec<_> = core.cms.hts().column_layout().header_slots().collect();
        let titles: Vec<_> = slots
            .iter()
            .map(|s| core.column_titles.get(&s.column_id).cloned())
            .collect();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let hovered_sep = core.hovered_separator;
        let active_sep = core.active_resize_separator;
        drop(core);

        let font_size = theme.typography.font_size;
        // SPEC-HAL-260A §14.5: header titles align with data column text
        // inset so each column reads as a vertical strip with one
        // consistent left edge. Body B3b uses `text_field.padding_x`
        // (matches editor children); header mirrors. See body.rs:910
        // comment for rationale.
        let text_padding = theme.geometry.text_field.padding_x;
        let header_text_color = theme.colors.typography.secondary;
        let separator_color = theme.colors.borders.default;

        // Header clip — constrains title text + separators to the
        // header strip. Per-slot clips below further constrain each
        // title to its column boundary.
        cx.scene.begin_clip(header_rect, None);
        let sf = cx.scene.scale_factor();
        let snapped_x = snap_to_physical(rect.x, sf);
        let slot_count = slots.len();

        for (i, (slot, title)) in slots.iter().zip(titles.iter()).enumerate() {
            // Scroll-domain-aware X: subtract scroll_x only for
            // scrollable columns; pinned columns ignore horizontal
            // scroll.
            let visual_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                snapped_x + slot.x - scroll_x
            } else {
                snapped_x + slot.x
            };

            // Separator X — snap in content space, then subtract
            // scroll_x for scrollable columns. Matches the per-slot
            // clip right edge for consistent rounding.
            let sep_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                snap_to_physical(snapped_x + slot.x + slot.width - 1.0, sf) - scroll_x
            } else {
                snap_to_physical(snapped_x + slot.x + slot.width - 1.0, sf)
            };

            // Per-slot clip — keeps long titles from overflowing into
            // adjacent columns. Right edge aligns with the separator X
            // so the title text never overlaps the separator pixel.
            let cell_clip_w = (sep_x - visual_x).max(0.0);
            cx.scene
                .begin_clip(RectF::new(visual_x, rect.y, cell_clip_w, header_h), None);

            if let Some(title) = title {
                let run = cx.text_engine.shape(title, font_size);
                // SPEC-HAL-092 §8A: canonical baseline placement. Header
                // height differs from row height but the rule is the same
                // — use ascent+descent, not run.height.
                let metrics = cx.text_engine.font_metrics(font_size);
                let baseline_y = rect.y
                    + compute_control_text_baseline_y(
                        header_h,
                        metrics.ascent,
                        metrics.descent,
                        theme.typography.baseline_offset,
                    );
                let text_y = baseline_y - metrics.ascent;
                let text_x = visual_x + text_padding;
                cx.text_engine.draw(
                    &run,
                    text_x,
                    text_y,
                    header_text_color,
                    LAYER_CONTENT,
                    cx.scene,
                );
            }

            cx.scene.end_clip();

            // Column separator (skip after last column). Width / color
            // vary with hover/active state — O4 mutates
            // `hovered_separator` / `active_resize_separator` from
            // pointer events; for now both are None and all separators
            // paint at 1px default.
            if i < slot_count - 1 {
                let is_active = active_sep == Some(slot.column_id);
                let is_hovered = hovered_sep == Some(slot.column_id);
                let (sep_width, sep_color) = if is_active {
                    (2.0, theme.colors.borders.focus)
                } else if is_hovered {
                    (2.0, theme.colors.borders.default)
                } else {
                    (1.0, separator_color)
                };
                cx.scene.push_rect(
                    RectF::new(sep_x, rect.y, sep_width, header_h),
                    sep_color,
                    LAYER_CONTENT + 1,
                    None,
                );
            }
        }

        cx.scene.end_clip();

        // Header bottom border — outside the header clip; full PE
        // width.
        let border_y = snap_to_physical(rect.y + header_h - 1.0, sf);
        cx.scene.push_rect(
            RectF::new(rect.x, border_y, rect.width, 1.0),
            separator_color,
            LAYER_CONTENT + 1,
            None,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // -----------------------------------------------------------------
        // O4: column resize (SPEC-HAL-260A §8.2 / §11.2).
        //
        // Mirrors `TreeView::handle_event`'s pointer block
        // (tree_view.rs:972–1043 Move; 1048–1109 Down; 1114–1136 Up).
        //
        // Phase discipline (INV-SEPARATOR-HITTEST-EXCLUDES-SCROLLBAR):
        //   - Move(Idle), Down, Up(Armed) accept Target + Bubble only.
        //     Capture-phase routing in those states is reserved for
        //     ScrollArea horizontal-scrollbar overlay hits.
        //   - Move(Dragging) and Up(Dragging) accept Target + Capture
        //     because the captured pointer routes through Capture once
        //     the drag transition has taken capture.
        //
        // SPEC §10.5 forbids PE-side Escape handling: no drag-cancel
        // on Escape. If a drag is in flight when the framework releases
        // the pointer (e.g. capture revoked), the next PointerUp from
        // Idle just no-ops; state will reset on a subsequent
        // PointerDown / Up cycle. Acceptable for the conformant path.
        // -----------------------------------------------------------------
        if let Event::Pointer(pe) = event {
            match pe.kind {
                // ----------------------------------------------------
                // Move
                // ----------------------------------------------------
                PointerEventKind::Move => match self.resize_state {
                    ResizeState::Idle => {
                        if phase != EventPhase::Target && phase != EventPhase::Bubble {
                            return false;
                        }
                        if cx.has_pointer_capture() {
                            return false;
                        }
                        let Some(rect) = cx.widget_global_rect() else {
                            return false;
                        };
                        let local_x = pe.global_pos.0 - rect.x;
                        let local_y = pe.global_pos.1 - rect.y;
                        let hit_sep = self.hit_test_separator(local_x, local_y);
                        self.core.borrow_mut().hovered_separator = hit_sep;
                        if hit_sep.is_some() {
                            cx.request_cursor(CursorIcon::ColResize);
                        }
                        return false;
                    }
                    ResizeState::Armed {
                        column_id,
                        start_x,
                        initial_width,
                    } => {
                        // First move after arm: transition Armed →
                        // Dragging, take pointer capture, apply the
                        // first width delta.
                        self.resize_state = ResizeState::Dragging {
                            column_id,
                            start_x,
                            initial_width,
                        };
                        cx.capture_pointer(PointerButton::Primary);
                        let delta_x = pe.global_pos.0 - start_x;
                        let new_width = initial_width + delta_x;
                        self.apply_column_width(column_id, new_width);
                        cx.invalidate_layout();
                        if let Some(body_id) = self.core.borrow().body_widget_id {
                            cx.invalidate_layout_by_id(body_id);
                        }
                        cx.request_continuous_frames();
                        cx.request_cursor(CursorIcon::ColResize);
                        return true;
                    }
                    ResizeState::Dragging {
                        column_id,
                        start_x,
                        initial_width,
                    } => {
                        if phase != EventPhase::Capture && phase != EventPhase::Target {
                            return false;
                        }
                        let delta_x = pe.global_pos.0 - start_x;
                        let new_width = initial_width + delta_x;
                        self.apply_column_width(column_id, new_width);
                        cx.invalidate_layout();
                        if let Some(body_id) = self.core.borrow().body_widget_id {
                            cx.invalidate_layout_by_id(body_id);
                        }
                        cx.request_continuous_frames();
                        cx.request_cursor(CursorIcon::ColResize);
                        return true;
                    }
                },
                // ----------------------------------------------------
                // Down — arm resize on separator hit
                // ----------------------------------------------------
                PointerEventKind::Down => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    if !matches!(self.resize_state, ResizeState::Idle) {
                        return false;
                    }
                    if phase != EventPhase::Target && phase != EventPhase::Bubble {
                        return false;
                    }
                    if cx.has_pointer_capture() {
                        return false;
                    }
                    let Some(rect) = cx.widget_global_rect() else {
                        return false;
                    };
                    let local_x = pe.global_pos.0 - rect.x;
                    let local_y = pe.global_pos.1 - rect.y;
                    let Some(column_id) = self.hit_test_separator(local_x, local_y) else {
                        return false;
                    };
                    let initial_width = self.get_column_width(column_id).unwrap_or(100.0);
                    self.resize_state = ResizeState::Armed {
                        column_id,
                        start_x: pe.global_pos.0,
                        initial_width,
                    };
                    self.core.borrow_mut().active_resize_separator = Some(column_id);
                    cx.request_cursor(CursorIcon::ColResize);
                    return true;
                }
                // ----------------------------------------------------
                // Up — finalize resize (commit current width)
                // ----------------------------------------------------
                PointerEventKind::Up => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    match self.resize_state {
                        ResizeState::Dragging { .. } => {
                            if phase == EventPhase::Capture || phase == EventPhase::Target {
                                cx.release_pointer();
                                self.finalize_resize();
                                return true;
                            }
                            return false;
                        }
                        ResizeState::Armed { .. } => {
                            if phase == EventPhase::Target || phase == EventPhase::Bubble {
                                self.finalize_resize();
                                return true;
                            }
                            return false;
                        }
                        ResizeState::Idle => {
                            return false;
                        }
                    }
                }
                // ----------------------------------------------------
                // All other pointer kinds (Scroll, DoubleClick,
                // TripleClick, DragStart): not handled by O4. Returning
                // false lets the event continue its normal phase walk.
                // ----------------------------------------------------
                _ => return false,
            }
        }

        // O3a: row-axis keyboard navigation only. Target-only —
        // bubble / capture phases are not consumed here. O3b adds Left
        // / Right (column nav); O3c adds Enter / Space / F2 / Escape;
        // O3d adds Tab (CommitAndAdvance during edit).
        if phase != EventPhase::Target {
            return false;
        }
        let Event::Key(key) = event else {
            return false;
        };
        if !key.is_down {
            return false;
        }

        // SPEC-HAL-260A §5.5.1 / §10.5: band keyboard expand/collapse.
        // Mirrors the TreeView convention at tree_view.rs:567-702.
        // Active ONLY when focus is on a Section / SubSection row —
        // for Data rows the `if let Some(focus_id) = band_focus` block
        // does not match and control falls through to the column-nav
        // handler immediately below.
        //
        //   Right / `l`              → expand collapsed band, OR
        //                              descend to first child if
        //                              already expanded.
        //   Left  / `h`              → collapse expanded band, OR
        //                              ascend to parent if already
        //                              collapsed (no-op at depth 0).
        //   Ctrl/Cmd + Right / `l`   → expand_subtree_by_id (recursive
        //                              expand of all descendants).
        //   Ctrl/Cmd + Left  / `h`   → collapse_subtree_by_id
        //                              (recursive collapse).
        //
        // Shift modifier on the descend / ascend cases switches
        // FocusTo → ExtendTo for selection-range extension, matching
        // TreeView. Topology mutations call hts methods directly (Tree
        // bypass of the intent funnel — pre-existing pattern, META
        // cleanup tracked separately and matches B4 disclosure and
        // T6 DoubleClick band-activation paths). Focus mutations
        // route through `dispatch_intent(PEIntent::Selection(..))`
        // per INV-PE-INTENT-FUNNEL.
        let band_focus = {
            let core = self.core.borrow();
            core.selection.focus_key.filter(|&k| core.is_section_row(k))
        };
        if let Some(focus_id) = band_focus {
            let recursive = key.modifiers.ctrl || key.modifiers.meta;
            let is_expand_key = matches!(key.key_code, KeyCode::Right | KeyCode::Character('l'));
            let is_collapse_key = matches!(key.key_code, KeyCode::Left | KeyCode::Character('h'));

            if is_expand_key && recursive {
                let changed = {
                    let core = self.core.borrow_mut();
                    core.cms.hts_mut().expand_subtree_by_id(focus_id)
                };
                if changed {
                    let mut core = self.core.borrow_mut();
                    request_scroll_to_focus(&mut core);
                    drop(core);
                    cx.invalidate_layout();
                    return true;
                }
                return false;
            }

            if is_collapse_key && recursive {
                let changed = {
                    let core = self.core.borrow_mut();
                    core.cms.hts_mut().collapse_subtree_by_id(focus_id)
                };
                if changed {
                    let mut core = self.core.borrow_mut();
                    request_scroll_to_focus(&mut core);
                    drop(core);
                    cx.invalidate_layout();
                    return true;
                }
                return false;
            }

            if is_expand_key {
                // Snapshot the band's topology state under a single
                // mut borrow (HTS::visible_rows takes &mut self —
                // orchestrates Topology → Virtualization phases); act
                // after the borrow releases so the subsequent
                // borrow_mut for `expand_by_id` / `dispatch_intent`
                // is safe. Matches TreeView's pattern at
                // tree_view.rs:592-595.
                let snap = {
                    let core = self.core.borrow_mut();
                    let visible = core.cms.hts_mut().visible_rows();
                    visible
                        .iter()
                        .position(|vr| vr.node_id == focus_id)
                        .map(|vi| {
                            let vr = &visible[vi];
                            let child_id =
                                if vr.has_children && vr.expanded && vi + 1 < visible.len() {
                                    Some(visible[vi + 1].node_id)
                                } else {
                                    None
                                };
                            (vr.has_children, vr.expanded, child_id)
                        })
                };
                if let Some((has_children, expanded, child_id)) = snap {
                    if has_children && !expanded {
                        {
                            let mut core = self.core.borrow_mut();
                            core.cms.hts_mut().expand_by_id(focus_id);
                            request_scroll_to_focus(&mut core);
                        }
                        cx.invalidate_layout();
                        return true;
                    } else if let Some(child) = child_id {
                        let action = if key.modifiers.shift {
                            OrderedSelectionAction::ExtendTo { key: child }
                        } else {
                            OrderedSelectionAction::FocusTo { key: child }
                        };
                        let mut core = self.core.borrow_mut();
                        core.dispatch_intent(PEIntent::Selection(action));
                        request_scroll_to_focus(&mut core);
                        drop(core);
                        cx.invalidate_layout();
                        return true;
                    }
                }
                return false;
            }

            if is_collapse_key {
                // HTS::visible_rows takes &mut self — see expand-path
                // snap closure above for the rationale.
                let snap = {
                    let core = self.core.borrow_mut();
                    let visible = core.cms.hts_mut().visible_rows();
                    visible
                        .iter()
                        .position(|vr| vr.node_id == focus_id)
                        .map(|vi| {
                            let vr = &visible[vi];
                            let parent_id = if vr.depth > 0 {
                                let target_depth = vr.depth - 1;
                                (0..vi)
                                    .rev()
                                    .find(|&j| visible[j].depth == target_depth)
                                    .map(|j| visible[j].node_id)
                            } else {
                                None
                            };
                            (vr.expanded, parent_id)
                        })
                };
                if let Some((expanded, parent_id)) = snap {
                    if expanded {
                        {
                            let mut core = self.core.borrow_mut();
                            core.cms.hts_mut().collapse_by_id(focus_id);
                            request_scroll_to_focus(&mut core);
                        }
                        cx.invalidate_layout();
                        return true;
                    } else if let Some(parent) = parent_id {
                        let action = if key.modifiers.shift {
                            OrderedSelectionAction::ExtendTo { key: parent }
                        } else {
                            OrderedSelectionAction::FocusTo { key: parent }
                        };
                        let mut core = self.core.borrow_mut();
                        core.dispatch_intent(PEIntent::Selection(action));
                        request_scroll_to_focus(&mut core);
                        drop(core);
                        cx.invalidate_layout();
                        return true;
                    }
                }
                return false;
            }
            // Not a tree-nav key on a band — fall through to other
            // handlers below.
        }

        // O3b: column-axis nav (Left / Right). Walks slot order; no
        // wrap. Independent from the row-axis match below — Left /
        // Right do not consume page_size or share OrderedSelection
        // semantics. Emitted as `ColumnIntent::To { column_id }`
        // through the intent funnel (INV-PE-INTENT-FUNNEL — the
        // permission gate decides Permit / Deny). Horizontal
        // scroll-to-reveal for the newly focused column is a separate
        // concern (HorizontalScrollLink) deferred past O3b.
        if matches!(key.key_code, KeyCode::Left | KeyCode::Right) {
            let direction: i32 = if key.key_code == KeyCode::Left { -1 } else { 1 };
            let target_col = {
                let core = self.core.borrow();
                next_column(&core, direction)
            };
            if let Some(column_id) = target_col {
                self.core
                    .borrow_mut()
                    .dispatch_intent(PEIntent::Column(ColumnIntent::To { column_id }));
                cx.invalidate_layout();
                return true;
            }
            return false;
        }

        // O3c: activation. SPEC-HAL-260A §10.5:
        //   Enter / `i` → `PEIntent::Activate { row_id: focus_key }`.
        //   Cell-aware effect routed inside `activate_row` (core.rs).
        //   Space → `OrderedSelectionAction::ToggleFocused` (row
        //   selection toggle, uniform across cell archetypes).
        // No focus_key → no row to activate / toggle; return false so
        // framework can route the key elsewhere. Escape is NOT handled
        // here — SPEC §10.5 + FORBID-KBD-ESCAPE-SEMANTICS reserve it
        // for the framework.
        let is_activate = matches!(key.key_code, KeyCode::Enter)
            || matches!(key.key_code, KeyCode::Character('i'));
        if is_activate {
            let Some(row_id) = self.core.borrow().selection.focus_key else {
                return false;
            };
            self.core
                .borrow_mut()
                .dispatch_intent(PEIntent::Activate { row_id });
            cx.invalidate_layout();
            return true;
        }
        if matches!(key.key_code, KeyCode::Space) {
            let has_focus = self.core.borrow().selection.focus_key.is_some();
            if !has_focus {
                return false;
            }
            self.core
                .borrow_mut()
                .dispatch_intent(PEIntent::Selection(OrderedSelectionAction::ToggleFocused));
            cx.invalidate_layout();
            return true;
        }

        // O3d: Tab / Shift+Tab cell traversal (Navigation mode).
        // SPEC-HAL-260A §10.4 + INV-PE-TAB-PE-DRIVEN: PE consumes Tab
        // inside its focus scope; the framework's focus_next chain is
        // reserved for boundary exits (suppressed here by
        // tab_at_last_cell = NoOp default).
        //
        // Forward (direction = +1) / Shift+Tab (direction = -1):
        // - Within row: step to next / previous navigable column.
        // - At row-end column: per `tab_at_row_end` — StayInRow consumes
        //   without movement; WrapToNextRow attempts a row-axis move via
        //   FocusNext / FocusPrevious, then column-resets to the first /
        //   last navigable column of the new row.
        // - At PE's overall last (or first) cell: per `tab_at_last_cell`
        //   — NoOp consumes without movement; WrapToFirstCell wraps to
        //   the first / last row and first / last navigable column.
        //
        // Ctrl / Meta / Alt + Tab is NOT consumed — those modifier combos
        // belong to app-level or OS-level handlers.
        if matches!(key.key_code, KeyCode::Tab) {
            if key.modifiers.ctrl || key.modifiers.meta || key.modifiers.alt {
                return false;
            }
            let direction: i32 = if key.modifiers.shift { -1 } else { 1 };

            // Snapshot navigation context under a single borrow.
            let (focus_key, current_col, nav_cols, cur_row_idx, total_rows, p_row_end, p_last_cell) = {
                let core = self.core.borrow();
                let focus_key = core.selection.focus_key;
                let current_col = core.cms.focus().col_id();
                let nav_cols = focus_key
                    .map(|rk| core.navigable_columns_for_row(rk))
                    .unwrap_or_default();
                let total_rows = core.cms.hts().len();
                let cur_row_idx = focus_key.and_then(|k| core.cms.hts().index_of(k));
                let p_row_end = core.policy.tab_at_row_end;
                let p_last_cell = core.policy.tab_at_last_cell;
                (
                    focus_key,
                    current_col,
                    nav_cols,
                    cur_row_idx,
                    total_rows,
                    p_row_end,
                    p_last_cell,
                )
            };

            // No focus or no navigable columns: consume Tab (INV-F8
            // trap) without movement.
            let Some(_focus_key) = focus_key else {
                return true;
            };
            if nav_cols.is_empty() {
                return true;
            }

            // Locate the current column inside the navigable list.
            // None = either no column focused yet (first Tab), or
            // current column is stale (no longer registered / not
            // navigable on this row kind). Both treated as "clamp to
            // edge" — forward clamps to first, backward to last.
            let col_pos = current_col.and_then(|c| nav_cols.iter().position(|x| *x == c));

            if col_pos.is_none() {
                let target = if direction > 0 {
                    nav_cols[0]
                } else {
                    nav_cols[nav_cols.len() - 1]
                };
                self.core
                    .borrow_mut()
                    .dispatch_intent(PEIntent::Column(ColumnIntent::To { column_id: target }));
                cx.invalidate_layout();
                return true;
            }
            let col_pos = col_pos.unwrap();

            // Within-row column step?
            let can_step_within_row = if direction > 0 {
                col_pos + 1 < nav_cols.len()
            } else {
                col_pos > 0
            };
            if can_step_within_row {
                let new_col = if direction > 0 {
                    nav_cols[col_pos + 1]
                } else {
                    nav_cols[col_pos - 1]
                };
                self.core
                    .borrow_mut()
                    .dispatch_intent(PEIntent::Column(ColumnIntent::To { column_id: new_col }));
                cx.invalidate_layout();
                return true;
            }

            // Row-end (forward) or row-start (backward). Consult policy.
            let can_step_row = match (cur_row_idx, direction) {
                (Some(i), d) if d > 0 && i + 1 < total_rows => true,
                (Some(i), d) if d < 0 && i > 0 => true,
                _ => false,
            };

            if can_step_row {
                match p_row_end {
                    TabRowEnd::StayInRow => {
                        // Consume without movement.
                        return true;
                    }
                    TabRowEnd::WrapToNextRow => {
                        let action = if direction > 0 {
                            OrderedSelectionAction::FocusNext
                        } else {
                            OrderedSelectionAction::FocusPrevious
                        };
                        self.core
                            .borrow_mut()
                            .dispatch_intent(PEIntent::Selection(action));
                        // Re-read focus_key to discover the new row,
                        // then column-reset.
                        let (new_row, new_col) = {
                            let core = self.core.borrow();
                            let new_row = core.selection.focus_key;
                            let new_col = new_row.and_then(|r| {
                                let nc = core.navigable_columns_for_row(r);
                                if direction > 0 {
                                    nc.first().copied()
                                } else {
                                    nc.last().copied()
                                }
                            });
                            (new_row, new_col)
                        };
                        if new_row.is_some() {
                            if let Some(c) = new_col {
                                self.core.borrow_mut().dispatch_intent(PEIntent::Column(
                                    ColumnIntent::To { column_id: c },
                                ));
                            }
                            let mut core = self.core.borrow_mut();
                            request_scroll_to_focus(&mut core);
                            drop(core);
                        }
                        cx.invalidate_layout();
                        return true;
                    }
                }
            }

            // PE's overall last (forward) or first (backward) cell.
            match p_last_cell {
                TabBoundary::NoOp => {
                    return true;
                }
                TabBoundary::WrapToFirstCell => {
                    // Guard against empty oracle: this branch is
                    // reachable with `total_rows == 0` when focus_key
                    // is stale (Some but not in the oracle —
                    // cur_row_idx then None, can_step_row then false,
                    // falls here). `key_at` panics on out-of-bounds
                    // per its trait docstring
                    // (ordered_selection/mod.rs:77-78).
                    if total_rows == 0 {
                        return true;
                    }
                    let target_row_idx = if direction > 0 { 0 } else { total_rows - 1 };
                    // `key_at` returns `u64` directly, NOT Option<u64>
                    // (HTS impl at hierarchical/mod.rs:579-582). With
                    // the `total_rows == 0` guard above, the call is
                    // panic-safe.
                    let target_row = self.core.borrow().cms.hts().key_at(target_row_idx);
                    let new_col = {
                        let core = self.core.borrow();
                        let nc = core.navigable_columns_for_row(target_row);
                        if direction > 0 {
                            nc.first().copied()
                        } else {
                            nc.last().copied()
                        }
                    };
                    self.core.borrow_mut().dispatch_intent(PEIntent::Selection(
                        OrderedSelectionAction::Select { key: target_row },
                    ));
                    if let Some(c) = new_col {
                        self.core
                            .borrow_mut()
                            .dispatch_intent(PEIntent::Column(ColumnIntent::To { column_id: c }));
                    }
                    let mut core = self.core.borrow_mut();
                    request_scroll_to_focus(&mut core);
                    drop(core);
                    cx.invalidate_layout();
                    return true;
                }
            }
        }

        // Page size from cached body_height. Falls back to 10 when
        // body_height hasn't been populated yet (pre-first-layout edge
        // case during programmatic event injection).
        let page_size = {
            let core = self.core.borrow();
            let rh = core.cms.hts().row_height();
            if rh > 0.0 && self.body_height > 0.0 {
                (self.body_height / rh).floor().max(1.0) as isize
            } else {
                10
            }
        };

        // Map row-axis keys to OrderedSelectionAction. Modifier
        // semantics:
        //   plain     → Focus*       (move focus). A follow-up
        //                            `Select { key: new_focus }` runs
        //                            after the dispatch when the new
        //                            focus row is a Data row —
        //                            select-follows-focus, mirrors the
        //                            mouse-click pathway in body.rs
        //                            (plain PointerDown → Select).
        //                            Bands are non-selectable per
        //                            §10.2, so the follow-up is
        //                            suppressed for band rows; their
        //                            `band_focused_overlay` carries
        //                            the active-state signal.
        //   Shift     → Extend*      (extend selection from anchor)
        //   Ctrl/Meta → MoveFocus*   (move focus only, keep selection)
        let action: OrderedSelectionAction<u64> = match key.key_code {
            KeyCode::Up if key.modifiers.shift => OrderedSelectionAction::ExtendPrevious,
            KeyCode::Up if key.modifiers.ctrl || key.modifiers.meta => {
                OrderedSelectionAction::MoveFocusPrevious
            }
            KeyCode::Up => OrderedSelectionAction::FocusPrevious,

            KeyCode::Down if key.modifiers.shift => OrderedSelectionAction::ExtendNext,
            KeyCode::Down if key.modifiers.ctrl || key.modifiers.meta => {
                OrderedSelectionAction::MoveFocusNext
            }
            KeyCode::Down => OrderedSelectionAction::FocusNext,

            KeyCode::Home if key.modifiers.shift => OrderedSelectionAction::ExtendFirst,
            KeyCode::Home if key.modifiers.ctrl || key.modifiers.meta => {
                OrderedSelectionAction::MoveFocusFirst
            }
            KeyCode::Home => OrderedSelectionAction::FocusFirst,

            KeyCode::End if key.modifiers.shift => OrderedSelectionAction::ExtendLast,
            KeyCode::End if key.modifiers.ctrl || key.modifiers.meta => {
                OrderedSelectionAction::MoveFocusLast
            }
            KeyCode::End => OrderedSelectionAction::FocusLast,

            KeyCode::PageUp if key.modifiers.shift => {
                OrderedSelectionAction::ExtendPage { delta: -page_size }
            }
            KeyCode::PageUp => OrderedSelectionAction::FocusPage { delta: -page_size },

            KeyCode::PageDown if key.modifiers.shift => {
                OrderedSelectionAction::ExtendPage { delta: page_size }
            }
            KeyCode::PageDown => OrderedSelectionAction::FocusPage { delta: page_size },

            _ => return false,
        };

        // Route through the intent funnel (INV-PE-INTENT-FUNNEL — the
        // permission gate runs first). After dispatch, request scroll
        // reveal for the new focused row before invalidating layout.
        //
        // SPEC-HAL-260A select-follows-focus (mouse parity): when the
        // primary action is a plain Focus* movement, dispatch a
        // follow-up `Select { key: new_focus }` so the new focus row
        // becomes the sole entry in `selected_keys`. This mirrors the
        // mouse PointerDown plain branch in body.rs which dispatches
        // `Select { key: node_id }` unconditionally (no archetype
        // check). The follow-up applies on bands too — §10.2's
        // "bands are non-selectable" governs the VISUAL treatment
        // (pass 2's band arms don't consult `is_selected`), not
        // `selected_keys` membership. Without this, stepping from a
        // Data row onto a band would leave the previously-selected
        // Data row in `selected_keys` and visually selected behind
        // the band focus — selection would lag focus by one step.
        // `was_plain_focus` is captured BEFORE the move so the
        // predicate survives the dispatch.
        let was_plain_focus = matches!(
            action,
            OrderedSelectionAction::FocusPrevious
                | OrderedSelectionAction::FocusNext
                | OrderedSelectionAction::FocusFirst
                | OrderedSelectionAction::FocusLast
                | OrderedSelectionAction::FocusPage { .. }
        );

        let mut core = self.core.borrow_mut();
        core.dispatch_intent(PEIntent::Selection(action));
        if was_plain_focus {
            if let Some(new_focus) = core.selection.focus_key {
                core.dispatch_intent(PEIntent::Selection(OrderedSelectionAction::Select {
                    key: new_focus,
                }));
            }
        }
        request_scroll_to_focus(&mut core);
        drop(core);
        cx.invalidate_layout();
        true
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        // SPEC-HAL-260A §10: PE is a focus target.
        true
    }

    fn defines_focus_scope(&self) -> bool {
        // SPEC-HAL-176 / SPEC-HAL-038 Stage 3: PE is a focus scope root
        // — Tab navigation is confined to PE's focusable descendants
        // (currently the active inline editor when one is mounted by
        // B6). Mutually exclusive with `capture_eligible` (which stays
        // default `false` for PE — PE itself does not consume raw
        // keyboard input as text).
        true
    }

    /// SPEC-HAL-260A §14.3 paint pass 2: `band_focused_overlay` and the
    /// data-row active selection color (`row_selected` vs
    /// `row_selected_inactive`) both gate on `core.focused`. This
    /// override mirrors `TreeView::on_focus` at `tree_view.rs:1211-1214`
    /// — the single seam through which the framework's focus state
    /// reaches the paint gate. Without it, `core.focused` stays at its
    /// init value (`false`) and band-focus / active-selection
    /// affordances never paint, regardless of `selection.focus_key`.
    fn on_focus(&mut self, cx: &mut EventContext) {
        self.core.borrow_mut().focused = true;
        cx.invalidate_layout();
    }

    /// SPEC-HAL-260A §14.3: the symmetric blur seam. Mirrors
    /// `TreeView::on_blur` at `tree_view.rs:1216-1228` (minus the
    /// resize-state cleanup, which is a separate INV-RESIZE-CLEAN-EXIT
    /// concern not yet wired on PE — tracked separately).
    fn on_blur(&mut self, cx: &mut EventContext) {
        self.core.borrow_mut().focused = false;
        cx.invalidate_layout();
    }

    fn debug_type_name(&self) -> &'static str {
        "PropertyEditor"
    }
}

impl WidgetDowncast for PropertyEditor {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
