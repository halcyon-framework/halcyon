// =============================================================================
// SPEC-HAL-259: TreeView — Composite Widget (ListView-Style Composition)
// =============================================================================
//
// Thin composite wrapper. Delegates body rendering to TreeViewBody
// inside ScrollArea. Mirrors ListView's composition pattern exactly.
//
// TreeView (composite)
//   └── ScrollArea (scroll state, scrollbars, clipping, smooth scroll)
//         └── TreeViewBody (layout, paint, expand/collapse)
//
// INV-TV-1: HTS is sole topology authority (via TreeViewCore).
// INV-TV-2: Widget does not store expansion state.
// =============================================================================

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::event::{
    CursorIcon, Event, EventPhase, KeyCode, KeyEvent, PointerButton, PointerEventKind,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::ordered_selection::{
    OrderedIndex, OrderedSelectionAction, OrderedSelectionState, SelectionMode,
    reduce_ordered_selection,
};
use crate::core::paint::LAYER_CONTENT;
use crate::core::scroll_container::HorizontalScrollLink;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, UpdateContext, Widget};
use crate::engines::cell_mechanics::CellMechanicsSubstrate;
use crate::engines::cell_source::CellSource;
use crate::engines::hierarchical::{
    HierarchicalTabularSubstrate, HtsOwnership, NodeMeta, SharedHts, VisibleRow,
};
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, ExpansionPolicy};
use crate::utils::id::WidgetInternalId;
use crate::widgets::cell_editor_factory::DefaultCellEditorFactory;
use crate::widgets::tabular_view::column::{ColumnId, ColumnModel, ColumnScrollDomain};
use crate::widgets::tabular_view::row::RowId;

use super::body::{
    TREE_INDENT_STEP, TreeTopologyIntent, TreeViewBody, TreeViewCore, emit_activation,
};
use super::footer::TreeViewFooter;

const DEFAULT_HEADER_HEIGHT: f32 = 24.0;

/// Ensure the focused node is visible after an expand/collapse/toggle.
///
/// If focus_key is absent or already in visible_rows, no-op.
/// Otherwise walks the parent chain (via topology) until a visible ancestor
/// is found and moves focus there. Falls back to the first visible row.
///
/// INV-NO-SELECTION-MUTATION: Only dispatches FocusTo (no selection change).
/// Compute scroll target to reveal the currently focused row.
/// Sets `core.requested_scroll_y` if the focused row is outside the viewport.
pub(crate) fn request_scroll_to_focus(core: &mut TreeViewCore) {
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
    // SPEC-PM-017: f32 shim — reveal math converts with the rung-3
    // request channel.
    let scroll_y = core.cms.hts().scroll_offset() as f32;
    let viewport_h = core.cms.hts().viewport_height();

    if row_top < scroll_y {
        core.requested_scroll_y = Some(row_top);
    } else if row_bottom > scroll_y + viewport_h {
        core.requested_scroll_y = Some(row_bottom - viewport_h);
    }
}

pub(crate) fn ensure_focus_visible(core: &mut TreeViewCore, visible_rows: &[VisibleRow]) {
    let focus_id = match core.selection.focus_key {
        Some(id) => id,
        None => return,
    };

    // Already visible — nothing to do.
    if visible_rows.iter().any(|vr| vr.node_id == focus_id) {
        return;
    }

    // Walk parent chain via topology.
    let mut current_id = focus_id;
    loop {
        let phys = match core.cms.hts().topology().index_for_id(current_id) {
            Some(p) => p,
            None => break,
        };
        let parent_phys = match core
            .cms
            .hts()
            .topology()
            .node_meta(phys)
            .and_then(|m| m.parent_index)
        {
            Some(p) => p,
            None => break,
        };
        let parent_id = core.cms.hts().topology().node_id(parent_phys);
        if visible_rows.iter().any(|vr| vr.node_id == parent_id) {
            let core_ref = &mut *core;
            reduce_ordered_selection(
                &mut core_ref.selection,
                OrderedSelectionAction::FocusTo { key: parent_id },
                &*core_ref.cms.hts(),
            );
            return;
        }
        current_id = parent_id;
    }

    // No visible ancestor — focus first visible row.
    if let Some(first) = visible_rows.first() {
        let first_id = first.node_id;
        let core_ref = &mut *core;
        reduce_ordered_selection(
            &mut core_ref.selection,
            OrderedSelectionAction::FocusTo { key: first_id },
            &*core_ref.cms.hts(),
        );
    }
}

/// Hit-test slop for column separator detection (pixels, each side).
pub(super) const SEPARATOR_HIT_HALF_WIDTH: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResizeSeparatorHit {
    column_id: ColumnId,
}

/// Column resize state machine (mirrors ListView's ResizeState).
///
/// States: Idle → Armed → Dragging → Idle
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

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeViewColumnProbeEntry {
    pub column_id: ColumnId,
    pub display_index: usize,
    pub visible: bool,
    pub resizable: bool,
    pub reorderable: bool,
    pub scroll_domain: ColumnScrollDomain,
    pub preferred_width: f32,
    pub resolved_x_offset: Option<f32>,
    pub resolved_width: Option<f32>,
    pub min_width: f32,
    pub max_width: Option<f32>,
    pub separator_x_content: Option<f32>,
    pub separator_x_viewport: Option<f32>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeViewStructuralProbeEntry {
    pub widget_id: u64,
    pub root_rect: RectF,
    pub header_rect: RectF,
    pub body_rect: RectF,
    pub footer_rect: RectF,
    pub horizontal_scroll_offset: f32,
    pub vertical_scroll_offset: f32,
    pub resize_state: &'static str,
    pub active_resize_separator: Option<ColumnId>,
    pub hovered_separator: Option<ColumnId>,
    pub pointer_in_expansion_region: bool,
    pub selected_node_ids: Vec<u64>,
    pub focused_node: Option<u64>,
    pub expanded_node_ids: Vec<u64>,
}

/// TreeView — composite hierarchical widget driven by SPEC-HAL-259 engines.
///
/// Owns shared state via `Rc<RefCell<TreeViewCore>>` (mirrors ListView's
/// `Rc<RefCell<ListViewCore>>` pattern). Creates TreeViewBody for tree
/// construction; caller wraps it in ScrollArea.
///
/// INV-TV-1: HTS is sole topology authority.
/// INV-TV-2: Widget does not store expansion state.
pub struct TreeView {
    id: WidgetInternalId,
    core: Rc<RefCell<TreeViewCore>>,
    /// Clone of the topology handle inside `core.cms` (INV-337-O1: same
    /// instance). Lets `hts()`/`hts_mut()` return guards tied to &self.
    hts: SharedHts,
    /// SPEC-HAL-337 §2: who constructed the topology this widget projects.
    ownership: HtsOwnership,
    header_height: f32,
    footer_height: f32,
    footer_taken: bool,
    resize_state: ResizeState,
    /// Cached font_size from theme (for auto-size text measurement).
    font_size: f32,
    /// Cached text_padding from theme (for auto-size calculation).
    text_padding: f32,
    /// Cached body height from last layout (for page size calculation).
    body_height: f32,
    /// Cached body width from last layout (for probe geometry).
    body_width: f32,
}

impl TreeView {
    /// Create a new TreeView with an internally minted topology substrate
    /// (SPEC-HAL-337 §2: `HtsOwnership::Internal`). The handle is exposed
    /// via `hts_handle()`.
    pub fn new() -> Self {
        let hts: SharedHts = Rc::new(RefCell::new(HierarchicalTabularSubstrate::new()));
        Self::with_handle(hts, HtsOwnership::Internal)
    }

    /// Create a TreeView projecting an injected shared topology substrate
    /// (SPEC-HAL-337 §2: `HtsOwnership::External`). Exactly one substrate
    /// instance exists per topology (INV-337-O1): the given handle is
    /// threaded into the CMS and the widget unchanged.
    pub fn with_hts(hts: SharedHts) -> Self {
        Self::with_handle(hts, HtsOwnership::External)
    }

    /// Shared constructor body — one code shape, no dual paths
    /// (SPEC-HAL-337 §2).
    fn with_handle(hts: SharedHts, ownership: HtsOwnership) -> Self {
        let link = HorizontalScrollLink::new();
        let freeze = Rc::new(Cell::new(false));
        let mut cms = CellMechanicsSubstrate::new(Rc::clone(&hts));
        cms.set_editor_factory(Box::new(DefaultCellEditorFactory::new()));
        Self {
            id: WidgetInternalId::new(),
            core: Rc::new(RefCell::new(TreeViewCore {
                activation_callback: None,
                activation_press: None,
                cms,
                tree_column_id: None,
                column_titles: HashMap::new(),
                selection: OrderedSelectionState::new(SelectionMode::Multi),
                selection_drag_anchor: None,
                selection_drag_active: false,
                selection_drag_last: None,
                hovered_separator: None,
                active_resize_separator: None,
                hovered_node: None,
                horizontal_scroll_link: Some(link),
                horizontal_scroll_freeze: Some(freeze),
                focused: false,
                body_widget_id: None,
                requested_scroll_y: None,
                scale_factor: 1.0,
                layout_configuration: 0,
                row_widget_factory: None,
                cell_source: None,
                topology_intents: Vec::new(),
            })),
            hts,
            ownership,
            header_height: DEFAULT_HEADER_HEIGHT,
            footer_height: 0.0,
            footer_taken: false,
            resize_state: ResizeState::Idle,
            font_size: 14.0,
            text_padding: 6.0,
            body_height: 0.0,
            body_width: 0.0,
        }
    }

    /// Create the body content widget (TreeViewBody).
    ///
    /// Call this before adding TreeView to the widget tree.
    /// Add the returned TreeViewBody as content of a ScrollArea,
    /// then add that ScrollArea as a child of TreeView.
    ///
    /// Mirrors ListView::create_body_widget().
    pub fn create_body_widget(&self) -> TreeViewBody {
        TreeViewBody::new(self.core.clone())
    }

    /// Set footer height (0 = no footer).
    pub fn set_footer_height(&mut self, h: f32) {
        self.footer_height = h;
        let mut core = self.core.borrow_mut();
        core.layout_configuration = core.layout_configuration.saturating_add(1);
    }

    /// Show or omit the owner-rendered header band.
    ///
    /// Configure this before mounting the TreeView. A hidden header resolves to
    /// zero height; the body remains inside the owning composite and begins at
    /// the TreeView's top edge.
    pub fn set_header_visible(&mut self, visible: bool) {
        self.header_height = if visible { DEFAULT_HEADER_HEIGHT } else { 0.0 };
        let mut core = self.core.borrow_mut();
        core.layout_configuration = core.layout_configuration.saturating_add(1);
    }

    /// Read whether the owner-rendered header band is present.
    pub fn header_visible(&self) -> bool {
        self.header_height > 0.0
    }

    /// Activate a live leaf after a qualified click or focused routed Enter.
    /// The callback receives a stable node ID after core borrows end. Enqueue
    /// that ID for the consumer's deferred action; do not mutate the tree here.
    pub fn set_activation_callback<F>(&mut self, callback: F)
    where
        F: FnMut(u64) + 'static,
    {
        let previous = self
            .core
            .borrow_mut()
            .activation_callback
            .replace(Box::new(callback));
        drop(previous);
    }

    /// Create the footer widget (TreeViewFooter).
    ///
    /// Call this before adding TreeView to the widget tree.
    /// Add the returned TreeViewFooter as a second child of TreeView
    /// (after the ScrollArea child).
    ///
    /// Panics if called more than once.
    pub fn create_footer_widget(&mut self) -> TreeViewFooter {
        assert!(
            !self.footer_taken,
            "create_footer_widget() called more than once"
        );
        self.footer_taken = true;
        TreeViewFooter::new(self.core.clone())
    }

    /// Get the horizontal scroll link for wiring to ScrollArea.
    ///
    /// Caller should pass `link.clone()` to `ScrollArea::with_horizontal_scroll_link()`.
    /// ScrollArea writes scroll_x; TreeView header reads it.
    pub fn horizontal_scroll_link(&self) -> HorizontalScrollLink {
        self.core
            .borrow()
            .horizontal_scroll_link
            .clone()
            .unwrap_or_default()
    }

    /// Get the horizontal scroll freeze flag for wiring to ScrollArea.
    ///
    /// Caller should pass `flag.clone()` to `ScrollArea::with_horizontal_freeze()`.
    /// TreeView sets the flag while a column resize is active.
    pub fn horizontal_scroll_freeze(&self) -> Rc<Cell<bool>> {
        self.core
            .borrow()
            .horizontal_scroll_freeze
            .clone()
            .unwrap_or_else(|| Rc::new(Cell::new(false)))
    }

    /// Set tree data (nodes with parent/child relationships).
    pub fn set_tree_data(&mut self, nodes: Vec<NodeMeta>) {
        self.core.borrow_mut().cms.hts_mut().set_tree_data(nodes);
    }

    /// Set columns.
    pub fn set_columns(&mut self, model: ColumnModel) {
        self.core.borrow_mut().cms.hts_mut().set_columns(model);
    }

    /// Set which column is the tree column.
    ///
    /// This column receives indentation, disclosure triangles, guide lines,
    /// and node labels. Other columns render as standard cells.
    pub fn set_tree_column(&mut self, column_id: ColumnId) {
        let mut core = self.core.borrow_mut();
        core.tree_column_id = Some(column_id);
        core.layout_configuration = core.layout_configuration.saturating_add(1);
    }

    /// Set a column title (rendered in the header).
    pub fn set_column_title(&mut self, column_id: ColumnId, title: impl Into<String>) {
        let mut core = self.core.borrow_mut();
        core.column_titles.insert(column_id, title.into());
        core.layout_configuration = core.layout_configuration.saturating_add(1);
    }

    /// Set row height.
    pub fn set_row_height(&mut self, height: f32) {
        self.core.borrow_mut().cms.hts_mut().set_row_height(height);
    }

    /// Expand a node.
    pub fn expand(&mut self, row_index: usize) {
        self.core.borrow_mut().cms.hts_mut().expand(row_index);
    }

    /// Collapse a node.
    pub fn collapse(&mut self, row_index: usize) {
        self.core.borrow_mut().cms.hts_mut().collapse(row_index);
    }

    /// Toggle a node's expansion state.
    pub fn toggle(&mut self, row_index: usize) {
        self.core.borrow_mut().cms.hts_mut().toggle(row_index);
    }

    /// Get revision counter.
    pub fn revision(&self) -> u64 {
        self.core.borrow().cms.hts().revision()
    }

    /// Access HTS (immutable). Frame-ephemeral guard (INV-337-A1).
    pub fn hts(&self) -> std::cell::Ref<'_, HierarchicalTabularSubstrate> {
        self.hts.borrow()
    }

    /// Access HTS (mutable). Frame-ephemeral guard (INV-337-A1).
    pub fn hts_mut(&self) -> std::cell::RefMut<'_, HierarchicalTabularSubstrate> {
        self.hts.borrow_mut()
    }

    /// Clone of the shared topology handle (SPEC-HAL-337 §2).
    pub fn hts_handle(&self) -> SharedHts {
        Rc::clone(&self.hts)
    }

    /// Who constructed the topology this widget projects (SPEC-HAL-337 §2).
    pub fn ownership(&self) -> HtsOwnership {
        self.ownership
    }

    /// SPEC-HAL-337 §4.1: enqueue a topology intent for application by the
    /// per-frame hook (`Widget::update`) before the next layout. The funnel
    /// entry for event handlers and tests. Programmatic consumer mutation
    /// outside event routing may keep using the direct substrate API.
    #[allow(
        dead_code,
        reason = "Retained typed entry to the existing deferred topology queue"
    )]
    pub(crate) fn enqueue_topology_intent(&self, intent: TreeTopologyIntent) {
        self.core.borrow_mut().topology_intents.push(intent);
    }

    /// SPEC-HAL-313 §3.5: Set expansion policy.
    pub fn set_expansion_policy(&mut self, policy: ExpansionPolicy) {
        self.core
            .borrow_mut()
            .cms
            .hts_mut()
            .set_expansion_policy(policy);
    }

    /// SPEC-HAL-313 §3.5: Set expanded height policy.
    pub fn set_expanded_height_policy(&mut self, policy: ExpandedHeightPolicy) {
        self.core
            .borrow_mut()
            .cms
            .hts_mut()
            .set_expanded_height_policy(policy);
    }

    /// SPEC-HAL-313 §3.5: Set the widget factory for expanded rows.
    ///
    /// The factory is called once per row when expanded. The produced widget
    /// lives until the row is collapsed.
    ///
    /// NOTE: Body integration is deferred until HTS supports variable row heights.
    /// The factory is stored but not consumed until that prerequisite is met.
    ///
    /// INV-313-FACTORY-FN-1: Factory must be Fn (not FnMut).
    pub fn set_row_widget_factory(&mut self, f: impl Fn(RowId) -> Box<dyn Widget> + 'static) {
        self.core.borrow_mut().row_widget_factory = Some(Box::new(f));
    }

    /// Set the cell source for non-tree column content.
    ///
    /// Cells are resolved into the shared substrate cache and read at paint.
    /// `TreeViewBody::layout` realizes cells for structural passes; callers may
    /// use `refresh_visible_cells()` for value-only refreshes.
    ///
    /// SPEC-HAL-188 (CellReader contract). Pass `None` to clear (data columns
    /// paint empty).
    pub fn set_cell_source(&mut self, source: Option<Rc<RefCell<dyn CellSource>>>) {
        let mut core = self.core.borrow_mut();
        core.cms.set_cell_source(source.clone());
        core.cell_source = source;
    }

    /// Refresh realized cell values after the cell source changes.
    ///
    /// This keeps high-frequency value changes on the paint path when topology,
    /// column geometry, and row heights are unchanged.
    pub fn refresh_visible_cells(&mut self) {
        let mut core = self.core.borrow_mut();
        if core.cms.hts().try_column_layout().is_none() {
            return;
        }
        let (visible, _total_count, _engine_first) = core.cms.hts_mut().visible_rows_with_total();
        core.cms.realize_cell_values(&visible);
    }

    #[cfg(test)]
    pub(crate) fn test_column_probe_entries(&self) -> Vec<TreeViewColumnProbeEntry> {
        let core = self.core.borrow();
        let columns: Vec<_> = core
            .cms
            .hts()
            .column_model()
            .columns()
            .iter()
            .cloned()
            .collect();
        let slots: Vec<_> = core
            .cms
            .hts()
            .try_column_layout()
            .map(|layout| layout.body_slots().collect())
            .unwrap_or_default();
        let horizontal_scroll = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |link| link.get());

        columns
            .into_iter()
            .enumerate()
            .map(|(display_index, column)| {
                let slot = slots
                    .iter()
                    .find(|slot| slot.column_id == column.id)
                    .copied();
                let separator_x_content = slot.map(|slot| slot.x + slot.width - 1.0);
                let separator_x_viewport = slot.map(|slot| {
                    if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                        slot.x + slot.width - horizontal_scroll - 1.0
                    } else {
                        slot.x + slot.width - 1.0
                    }
                });

                TreeViewColumnProbeEntry {
                    column_id: column.id,
                    display_index,
                    visible: column.visible,
                    resizable: column.resizable,
                    reorderable: column.reorderable,
                    scroll_domain: column.scroll_domain,
                    preferred_width: column.preferred_width,
                    resolved_x_offset: slot.map(|slot| slot.x),
                    resolved_width: slot.map(|slot| slot.width),
                    min_width: column.min_width,
                    max_width: column.max_width.is_finite().then_some(column.max_width),
                    separator_x_content,
                    separator_x_viewport,
                }
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_structural_probe_entry(&self) -> TreeViewStructuralProbeEntry {
        let core = self.core.borrow();
        let horizontal_scroll_offset = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |link| link.get());
        let vertical_scroll_offset = core.cms.hts().scroll_offset() as f32;
        let root_rect = RectF::new(
            0.0,
            0.0,
            self.body_width,
            self.header_height + self.body_height + self.footer_height,
        );
        let header_rect = RectF::new(0.0, 0.0, self.body_width, self.header_height);
        let body_rect = RectF::new(0.0, self.header_height, self.body_width, self.body_height);
        let footer_rect = RectF::new(
            0.0,
            self.header_height + self.body_height,
            self.body_width,
            self.footer_height,
        );
        let mut selected_node_ids: Vec<u64> =
            core.selection.selected_keys.iter().copied().collect();
        selected_node_ids.sort_unstable();

        let mut expanded_node_ids = Vec::new();
        {
            let hts = core.cms.hts();
            let topology = hts.topology();
            for row_index in 0..topology.node_count() {
                if topology.is_expanded(row_index) {
                    expanded_node_ids.push(topology.node_id(row_index));
                }
            }
        }

        TreeViewStructuralProbeEntry {
            widget_id: self.id.0,
            root_rect,
            header_rect,
            body_rect,
            footer_rect,
            horizontal_scroll_offset,
            vertical_scroll_offset,
            resize_state: match self.resize_state {
                ResizeState::Idle => "idle",
                ResizeState::Armed { .. } => "armed",
                ResizeState::Dragging { .. } => "dragging",
            },
            active_resize_separator: core.active_resize_separator,
            hovered_separator: core.hovered_separator,
            pointer_in_expansion_region: false,
            selected_node_ids,
            focused_node: core.selection.focus_key,
            expanded_node_ids,
        }
    }

    // =========================================================================
    // Column resize internals
    // =========================================================================

    /// Hit-test header separators. Returns column whose right-edge separator is near `local_x`.
    ///
    /// `local_x` is viewport-local (pe.global_pos.0 - rect.x).
    /// Scrollable columns convert to content-local by adding scroll_x.
    /// Skips last column (no separator after rightmost column).
    fn hit_test_separator(&self, local_x: f32) -> Option<ResizeSeparatorHit> {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let slots: Vec<_> = core.cms.hts().try_column_layout()?.header_slots().collect();
        drop(core);

        if slots.is_empty() {
            return None;
        }

        for pair in slots.windows(2) {
            let left = pair[0];
            let sep_x = left.x + left.width - 1.0;
            let test_x = if left.scroll_domain == ColumnScrollDomain::Scrollable {
                local_x + scroll_x
            } else {
                local_x
            };
            if (test_x - sep_x).abs() <= SEPARATOR_HIT_HALF_WIDTH {
                return Some(ResizeSeparatorHit {
                    column_id: left.column_id,
                });
            }
        }
        None
    }

    fn separator_target_resizable(&self, hit: ResizeSeparatorHit) -> bool {
        let core = self.core.borrow();
        let hts = core.cms.hts();
        let model = hts.column_model();
        model.get(hit.column_id).map_or(true, |c| c.resizable)
    }

    /// Get resolved width for a column.
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
    /// CLEARS-ELASTIC, set preferred_width); TreeView proposes the width only.
    fn apply_column_width(&mut self, column_id: ColumnId, width: f32) {
        self.core
            .borrow_mut()
            .cms
            .hts_mut()
            .commit_user_resize(column_id, width);
    }

    /// Finalize resize: clear all resize state (commit path — width stays).
    fn finalize_resize(&mut self) {
        self.resize_state = ResizeState::Idle;
        let mut core = self.core.borrow_mut();
        core.active_resize_separator = None;
        core.hovered_separator = None;
        // INV-RESIZE-HSCROLL-2: Unfreeze horizontal scroll when resize ends.
        if let Some(ref freeze) = core.horizontal_scroll_freeze {
            freeze.set(false);
        }
    }

    /// Cancel resize and rollback to original width (mirrors ListView::cancel_resize).
    ///
    /// INV-RESIZE-CLEAN-EXIT: On cancel, state is fully reset.
    /// INV-RESIZE-DETERMINISTIC: Rollback restores initial width exactly.
    fn cancel_resize(&mut self) {
        if let ResizeState::Dragging {
            column_id,
            initial_width,
            ..
        } = self.resize_state
        {
            self.apply_column_width(column_id, initial_width);
        }
        self.finalize_resize();
    }

    /// Compute auto-size width for a column based on visible content.
    ///
    /// INV-AUTOSIZE-VISIBLE: Only measures visible rows (no full scan).
    /// INV-AUTOSIZE-HEADER: Includes header text width.
    /// INV-AUTOSIZE-SAME-PATH: Uses same apply_column_width path as drag.
    ///
    /// For the tree column: measures indentation + icon + label text.
    /// For non-tree columns: header text only (no cell data in current model).
    fn compute_auto_size_width(
        &self,
        column_id: ColumnId,
        visible_rows: &[VisibleRow],
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) -> f32 {
        let core = self.core.borrow();

        // Start with header text width.
        let mut max_width: f32 = 0.0;
        if let Some(title) = core.column_titles.get(&column_id) {
            let run = text_engine.shape(title, self.font_size);
            max_width = run.width;
        }

        // Tree column: measure visible rows (indentation + icon + label).
        let is_tree_col = core.tree_column_id == Some(column_id);
        let row_height = core.cms.hts().row_height();
        drop(core);

        if is_tree_col {
            let icon_size = (row_height * 0.7).floor().clamp(12.0, 18.0);
            let icon_gap = 3.0_f32;

            for vr in visible_rows {
                if vr.label.is_empty() {
                    continue;
                }
                let indent = vr.depth as f32 * TREE_INDENT_STEP;
                let prefix = indent + TREE_INDENT_STEP + icon_size + icon_gap;
                let run = text_engine.shape(&vr.label, self.font_size);
                max_width = max_width.max(prefix + run.width);
            }
        }

        // Add padding on both sides.
        max_width + self.text_padding * 2.0
    }

    // =========================================================================
    // Public Selection API
    // =========================================================================

    /// Access selection state (immutable).
    pub fn selection_state(&self) -> std::cell::Ref<'_, OrderedSelectionState<u64>> {
        std::cell::Ref::map(self.core.borrow(), |c| &c.selection)
    }

    /// Access selection state (mutable).
    pub fn selection_state_mut(&self) -> std::cell::RefMut<'_, OrderedSelectionState<u64>> {
        std::cell::RefMut::map(self.core.borrow_mut(), |c| &mut c.selection)
    }

    /// Clear all selection.
    pub fn clear_selection(&self) {
        let mut core = self.core.borrow_mut();
        let core_ref = &mut *core;
        reduce_ordered_selection(
            &mut core_ref.selection,
            OrderedSelectionAction::ClearSelection,
            &*core_ref.cms.hts(),
        );
    }

    /// Select all visible rows.
    pub fn select_all(&self) {
        let mut core = self.core.borrow_mut();
        let core_ref = &mut *core;
        reduce_ordered_selection(
            &mut core_ref.selection,
            OrderedSelectionAction::SelectAll,
            &*core_ref.cms.hts(),
        );
    }

    // =========================================================================
    // Keyboard Handling
    // =========================================================================

    fn handle_keyboard(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !event.is_down {
            return false;
        }

        if event.key_code == KeyCode::Enter {
            if event.is_repeat {
                return false;
            }
            let node_id = {
                let core = self.core.borrow();
                if !core.focused || core.activation_callback.is_none() {
                    return false;
                }
                let focus = core.selection.focus_key;
                let visible = core.cms.hts_mut().visible_rows();
                visible
                    .iter()
                    .find(|row| Some(row.node_id) == focus && !row.has_children)
                    .map(|row| row.node_id)
            };
            return node_id.is_some_and(|id| emit_activation(&self.core, id));
        }

        // Escape: cancel resize if active (rollback), otherwise clear selection.
        if event.key_code == KeyCode::Escape {
            if !matches!(self.resize_state, ResizeState::Idle) {
                if matches!(self.resize_state, ResizeState::Dragging { .. }) {
                    cx.release_pointer();
                }
                self.cancel_resize();
                cx.invalidate_layout();
                return true;
            }
            let mut core = self.core.borrow_mut();
            let core_ref = &mut *core;
            reduce_ordered_selection(
                &mut core_ref.selection,
                OrderedSelectionAction::ClearSelection,
                &*core_ref.cms.hts(),
            );
            drop(core);
            cx.invalidate_layout();
            return true;
        }

        // Cmd/Ctrl + Right: recursively expand subtree (deferred, INV-337-M1).
        // Consumes whenever a focused row exists; the substrate decides
        // whether anything changes (documented delta, this task).
        if (event.key_code == KeyCode::Right || event.key_code == KeyCode::Character('l'))
            && (event.modifiers.ctrl || event.modifiers.meta)
        {
            let mut core = self.core.borrow_mut();
            if let Some(focus_id) = core.selection.focus_key {
                core.topology_intents
                    .push(TreeTopologyIntent::ExpandSubtree { node_id: focus_id });
                let body_id = core.body_widget_id;
                drop(core);
                cx.invalidate_layout();
                if let Some(bid) = body_id {
                    cx.invalidate_layout_by_id(bid);
                }
                return true;
            }
            drop(core);
            return false;
        }

        // Right arrow: expand or descend into first child.
        if event.key_code == KeyCode::Right || event.key_code == KeyCode::Character('l') {
            let mut core = self.core.borrow_mut();
            let visible = core.cms.hts_mut().visible_rows();
            let vi = visible
                .iter()
                .position(|vr| Some(vr.node_id) == core.selection.focus_key);
            if let Some(vi) = vi {
                let vr = &visible[vi];
                let node_id = vr.node_id;
                if vr.has_children && !vr.expanded {
                    // Expand collapsed parent (deferred, INV-337-M1).
                    core.topology_intents
                        .push(TreeTopologyIntent::Expand { node_id });
                    let body_id = core.body_widget_id;
                    drop(core);
                    cx.invalidate_layout();
                    if let Some(bid) = body_id {
                        cx.invalidate_layout_by_id(bid);
                    }
                    return true;
                } else if vr.has_children && vr.expanded && vi + 1 < visible.len() {
                    // Descend to first child.
                    let child_id = visible[vi + 1].node_id;
                    let action = if event.modifiers.shift {
                        OrderedSelectionAction::ExtendTo { key: child_id }
                    } else {
                        OrderedSelectionAction::FocusTo { key: child_id }
                    };
                    let core_ref = &mut *core;
                    reduce_ordered_selection(&mut core_ref.selection, action, &*core_ref.cms.hts());
                    request_scroll_to_focus(core_ref);
                    drop(core);
                    cx.invalidate_layout();
                    return true;
                }
            }
            drop(core);
            return false;
        }

        // Cmd/Ctrl + Left: recursively collapse subtree (deferred, INV-337-M1).
        // Consumes whenever a focused row exists; the substrate decides
        // whether anything changes (documented delta, this task).
        if (event.key_code == KeyCode::Left || event.key_code == KeyCode::Character('h'))
            && (event.modifiers.ctrl || event.modifiers.meta)
        {
            let mut core = self.core.borrow_mut();
            if let Some(focus_id) = core.selection.focus_key {
                core.topology_intents
                    .push(TreeTopologyIntent::CollapseSubtree { node_id: focus_id });
                let body_id = core.body_widget_id;
                drop(core);
                cx.invalidate_layout();
                if let Some(bid) = body_id {
                    cx.invalidate_layout_by_id(bid);
                }
                return true;
            }
            drop(core);
            return false;
        }

        // Left arrow: collapse or ascend to parent.
        if event.key_code == KeyCode::Left || event.key_code == KeyCode::Character('h') {
            let mut core = self.core.borrow_mut();
            let visible = core.cms.hts_mut().visible_rows();
            let vi = visible
                .iter()
                .position(|vr| Some(vr.node_id) == core.selection.focus_key);
            if let Some(vi) = vi {
                let vr = &visible[vi];
                let node_id = vr.node_id;
                if vr.expanded {
                    // Collapse expanded node (deferred, INV-337-M1).
                    core.topology_intents
                        .push(TreeTopologyIntent::Collapse { node_id });
                    let body_id = core.body_widget_id;
                    drop(core);
                    cx.invalidate_layout();
                    if let Some(bid) = body_id {
                        cx.invalidate_layout_by_id(bid);
                    }
                    return true;
                } else if vr.depth > 0 {
                    // Ascend to parent: scan backward for nearest row with depth == depth - 1.
                    let target_depth = vr.depth - 1;
                    if let Some(parent_vi) =
                        (0..vi).rev().find(|&j| visible[j].depth == target_depth)
                    {
                        let parent_id = visible[parent_vi].node_id;
                        let action = if event.modifiers.shift {
                            OrderedSelectionAction::ExtendTo { key: parent_id }
                        } else {
                            OrderedSelectionAction::FocusTo { key: parent_id }
                        };
                        let core_ref = &mut *core;
                        reduce_ordered_selection(
                            &mut core_ref.selection,
                            action,
                            &*core_ref.cms.hts(),
                        );
                        request_scroll_to_focus(core_ref);
                        drop(core);
                        cx.invalidate_layout();
                        return true;
                    }
                }
            }
            drop(core);
            return false;
        }

        // Page size from cached body_height.
        let page_size = {
            let core = self.core.borrow();
            let rh = core.cms.hts().row_height();
            if rh > 0.0 && self.body_height > 0.0 {
                (self.body_height / rh).floor().max(1.0) as isize
            } else {
                10
            }
        };

        let action = match event.key_code {
            KeyCode::Up if event.modifiers.shift => OrderedSelectionAction::ExtendPrevious,
            KeyCode::Up if event.modifiers.ctrl || event.modifiers.meta => {
                OrderedSelectionAction::MoveFocusPrevious
            }
            KeyCode::Up => OrderedSelectionAction::FocusPrevious,

            KeyCode::Down if event.modifiers.shift => OrderedSelectionAction::ExtendNext,
            KeyCode::Down if event.modifiers.ctrl || event.modifiers.meta => {
                OrderedSelectionAction::MoveFocusNext
            }
            KeyCode::Down => OrderedSelectionAction::FocusNext,

            KeyCode::Home if event.modifiers.shift => OrderedSelectionAction::ExtendFirst,
            KeyCode::Home if event.modifiers.ctrl || event.modifiers.meta => {
                OrderedSelectionAction::MoveFocusFirst
            }
            KeyCode::Home => OrderedSelectionAction::FocusFirst,

            KeyCode::End if event.modifiers.shift => OrderedSelectionAction::ExtendLast,
            KeyCode::End if event.modifiers.ctrl || event.modifiers.meta => {
                OrderedSelectionAction::MoveFocusLast
            }
            KeyCode::End => OrderedSelectionAction::FocusLast,

            KeyCode::PageUp if event.modifiers.shift => {
                OrderedSelectionAction::ExtendPage { delta: -page_size }
            }
            KeyCode::PageUp => OrderedSelectionAction::FocusPage { delta: -page_size },

            KeyCode::PageDown if event.modifiers.shift => {
                OrderedSelectionAction::ExtendPage { delta: page_size }
            }
            KeyCode::PageDown => OrderedSelectionAction::FocusPage { delta: page_size },

            KeyCode::Space => OrderedSelectionAction::ToggleFocused,

            // SPEC-HAL-176: j/k mirror Down/Up for vim navigation.
            KeyCode::Character('j') if event.modifiers.shift => OrderedSelectionAction::ExtendNext,
            KeyCode::Character('j') if event.modifiers.ctrl || event.modifiers.meta => {
                OrderedSelectionAction::MoveFocusNext
            }
            KeyCode::Character('j') => OrderedSelectionAction::FocusNext,

            KeyCode::Character('k') if event.modifiers.shift => {
                OrderedSelectionAction::ExtendPrevious
            }
            KeyCode::Character('k') if event.modifiers.ctrl || event.modifiers.meta => {
                OrderedSelectionAction::MoveFocusPrevious
            }
            KeyCode::Character('k') => OrderedSelectionAction::FocusPrevious,

            KeyCode::Character('a') if event.modifiers.ctrl || event.modifiers.meta => {
                OrderedSelectionAction::SelectAll
            }

            _ => return false,
        };

        let mut core = self.core.borrow_mut();
        let core_ref = &mut *core;
        reduce_ordered_selection(&mut core_ref.selection, action, &*core_ref.cms.hts());
        request_scroll_to_focus(core_ref);
        drop(core);
        cx.invalidate_layout();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hal371_treeview_focus_owned_node_activation_callback_borrows() {
        let mut view = TreeView::new();
        let weak = Rc::downgrade(&view.core);
        let output = Rc::new(Cell::new(None));
        let seen = Rc::clone(&output);
        view.set_activation_callback(move |id| {
            let core = weak.upgrade().unwrap();
            let core = core.borrow();
            let _ = core.selection.focus_key;
            let _ = core.cms.hts().revision();
            seen.set(Some(id));
        });
        // This is the borrow-release proof only. Public delivery is exercised by
        // the mounted, qualified raw-input tests in treeview_smoke.
        assert!(emit_activation(&view.core, 42));
        assert_eq!(output.get(), Some(42));
        assert!(view.core.borrow().activation_callback.is_some());
    }

    #[test]
    fn tree_view_horizontal_scroll_freeze_uses_shared_core_flag() {
        let tree_view = TreeView::new();
        let freeze = tree_view.horizontal_scroll_freeze();

        assert!(!freeze.get());
        freeze.set(true);
        assert!(
            tree_view
                .core
                .borrow()
                .horizontal_scroll_freeze
                .as_ref()
                .is_some_and(|core_freeze| core_freeze.get())
        );
    }

    #[test]
    fn tree_view_finalize_resize_clears_horizontal_scroll_freeze() {
        let mut tree_view = TreeView::new();
        let freeze = tree_view.horizontal_scroll_freeze();

        freeze.set(true);
        tree_view.finalize_resize();

        assert!(!freeze.get());
    }
}

impl Default for TreeView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for TreeView {
    fn update(&mut self, _dt: f32, _cx: &mut UpdateContext) {
        // SPEC-HAL-337 §4.1 / INV-337-M1: the per-frame application hook.
        // Event handlers enqueued topology intents and invalidated layout;
        // apply them here so this frame's layout sees the new topology.
        let mut core = self.core.borrow_mut();
        if core.topology_intents.is_empty() {
            return;
        }
        let intents = std::mem::take(&mut core.topology_intents);
        let rev_before = core.cms.hts().revision();
        for intent in intents {
            match intent {
                TreeTopologyIntent::Toggle { node_id } => {
                    core.cms.hts_mut().toggle_by_id(node_id);
                }
                TreeTopologyIntent::Expand { node_id } => {
                    core.cms.hts_mut().expand_by_id(node_id);
                }
                TreeTopologyIntent::Collapse { node_id } => {
                    core.cms.hts_mut().collapse_by_id(node_id);
                }
                TreeTopologyIntent::ExpandSubtree { node_id } => {
                    core.cms.hts_mut().expand_subtree_by_id(node_id);
                }
                TreeTopologyIntent::CollapseSubtree { node_id } => {
                    core.cms.hts_mut().collapse_subtree_by_id(node_id);
                }
            }
        }
        // Choreography once per batch, only when the topology actually
        // changed — revision() bumps on any state mutation, so the gate
        // preserves the pre-funnel no-change-no-scroll behavior exactly.
        if core.cms.hts().revision() != rev_before {
            let fresh = core.cms.hts_mut().visible_rows();
            ensure_focus_visible(&mut core, &fresh);
            request_scroll_to_focus(&mut core);
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
        let body_h = (h - header_h - self.footer_height).max(0.0);
        self.body_width = w;
        self.body_height = body_h;

        // Cache theme values for event-path use (auto-size measurement).
        self.font_size = cx.theme.typography.font_size;
        self.text_padding = cx.theme.list_geom.padding_x;

        // Resolve column geometry at composite level — before child layout.
        // Body (TreeViewBody) is a pure consumer; resolution happens exactly once here.
        let sf = self.core.borrow().scale_factor;
        self.core.borrow_mut().cms.hts_mut().resolve_geometry(w, sf);

        // Delegate to ScrollArea child (children[0]), positioned below header.
        if !children.is_empty() {
            let c = Constraints::new(w, w, body_h, body_h);
            cx.layout_child(children[0], c);
            cx.set_child_rect(children[0], RectF::new(0.0, header_h, w, body_h));
        }

        // Layout footer (children[1]) below body.
        if children.len() > 1 {
            let fc = Constraints::new(w, w, self.footer_height, self.footer_height);
            cx.layout_child(children[1], fc);
            cx.set_child_rect(
                children[1],
                RectF::new(0.0, header_h + body_h, w, self.footer_height),
            );
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
        let theme = cx.theme;
        let header_h = self.header_height;
        self.core.borrow_mut().scale_factor = cx.scene.scale_factor();

        if !self.header_visible() {
            return;
        }

        // --- Header ---
        let header_rect = RectF::new(rect.x, rect.y, rect.width, header_h);
        cx.scene.push_rect(
            header_rect,
            theme.colors.list.header_background,
            LAYER_CONTENT,
            None,
        );

        // Collect column slots, titles, scroll_x, and separator state in a single borrow.
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
        let text_padding = theme.list_geom.padding_x;
        let header_text_color = theme.colors.typography.secondary;
        let separator_color = theme.colors.borders.default;

        // Clip header content (matches ListView header clip).
        cx.scene.begin_clip(header_rect, None);
        let sf = cx.scene.scale_factor();
        let snapped_x = snap_to_physical(rect.x, sf);

        let slot_count = slots.len();
        for (i, (slot, title)) in slots.iter().zip(titles.iter()).enumerate() {
            // Scroll-domain-aware X: subtract scroll_x for scrollable columns.
            let visual_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                snapped_x + slot.x - scroll_x
            } else {
                snapped_x + slot.x
            };

            // Per-slot separator X: snap in content space, subtract scroll raw.
            let sep_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                snap_to_physical(snapped_x + slot.x + slot.width - 1.0, sf) - scroll_x
            } else {
                snap_to_physical(snapped_x + slot.x + slot.width - 1.0, sf)
            };

            // Per-slot clip: constrain title text to column boundary.
            // Clip right edge = separator X, ensuring consistent rounding.
            let cell_clip_w = (sep_x - visual_x).max(0.0);
            cx.scene
                .begin_clip(RectF::new(visual_x, rect.y, cell_clip_w, header_h), None);

            // Column title text.
            if let Some(title) = title {
                let run = cx.text_engine.shape(title, font_size);
                let text_y = rect.y + (header_h - run.height) / 2.0;
                let text_x = visual_x + text_padding;
                cx.text_engine.draw(
                    &run,
                    text_x,
                    text_y,
                    header_text_color,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
            }

            cx.scene.end_clip();

            // Column separator (right edge, outside per-slot text clip). Skip after last column.
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

        // Header bottom border (outside clip, full width — matches ListView).
        let sf = cx.scene.scale_factor();
        let border_y = snap_to_physical(rect.y + header_h - 1.0, sf);
        cx.scene.push_rect(
            RectF::new(rect.x, border_y, rect.width, 1.0),
            separator_color,
            LAYER_CONTENT + 1,
            None,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if let Event::Key(ke) = event {
            // Key events dispatched to focused widget in Target phase only (no Bubble).
            if phase == EventPhase::Target {
                return self.handle_keyboard(cx, ke);
            }
            return false;
        }

        // SPEC-HAL-176: TextInput is reserved for composed text destined for
        // focused input controls (TextField, TextArea). TreeView is not an
        // input control. Every character keypress (in both Navigation and
        // Capture modes) is delivered via the paired Event::Key dispatched
        // from route_input_event's KeyDown branch — see platform/window.rs
        // Step 3 (lines 423-437) which emits the Key event for any
        // non-clipboard character keypress regardless of mode, and
        // core/app.rs:2606-2614 which restates SPEC-HAL-176.
        //
        // The previous synthesize-KeyEvent-from-Text path in this block
        // caused double-dispatch: each printable keypress reached
        // handle_keyboard once via the real Key event AND once via the
        // synthetic Key reconstructed from the paired TextInput. handle_keyboard
        // already dispatches Character('j')/('k')/('h')/('l') and Cmd/Shift
        // variants on the Key path (lines 586-625, 652-697, 747-760), so the
        // synthetic path was pure duplication. Ignoring Event::Text here closes
        // the duplicate path.
        if let Event::Text(_) = event {
            return false;
        }

        let Event::Pointer(pe) = event else {
            return false;
        };

        match pe.kind {
            // =================================================================
            // PointerMove: hover detection (Idle) or resize tracking (Armed/Dragging)
            // =================================================================
            PointerEventKind::Move => match self.resize_state {
                ResizeState::Idle => {
                    // INV-SEPARATOR-HITTEST-EXCLUDES-SCROLLBAR: Only process hover in
                    // Target/Bubble phases. Capture-phase events may be destined for
                    // ScrollArea's horizontal scrollbar (overlay hit region).
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
                    let hit_sep = self.hit_test_separator(local_x);
                    let hit_sep = hit_sep.filter(|hit| self.separator_target_resizable(*hit));
                    self.core.borrow_mut().hovered_separator = hit_sep.map(|hit| hit.column_id);

                    if hit_sep.is_some() {
                        cx.request_cursor(CursorIcon::ColResize);
                    }
                    false
                }
                ResizeState::Armed {
                    column_id,
                    start_x,
                    initial_width,
                } => {
                    // First move after arm → transition to Dragging + capture.
                    self.resize_state = ResizeState::Dragging {
                        column_id,
                        start_x,
                        initial_width,
                    };
                    cx.capture_pointer(PointerButton::Primary);

                    let delta_x = pe.global_pos.0 - start_x;
                    self.apply_column_width(column_id, initial_width + delta_x);
                    cx.invalidate_layout();
                    if let Some(body_id) = self.core.borrow().body_widget_id {
                        cx.invalidate_layout_by_id(body_id);
                    }
                    cx.request_continuous_frames();
                    cx.request_cursor(CursorIcon::ColResize);
                    true
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
                    self.apply_column_width(column_id, initial_width + delta_x);
                    cx.invalidate_layout();
                    if let Some(body_id) = self.core.borrow().body_widget_id {
                        cx.invalidate_layout_by_id(body_id);
                    }
                    cx.request_continuous_frames();
                    cx.request_cursor(CursorIcon::ColResize);
                    true
                }
            },

            // =================================================================
            // PointerDown: arm resize if on separator
            // =================================================================
            PointerEventKind::Down => {
                if pe.button != Some(PointerButton::Primary) {
                    return false;
                }
                if !matches!(self.resize_state, ResizeState::Idle) {
                    return false;
                }

                // INV-SEPARATOR-HITTEST-EXCLUDES-SCROLLBAR: Only arm resize in
                // Target/Bubble phases (matches ListView). Capture-phase events
                // may be destined for ScrollArea's scrollbar overlay.
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

                let Some(hit) = self.hit_test_separator(local_x) else {
                    return false;
                };
                if !self.separator_target_resizable(hit) {
                    return false;
                }

                let Some(initial_width) = self.get_column_width(hit.column_id) else {
                    return false;
                };
                self.resize_state = ResizeState::Armed {
                    column_id: hit.column_id,
                    start_x: pe.global_pos.0,
                    initial_width,
                };
                {
                    let mut core = self.core.borrow_mut();
                    core.active_resize_separator = Some(hit.column_id);
                    if let Some(ref freeze) = core.horizontal_scroll_freeze {
                        freeze.set(true);
                    }
                }
                cx.request_cursor(CursorIcon::ColResize);
                true
            }

            // =================================================================
            // PointerUp: finalize resize
            // =================================================================
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
                        false
                    }
                    ResizeState::Armed { .. } => {
                        if phase == EventPhase::Target || phase == EventPhase::Bubble {
                            self.finalize_resize();
                            return true;
                        }
                        false
                    }
                    ResizeState::Idle => false,
                }
            }

            // =================================================================
            // DoubleClick: Auto-size column to content
            //
            // INV-AUTOSIZE-VISIBLE: Only measures visible rows.
            // INV-AUTOSIZE-SAME-PATH: Uses same apply_column_width path as drag.
            // =================================================================
            PointerEventKind::DoubleClick if pe.button == Some(PointerButton::Primary) => {
                // Only auto-size from Idle or Armed state (not during drag).
                if matches!(self.resize_state, ResizeState::Dragging { .. }) {
                    return false;
                }

                // INV-SEPARATOR-HITTEST-EXCLUDES-SCROLLBAR: Only autosize in
                // Target/Bubble phases (matches ListView).
                if phase != EventPhase::Target && phase != EventPhase::Bubble {
                    return false;
                }

                let Some(rect) = cx.widget_global_rect() else {
                    return false;
                };

                let local_x = pe.global_pos.0 - rect.x;
                let hit_sep = self.hit_test_separator(local_x);

                if let Some(hit) = hit_sep {
                    let column_id = hit.column_id;
                    // Get visible rows for measurement.
                    let visible_rows = {
                        let core = self.core.borrow_mut();
                        core.cms.hts_mut().visible_rows()
                    };

                    // Compute auto-size width from visible content.
                    let auto_width =
                        self.compute_auto_size_width(column_id, &visible_rows, cx.text_engine());

                    // Apply via same path as drag resize.
                    self.apply_column_width(column_id, auto_width);

                    // Clear any armed state (double-click may arrive after single-click armed).
                    self.finalize_resize();

                    cx.invalidate_layout();
                    if let Some(body_id) = self.core.borrow().body_widget_id {
                        cx.invalidate_layout_by_id(body_id);
                    }
                    return true;
                }

                false
            }

            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "TreeView"
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.core.borrow_mut().focused = true;
        cx.invalidate_layout();
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        let press = {
            let mut core = self.core.borrow_mut();
            core.focused = false;
            core.activation_press.take()
        };
        if let Some(press) = press {
            cx.retire_descendant_pointer_capture(press.pointer, press.body);
        }

        // INV-RESIZE-CLEAN-EXIT: Focus loss cancels resize (rollback, not commit).
        if !matches!(self.resize_state, ResizeState::Idle) {
            if matches!(self.resize_state, ResizeState::Dragging { .. }) {
                cx.release_pointer();
            }
            self.cancel_resize();
        }

        cx.invalidate_layout();
    }

    fn on_unmount(&mut self) {
        self.core.borrow_mut().activation_press = None;
    }
}

// Sealed WidgetDowncast impl (required by Widget trait).
impl crate::core::widget::WidgetDowncast for TreeView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
