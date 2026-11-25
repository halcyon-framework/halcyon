// =============================================================================
// TreeViewBody: Row content widget for TreeView body region
// =============================================================================
//
// This widget paints tree rows and is the content of ScrollArea.
// It shares state with TreeView via Rc<RefCell<TreeViewCore>>.
//
// ScrollArea handles:
// - Scroll state and smooth scrolling
// - Scroll event handling
// - Clipping
// - Scrollbar rendering
//
// TreeViewBody handles:
// - Row painting at layout positions (NO scroll math — ScrollArea does that)
// - Tree guide lines
// - Disclosure triangles (expand/collapse)
// - Row hit-testing and expand/collapse events
//
// Mirrors ListViewBody (src/widgets/list_view/body.rs) structurally.
// =============================================================================

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::core::event::{
    Event, EventPhase, PointerButton, PointerEvent, PointerEventKind, PointerId,
};
use crate::core::icon::IconId;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::ordered_selection::{
    OrderedSelectionAction, OrderedSelectionState, reduce_ordered_selection,
};
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, ScrollSyncInvalidation, Widget,
};
use crate::core::widget_tree::WidgetKey;
use crate::engines::cell_mechanics::CellMechanicsSubstrate;
use crate::engines::cell_source::{CellSource, EditExitReason};
use crate::engines::hierarchical::{FixedWindowStamp, VisibleRow};
use crate::engines::row_topology::TopologyEngine;

use super::tree_view::SEPARATOR_HIT_HALF_WIDTH;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::disclosure_toggle::DisclosureToggle;
use crate::widgets::tabular_view::column::ColumnId;
use crate::widgets::tabular_view::row::{RowId, RowProjection};
use crate::widgets::tabular_view::tree_paint::{self, TreeRenderData};

/// Tree indentation per depth level (pixels).
pub(super) const TREE_INDENT_STEP: f32 = 20.0;

/// Width of tree control zone (disclosure triangle hit area).
pub(super) const TREE_CONTROL_WIDTH: f32 = 16.0;

/// Width of guide lines (pixels).
const GUIDE_LINE_WIDTH: f32 = 1.0;

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeViewRealizedCellProbeEntry {
    pub row_id: u64,
    pub column_id: ColumnId,
    pub rect: RectF,
    pub content_state: Option<String>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeViewRealizedRowProbeEntry {
    pub row_id: u64,
    pub data_index: usize,
    pub visual_index: usize,
    pub rect: RectF,
    pub cells: Vec<TreeViewRealizedCellProbeEntry>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeViewVisibleProjectionProbeEntry {
    pub node_id: u64,
    pub data_index: usize,
    pub visual_index: usize,
    pub depth: u32,
    pub expanded: bool,
    pub has_children: bool,
    pub guide_mask: u64,
    pub stem_down: bool,
    pub label: String,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeViewDisclosureHitZoneProbeEntry {
    pub node_id: u64,
    pub rect: RectF,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TreeViewIndentationGeometryProbeEntry {
    pub node_id: u64,
    pub depth: u32,
    pub indent_x: f32,
    pub control_rect: Option<RectF>,
    pub tree_column_clip: Option<RectF>,
}

// =============================================================================
// Shared state between TreeView (composite) and TreeViewBody (content widget).
// Mirrors ListViewCore pattern (Rc<RefCell> shared ownership).
// =============================================================================

/// SPEC-HAL-337 §4.1 / INV-337-M1: deferred topology intents. Event handlers
/// enqueue; `TreeView::update` (the per-frame application hook) drains and
/// applies before layout. INV-TV-1: HTS stays the sole topology authority —
/// intents name nodes, the substrate decides what changes.
pub(crate) enum TreeTopologyIntent {
    Toggle { node_id: u64 },
    Expand { node_id: u64 },
    Collapse { node_id: u64 },
    ExpandSubtree { node_id: u64 },
    CollapseSubtree { node_id: u64 },
}

/// HAL371: one original native press, never a second selection/focus owner.
#[derive(Clone, Copy)]
pub(super) struct ActivationPress {
    pub(super) pointer: PointerId,
    pub(super) node_id: u64,
    pub(super) topology_revision: u64,
    pub(super) body: TreeWidgetId,
}

pub(crate) struct TreeViewCore {
    pub(super) activation_callback: Option<Box<dyn FnMut(u64)>>,
    pub(super) activation_press: Option<ActivationPress>,
    pub(crate) cms: CellMechanicsSubstrate,
    pub(crate) tree_column_id: Option<ColumnId>,
    pub(crate) column_titles: HashMap<ColumnId, String>,
    /// Selection state (identity-based, survives virtualization). Routes through reducer.
    pub(crate) selection: OrderedSelectionState<u64>,
    /// Drag-select anchor node_id (set on shift+pointer-down, cleared on pointer-up).
    pub(crate) selection_drag_anchor: Option<u64>,
    /// Whether a drag-select gesture is currently active (pointer captured).
    pub(crate) selection_drag_active: bool,
    /// Last node during drag (for change detection).
    pub(crate) selection_drag_last: Option<u64>,
    /// Currently hovered column separator (if any). Set by TreeView, read by body for paint.
    pub(crate) hovered_separator: Option<ColumnId>,
    /// Currently resizing column separator (if any). Active resize visual has priority over hover.
    pub(crate) active_resize_separator: Option<ColumnId>,
    /// Currently hovered data row (node identity). Set by TreeViewBody on pointer
    /// Move, read by paint to draw the row-hover band. Identity-based (survives
    /// virtualization), mirroring `selection`. `None` when the pointer is not over a row.
    pub(crate) hovered_node: Option<u64>,
    /// Horizontal scroll link for header alignment with body scroll.
    pub(crate) horizontal_scroll_link: Option<crate::core::scroll_container::HorizontalScrollLink>,
    /// Horizontal scroll freeze flag for column resize.
    pub(crate) horizontal_scroll_freeze: Option<Rc<Cell<bool>>>,
    /// Whether the parent TreeView currently has keyboard focus.
    /// Set by TreeView::on_focus / on_blur. Paint uses this to choose
    /// row_selected vs row_selected_inactive (mirrors ListView's core.focused).
    pub(crate) focused: bool,
    /// Body widget's TreeWidgetId for layout invalidation.
    ///
    /// Set by TreeViewBody on first event dispatch. Used by TreeView to mark
    /// Body dirty when column widths change (Body must re-resolve geometry).
    pub(crate) body_widget_id: Option<TreeWidgetId>,
    /// Requested scroll Y for focus reveal after keyboard navigation.
    /// Set by TreeView::handle_keyboard, consumed by TreeViewBody::take_requested_scroll_y.
    pub(super) requested_scroll_y: Option<f32>,
    /// Cached scale factor for geometry resolution.
    pub(crate) scale_factor: f32,
    /// Owner configuration changes that a scroll-only refresh cannot apply.
    pub(super) layout_configuration: u64,
    /// SPEC-HAL-313 §3.5: Pending row widget factory.
    /// Set by TreeView::set_row_widget_factory(). Will be taken by TreeViewBody
    /// when HTS variable-height support enables body integration.
    pub(crate) row_widget_factory:
        Option<Box<dyn Fn(RowId) -> Box<dyn crate::core::widget::Widget>>>,
    /// Cell content source for non-tree columns. Read-only at this stage.
    ///
    /// `None` means data columns paint empty (legacy behavior preserved).
    /// `Some` enables per-variant cell rendering from `PropertyValue`.
    ///
    /// SPEC-HAL-188 (CellReader contract). `TreeViewBody` populates a
    /// per-realization `realized_cell_values` cache at end of layout from
    /// this source; paint reads only from the cache (SPEC-PM-016-R1).
    pub(crate) cell_source: Option<Rc<RefCell<dyn CellSource>>>,
    /// SPEC-HAL-337 §4.1 / INV-337-M1: topology intents enqueued during
    /// event routing, drained by `TreeView::update` before layout.
    pub(crate) topology_intents: Vec<TreeTopologyIntent>,
}

/// All qualification borrows must end before consumer code can read the core.
pub(super) fn emit_activation(core: &Rc<RefCell<TreeViewCore>>, node_id: u64) -> bool {
    let callback = core.borrow_mut().activation_callback.take();
    let Some(mut callback) = callback else {
        return false;
    };
    callback(node_id);
    core.borrow_mut()
        .activation_callback
        .get_or_insert(callback);
    true
}

// =============================================================================
// INV-TV-3 / INV-TV-6: O(1) TreeRenderData impl for Vec<VisibleRow>.
// Index parameter is the visible index (position in the Vec), NOT physical row index.
// =============================================================================

impl TreeRenderData for Vec<VisibleRow> {
    fn depth(&self, row_index: usize) -> u32 {
        self[row_index].depth
    }

    fn has_children(&self, row_index: usize) -> bool {
        self[row_index].has_children
    }

    fn guide_mask(&self, row_index: usize) -> u64 {
        self[row_index].guide_mask
    }

    fn stem_down(&self, row_index: usize) -> bool {
        self[row_index].stem_down
    }
}

/// TreeViewBody — row content widget inside ScrollArea.
///
/// Paints tree rows with indentation, disclosure triangles, and guide lines.
/// Handles expand/collapse clicks. Does NOT own scroll state.
///
/// INV-TV-1: HTS is sole topology authority.
/// INV-TV-2: Widget does not store expansion state.
/// INV-TV-3: Paint path O(n_visible).
/// INV-TV-6: No physical row_index search in paint or hit-test.
pub struct TreeViewBody {
    id: WidgetInternalId,
    core: Rc<RefCell<TreeViewCore>>,
    /// Cached visible rows from last layout.
    visible_rows: Vec<VisibleRow>,
    fixed_window: Option<(u64, FixedWindowStamp)>,
    /// Topology version of the body-owned hit geometry; viewport refresh is not
    /// source replacement (the broader HTS revision also counts every layout).
    visible_topology_revision: u64,
    /// Sorted index of the first visible row, computed during layout.
    /// Paint and hit-test read this — never recompute from scroll offset.
    first_sorted: usize,
    /// Viewport height from last layout.
    viewport_height: f32,
    /// Cached content height from last layout (total_count * row_height).
    /// Used by paint for background that covers the full content extent.
    content_height: f32,
    /// Disclosure toggle child widget nodes keyed by stable node_id (INV-UI-IDENTITY-STABLE).
    disclosure_nodes: HashMap<u64, crate::core::widget_tree::NodeId>,
    /// Cached tree-column clip rect (widget-local) for framework child clipping via clip_bounds().
    /// Covers the tree column and a qualified mounted cell editor, if present.
    /// DisclosureToggle geometry is independently admitted wholly inside its column.
    tree_col_clip: Option<RectF>,
    /// Debug-only: paint metadata query counter (INV-TV-3 enforcement).
    #[cfg(debug_assertions)]
    paint_query_count: std::cell::Cell<usize>,
}

impl TreeViewBody {
    pub(crate) fn new(core: Rc<RefCell<TreeViewCore>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            core,
            visible_rows: Vec::new(),
            fixed_window: None,
            visible_topology_revision: 0,
            first_sorted: 0,
            viewport_height: 0.0,
            content_height: 0.0,
            disclosure_nodes: HashMap::new(),
            tree_col_clip: None,
            #[cfg(debug_assertions)]
            paint_query_count: std::cell::Cell::new(0),
        }
    }

    /// Hit-test: viewport-local Y → visible row index.
    ///
    /// `local_y` is viewport-local (pe.global_pos.1 - widget_global_rect.y).
    /// Adds scroll offset to convert to content Y, then maps to sorted index.
    fn hit_test_row(&self, local_y: f32) -> Option<usize> {
        let core = self.core.borrow();
        let row_height = core.cms.hts().row_height();
        // SPEC-PM-017 R1b / INV-PM-017-3: viewport-local → content index in
        // f64, so the small local_y is not absorbed by a quantized f32
        // content-space sum at deep offsets.
        let scroll_offset = core.cms.hts().scroll_offset();
        drop(core);

        if row_height <= 0.0 || local_y < 0.0 || local_y >= self.viewport_height {
            return None;
        }

        let content_y = local_y as f64 + scroll_offset;
        let sorted_index = (content_y / row_height as f64).floor() as usize;

        if sorted_index < self.first_sorted {
            return None;
        }
        let vi = sorted_index - self.first_sorted;
        if vi < self.visible_rows.len() {
            Some(vi)
        } else {
            None
        }
    }

    /// Check if viewport-local X is on a column separator.
    ///
    /// Used to prevent row selection from consuming PointerDown events
    /// that should bubble to TreeView for column resize arming.
    fn is_on_separator(&self, local_x: f32) -> bool {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let hts = core.cms.hts();
        let Some(layout) = hts.try_column_layout() else {
            return false;
        };
        let slots: Vec<_> = layout.header_slots().collect();
        drop(hts);
        drop(core);

        if slots.len() <= 1 {
            return false;
        }

        let geometry_x = local_x + scroll_x;
        slots[..slots.len() - 1].iter().any(|slot| {
            (geometry_x - (slot.x + slot.width - 1.0)).abs() <= SEPARATOR_HIT_HALF_WIDTH
        })
    }

    fn activation_leaf_at(&self, cx: &EventContext, event: &PointerEvent) -> Option<u64> {
        let rect = cx.widget_global_rect()?;
        let x = event.global_pos.0 - rect.x;
        let y = event.global_pos.1 - rect.y;
        if !(x >= 0.0 && x < rect.width && y >= 0.0 && y < self.viewport_height)
            || self.is_on_separator(x)
        {
            return None;
        }
        let row = &self.visible_rows[self.hit_test_row(y)?];
        let core = self.core.borrow();
        let hts = core.cms.hts();
        let topology = hts.topology();
        if row.has_children
            || topology.is_topology_dirty()
            || topology.topology_version() != self.visible_topology_revision
            || topology.index_for_id(row.node_id) != Some(row.row_index)
        {
            return None;
        }
        Some(row.node_id)
    }

    fn handle_activation_press(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let Some(press) = self.core.borrow().activation_press else {
            return false;
        };
        let plain = !event.modifiers.shift
            && !event.modifiers.ctrl
            && !event.modifiers.alt
            && !event.modifiers.meta;
        let same_row = press.pointer == event.id
            && press.body == cx.widget_id
            && self.activation_leaf_at(cx, event) == Some(press.node_id)
            && self.core.borrow().cms.hts().topology().topology_version()
                == press.topology_revision;
        let up = event.kind == PointerEventKind::Up;
        let invalid = !cx.has_pointer_capture()
            || !plain
            || !same_row
            || matches!(
                event.kind,
                PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick
                    | PointerEventKind::DragStart
                    | PointerEventKind::Scroll { .. }
            );
        if up || invalid {
            self.core.borrow_mut().activation_press = None;
            if cx.has_pointer_capture() && press.body == cx.widget_id {
                cx.release_pointer();
            }
            if up && !invalid && event.button == Some(PointerButton::Primary) {
                emit_activation(&self.core, press.node_id);
            }
            return matches!(
                event.kind,
                PointerEventKind::Up
                    | PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick
                    | PointerEventKind::DragStart
            );
        }
        // Multi-click metadata is not another activation; matching release owns it.
        // Keep native hover and editable-cell DoubleClick processing intact.
        false
    }

    fn arm_activation_press(&self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if event.button != Some(PointerButton::Primary)
            || event.modifiers.shift
            || event.modifiers.ctrl
            || event.modifiers.alt
            || event.modifiers.meta
        {
            return false;
        }
        let Some(node_id) = self.activation_leaf_at(cx, event) else {
            return false;
        };
        let mut core = self.core.borrow_mut();
        if core.activation_callback.is_none() || core.activation_press.is_some() {
            return false;
        }
        let topology_revision = core.cms.hts().topology().topology_version();
        core.activation_press = Some(ActivationPress {
            pointer: event.id,
            node_id,
            topology_revision,
            body: cx.widget_id,
        });
        cx.capture_pointer(PointerButton::Primary);
        true
    }

    #[cfg(test)]
    pub(crate) fn test_realized_row_probe_entries(&self) -> Vec<TreeViewRealizedRowProbeEntry> {
        let core = self.core.borrow();
        let row_height = core.cms.hts().row_height();
        let scroll_y = core.cms.hts().scroll_offset();
        let total_width = core.cms.hts().column_layout().total_width();
        let tree_col_id = core.tree_column_id;
        let slots: Vec<_> = core.cms.hts().column_layout().body_slots().collect();

        self.visible_rows
            .iter()
            .enumerate()
            .map(|(visual_index, row)| {
                let y = ((self.first_sorted + visual_index) as f64 * row_height as f64 - scroll_y)
                    as f32;
                let cells = slots
                    .iter()
                    .map(|slot| {
                        let content_state = if tree_col_id == Some(slot.column_id) {
                            Some(row.label.clone())
                        } else {
                            core.cms
                                .realized_cell_value(row.node_id, slot.column_id)
                                .map(|cell| cell.value.display_text())
                        };
                        TreeViewRealizedCellProbeEntry {
                            row_id: row.node_id,
                            column_id: slot.column_id,
                            rect: RectF::new(slot.x, y, slot.width, row_height),
                            content_state,
                        }
                    })
                    .collect();

                TreeViewRealizedRowProbeEntry {
                    row_id: row.node_id,
                    data_index: row.row_index,
                    visual_index,
                    rect: RectF::new(0.0, y, total_width, row_height),
                    cells,
                }
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_visible_projection_probe_entries(
        &self,
    ) -> Vec<TreeViewVisibleProjectionProbeEntry> {
        self.visible_rows
            .iter()
            .enumerate()
            .map(|(visual_index, row)| TreeViewVisibleProjectionProbeEntry {
                node_id: row.node_id,
                data_index: row.row_index,
                visual_index,
                depth: row.depth,
                expanded: row.expanded,
                has_children: row.has_children,
                guide_mask: row.guide_mask,
                stem_down: row.stem_down,
                label: row.label.clone(),
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_disclosure_hit_zone_probe_entries(
        &self,
    ) -> Vec<TreeViewDisclosureHitZoneProbeEntry> {
        self.test_disclosure_geometry()
            .into_iter()
            .filter_map(|(row, control_rect)| {
                control_rect.map(|rect| TreeViewDisclosureHitZoneProbeEntry {
                    node_id: row.node_id,
                    rect,
                })
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_indentation_geometry_probe_entries(
        &self,
    ) -> Vec<TreeViewIndentationGeometryProbeEntry> {
        let tree_column_clip = self.tree_col_clip;
        self.test_disclosure_geometry()
            .into_iter()
            .map(
                |(row, control_rect)| TreeViewIndentationGeometryProbeEntry {
                    node_id: row.node_id,
                    depth: row.depth,
                    indent_x: row.depth as f32 * TREE_INDENT_STEP,
                    control_rect,
                    tree_column_clip,
                },
            )
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_set_hovered_node(&self, hovered_node: Option<u64>) {
        self.core.borrow_mut().hovered_node = hovered_node;
    }

    #[cfg(test)]
    pub(crate) fn test_hovered_node(&self) -> Option<u64> {
        self.core.borrow().hovered_node
    }

    #[cfg(test)]
    fn test_disclosure_geometry(&self) -> Vec<(VisibleRow, Option<RectF>)> {
        let core = self.core.borrow();
        let Some(tree_col_id) = core.tree_column_id else {
            return self
                .visible_rows
                .iter()
                .cloned()
                .map(|row| (row, None))
                .collect();
        };
        let row_height = core.cms.hts().row_height();
        let scroll_y = core.cms.hts().scroll_offset();
        let slots: Vec<_> = core.cms.hts().column_layout().body_slots().collect();
        let Some(tree_slot) = slots.iter().find(|slot| slot.column_id == tree_col_id) else {
            return self
                .visible_rows
                .iter()
                .cloned()
                .map(|row| (row, None))
                .collect();
        };
        let effective_right = tree_slot.x + tree_slot.width - 1.0;

        self.visible_rows
            .iter()
            .enumerate()
            .map(|(visual_index, row)| {
                let indent = row.depth as f32 * TREE_INDENT_STEP;
                let bx = tree_slot.x + indent + (TREE_INDENT_STEP - TREE_CONTROL_WIDTH) * 0.5;
                let by = ((self.first_sorted + visual_index) as f64 * row_height as f64 - scroll_y)
                    as f32;
                let control_rect = (row.has_children && bx + TREE_CONTROL_WIDTH <= effective_right)
                    .then_some(RectF::new(bx, by, TREE_CONTROL_WIDTH, row_height));
                (row.clone(), control_rect)
            })
            .collect()
    }
}

impl Widget for TreeViewBody {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[crate::core::widget_tree::NodeId],
    ) -> LayoutResult {
        let available_width = constraints.max_width;
        let available_height = constraints.max_height;
        self.fixed_window = None;

        let core = self.core.borrow_mut();
        core.cms.hts_mut().set_viewport_height(available_height);

        // Single engine call — topology evaluated once, returns visible slice,
        // total count, and the engine-computed first sorted index
        // (SPEC-PM-017 / SPEC-PM-009: single source — no f32 re-derivation
        // from scroll).
        let (visible, total_count, engine_first) = core.cms.hts_mut().visible_rows_with_total();
        let row_height = core.cms.hts().row_height();
        let content_width = core.cms.hts().column_layout().total_width();
        // SPEC-PM-017 R1b: f64 scroll for viewport-relative child geometry.
        let scroll_y = core.cms.hts().scroll_offset();
        let topology_revision = core.cms.hts().topology().topology_version();
        drop(core);

        self.visible_rows = visible;
        self.visible_topology_revision = topology_revision;
        self.first_sorted = engine_first;
        self.viewport_height = available_height;
        // SPEC-PM-017 R1b: single f64 product, narrowed once for the
        // (rung-3 f32) scroll domain.
        let content_height = (total_count as f64 * row_height as f64) as f32;
        self.content_height = content_height;

        // Drain editor-exit outcomes from the substrate queue. Commit (or
        // CommitAndAdvance) writes the new value back through the cell source;
        // Cancel / ForcedCancel discard. Any drained outcome ends the session,
        // so the reconcile below unmounts the editor this pass. Runs before
        // realize so a committed value is reflected in this pass's cache.
        let outcomes = self.core.borrow_mut().cms.drain_edit_outcomes();
        if !outcomes.is_empty() {
            let source = self.core.borrow().cell_source.clone();
            for outcome in outcomes {
                if matches!(
                    outcome.reason,
                    EditExitReason::Commit | EditExitReason::CommitAndAdvance { .. }
                ) {
                    if let (Some(source), Some(value)) = (source.as_ref(), outcome.value) {
                        source
                            .borrow_mut()
                            .commit_cell(outcome.row, outcome.col, value);
                    }
                }
            }
            self.core.borrow_mut().cms.set_edit_session(None);
        }

        // Realize cell values into the CMS cache for the current visible
        // window. The CMS-owned realized truth is what the editor mount reads
        // for its initial value; CellSource is queried once per visible
        // non-tree cell here, never at paint or mount. No-op when no source.
        self.core
            .borrow_mut()
            .cms
            .realize_cell_values(&self.visible_rows);

        // --- Child widget reconciliation (INV-UI-IDENTITY-STABLE) ---
        let self_node_id = cx
            .current_node()
            .expect("TreeViewBody must have current_node during layout");

        // Mount / reposition / unmount the active edit-session editor. The
        // editor is built by the CMS-registered factory and reads its initial
        // value from the realized-cell cache populated above; on exit it pushes
        // an EditOutcome onto the substrate queue. No session and no mounted
        // editor -> no-op.
        self.core.borrow_mut().cms.reconcile_edit_session_owned(
            cx,
            self_node_id,
            &self.visible_rows,
        );

        // --- Disclosure toggle reconciliation ---
        let has_tree_col = self.core.borrow().tree_column_id.is_some();

        if has_tree_col {
            // Tree column geometry for disclosure positioning and overflow check.
            // Uses body_slots() — same resolved layout data path as paint + separator bands.
            let (tree_slot_x, tree_slot_width) = {
                let core = self.core.borrow();
                let tree_col_id = core.tree_column_id.unwrap();
                let hts = core.cms.hts();
                let layout = hts.column_layout();
                let slots: Vec<_> = layout.body_slots().collect();
                let tree_slot = slots
                    .iter()
                    .find(|s| s.column_id == tree_col_id)
                    .expect("tree column must exist in body_slots");
                (tree_slot.x, tree_slot.width)
            };

            let effective_right = tree_slot_x + tree_slot_width - 1.0;

            // clip_bounds() child clip: constrain all children (DisclosureToggle) to tree column.
            // Widget-local coords: framework converts via abs_rect.x + clip_rect.x.
            // Height = content_height (not viewport) because this clip lives inside
            // SPEC-PM-017 R1b: the scene Scroll node no longer translates Y
            // for this body (content-owned vertical translation), so the Y
            // clip lives in viewport space and spans the viewport height.
            // X is still scene-translated; the X extent stays content-space.
            let clip_w = (effective_right - tree_slot_x).max(0.0);
            self.tree_col_clip = Some(RectF::new(tree_slot_x, 0.0, clip_w, available_height));

            // Nodes eligible for disclosure toggles: visible, has_children, AND fits within
            // the effective clip boundary (same boundary used in paint for tree-column clip).
            let eligible_ids: HashSet<u64> = self
                .visible_rows
                .iter()
                .filter(|vr| vr.has_children)
                .filter(|vr| {
                    let indent = vr.depth as f32 * TREE_INDENT_STEP;
                    let bx = tree_slot_x + indent + (TREE_INDENT_STEP - TREE_CONTROL_WIDTH) * 0.5;
                    bx + TREE_CONTROL_WIDTH <= effective_right
                })
                .map(|vr| vr.node_id)
                .collect();

            // Remove stale disclosure nodes (INV-CHILD-BOUNDED).
            let stale: Vec<u64> = self
                .disclosure_nodes
                .keys()
                .filter(|id| !eligible_ids.contains(id))
                .copied()
                .collect();
            for id in stale {
                if let Some(child_node) = self.disclosure_nodes.remove(&id) {
                    cx.tree.remove_node(child_node);
                }
            }

            let disclosure_constraints =
                Constraints::tight(Size::new(TREE_CONTROL_WIDTH, row_height));

            for (i, vr) in self.visible_rows.iter().enumerate() {
                if !eligible_ids.contains(&vr.node_id) {
                    continue;
                }
                let node_id = vr.node_id;

                let child_node = if let Some(&existing) = self.disclosure_nodes.get(&node_id) {
                    // Sync expanded state from HTS projection.
                    if let Some(n) = cx.tree.node_mut(existing) {
                        if let Some(dt) = n.widget.as_any_mut().downcast_mut::<DisclosureToggle>() {
                            dt.set_expanded(vr.expanded);
                        }
                    }
                    existing
                } else {
                    // Create new disclosure toggle with callback routed to the
                    // topology-intent funnel (SPEC-HAL-337 INV-337-M1). The
                    // toggle's own Up arm invalidates layout after the callback;
                    // TreeView::update applies the intent before that frame's
                    // layout. HTS stays sole topology authority.
                    let core_rc = self.core.clone();
                    let dt = DisclosureToggle::new(vr.expanded).on_toggle(move || {
                        core_rc
                            .borrow_mut()
                            .topology_intents
                            .push(TreeTopologyIntent::Toggle { node_id });
                    });
                    // Disclosure tag = bit 0 = 0 (`node_id << 1`). Cell-
                    // editor pools use bit 0 = 1, making the two pools
                    // structurally disjoint regardless of node_id range.
                    let key = Some(WidgetKey::U64(node_id << 1));
                    let child = cx.tree.add_child(self_node_id, Box::new(dt), key, 0);
                    self.disclosure_nodes.insert(node_id, child);
                    child
                };

                cx.layout_child(child_node, disclosure_constraints);

                // Position: in tree column at indent offset, viewport-
                // relative Y via the single f64 subtraction (SPEC-PM-017
                // R1b) — toggle rects live in the same small space the body
                // paints in, which also restores their hit rects at depth.
                let indent = vr.depth as f32 * TREE_INDENT_STEP;
                let bx = tree_slot_x + indent + (TREE_INDENT_STEP - TREE_CONTROL_WIDTH) * 0.5;
                let by = ((self.first_sorted + i) as f64 * row_height as f64 - scroll_y) as f32;
                cx.set_child_rect(
                    child_node,
                    RectF::new(bx, by, TREE_CONTROL_WIDTH, row_height),
                );
            }
        } else {
            // No tree column — remove all disclosure nodes.
            for (_, child_node) in self.disclosure_nodes.drain() {
                cx.tree.remove_node(child_node);
            }
            self.tree_col_clip = None;
        }

        // HAL391: CMS publishes the actual editor allocation. This body owns
        // viewport-relative Y (PM-017 R1b), just as for its disclosure children.
        // A tree-column-only child clip must not erase a real editor in another
        // column. Extend the existing child boundary from those same allocations;
        // disclosure admission above still requires its entire rect to fit.
        let session = self.core.borrow().cms.edit_session()
            .and_then(|session| session.editor_node_id.map(|node| (node, session.row_id)));
        if let Some((editor, row)) = session {
            if cx.tree.node(editor).is_some_and(|node| node.cell_editor_presentation
                == crate::core::appearance::CellEditorPresentation::CellIntegrated) {
                if let (Some(vi), Some(mut allocation)) = (
                    self.visible_rows.iter().position(|visible| visible.node_id == row),
                    cx.tree.get_rect(editor),
                ) {
                    allocation.y = ((self.first_sorted + vi) as f64 * row_height as f64 - scroll_y) as f32;
                    cx.set_child_rect(editor, allocation);
                    let left = self.tree_col_clip.map_or(allocation.x, |clip| clip.x.min(allocation.x));
                    let right = self.tree_col_clip.map_or(allocation.right(), |clip| clip.right().max(allocation.right()));
                    self.tree_col_clip = Some(RectF::new(left, 0.0, right - left, available_height));
                }
            }
        }

        // Layout contract: size = viewport-constrained, content_size = total extent.
        // ScrollArea uses content_size for scroll domain (scrollbar sizing, max_scroll).
        let size = Size::new(available_width, available_height);
        let content_size = Size::new(content_width, content_height);
        if cx.content_owns_vertical_translation()
            && available_width.is_finite()
            && available_width > 0.0
            && available_height.is_finite()
            && available_height > 0.0
            && !self.visible_rows.is_empty()
            && self.disclosure_nodes.is_empty()
            && cx.tree.children(self_node_id).is_empty()
        {
            let core = self.core.borrow();
            if core.cell_source.is_none()
                && core.cms.edit_session().is_none()
                && core.layout_configuration != u64::MAX
            {
                self.fixed_window = core
                    .cms
                    .hts_mut()
                    .fixed_window_stamp()
                    .map(|stamp| (core.layout_configuration, stamp));
            }
        }
        LayoutResult::new(size, content_size)
    }

    fn paint(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _children: &[crate::core::widget_tree::NodeId],
    ) {
        let theme = cx.theme;
        let font_size = theme.typography.font_size;

        #[cfg(debug_assertions)]
        self.paint_query_count.set(0);

        // Debug-only clip stack depth guard.
        #[cfg(debug_assertions)]
        let mut clip_depth: i32 = 0;

        // Content width for row backgrounds — must span full scrollable width.
        let content_width = self.core.borrow().cms.hts().column_layout().total_width();
        let row_bg_width = content_width.max(rect.width);

        if self.visible_rows.is_empty() {
            // Empty viewport still gets a row-background substrate, viewport-bounded.
            cx.scene.push_rect(
                RectF::new(rect.x, rect.y, row_bg_width, rect.height),
                theme.colors.list.row_background,
                LAYER_CONTENT - 2,
                None,
            );
            return;
        }

        let core = self.core.borrow();
        let row_height = core.cms.hts().row_height();
        let tree_col_id = core.tree_column_id;
        let slots: Vec<_> = core.cms.hts().column_layout().body_slots().collect();
        let sep_slots: Vec<_> = core.cms.hts().column_layout().header_slots().collect();
        let hovered_sep = core.hovered_separator;
        let active_sep = core.active_resize_separator;
        // SPEC-PM-017 R1b / INV-PM-017-3: the f64 scroll offset feeds the
        // single subtraction in row_y_abs; nothing downstream sees
        // content-magnitude Y.
        let scroll_y = core.cms.hts().scroll_offset();

        let first_sorted = self.first_sorted;

        let sf = cx.scene.scale_factor();
        let base_x = snap_to_physical(rect.x, sf);
        let snapped_y = snap_to_physical(rect.y, sf);

        // INV-TV-PAINT-CONTENT-COORDS (R1b form): canonical Y for every
        // per-row primitive — now viewport-relative. The content-space
        // product and the scroll subtraction happen in f64 (INV-PM-017-3);
        // only the small viewport-local result narrows to f32.
        let row_y_abs = |vi: usize| -> f32 {
            let content_y = (first_sorted + vi) as f64 * row_height as f64;
            snapped_y + (content_y - scroll_y) as f32
        };

        // Row background at LAYER_CONTENT - 2: below guides (LAYER_CONTENT - 1), content
        // (LAYER_CONTENT), and the per-row selection fills (also LAYER_CONTENT - 2, pushed
        // below with higher order_key so they composite on top).
        //
        // INV-TV-BG-VISIBLE-EXTENT: height spans the visible-row extent, never the full
        // scroll content height. A full-content background emits a quad of logical height
        // total_count * row_height; for large topologies that exceeds the GPU guard band
        // and the rect-pipeline draw for the entire z=0 batch (this background plus the
        // co-batched selection fills) is discarded — neither reaches the framebuffer.
        // Same extent the guide clip uses: row_y_abs(0), visible_rows.len() * row_height.
        let bg_height = (self.visible_rows.len() as f32 * row_height).max(rect.height);
        cx.scene.push_rect(
            RectF::new(base_x, row_y_abs(0), row_bg_width, bg_height),
            theme.colors.list.row_background,
            LAYER_CONTENT - 2,
            None,
        );

        // Precompute column slot lookups — deterministic, no per-row iteration dependence.
        let tree_slot =
            tree_col_id.and_then(|id| slots.iter().find(|s| s.column_id == id).copied());

        // --- Row selection backgrounds + focus outlines (LAYER_CONTENT - 2) ---
        // Mirrors ListView Pass 1: selection bg over default bg, focus border on top.
        let list_colors = &theme.colors.list;
        for (i, vr) in self.visible_rows.iter().enumerate() {
            let is_selected = core.selection.is_selected(vr.node_id);
            let is_focused = core.selection.is_focused(vr.node_id);
            let is_hovered = Some(vr.node_id) == core.hovered_node;
            if is_selected || is_focused || is_hovered {
                let row_y = row_y_abs(i);
                let row_rect = RectF::new(base_x, row_y, row_bg_width, row_height);
                // Row-hover band: weaker than selection, painted only when the row is
                // not selected (selection has visual priority — mirrors List precedence).
                if is_hovered && !is_selected {
                    cx.scene
                        .push_rect(row_rect, list_colors.row_hover, LAYER_CONTENT - 2, None);
                }
                if is_selected {
                    let sel_color = if core.focused {
                        list_colors.row_selected
                    } else {
                        list_colors.row_selected_inactive
                    };
                    cx.scene
                        .push_rect(row_rect, sel_color, LAYER_CONTENT - 2, None);
                }
                if is_focused {
                    cx.scene.push_border_rect(
                        row_rect,
                        theme.colors.borders.focus,
                        1.0,
                        LAYER_CONTENT,
                        None,
                    );
                }
            }
        }

        // --- Guide lines (clipped to tree column) ---
        if let Some(ts) = tree_slot {
            let tree_col_x = base_x + ts.x;

            let row_projections: Vec<_> = self
                .visible_rows
                .iter()
                .enumerate()
                .map(|(vi, vr)| RowProjection {
                    row_id: RowId::from_raw(vr.row_index as u64),
                    row_index: vi,
                    // SPEC-PM-017 R1b: viewport-relative, single f64 subtraction.
                    y_offset: ((first_sorted + vi) as f64 * row_height as f64 - scroll_y) as f32,
                    height: row_height,
                })
                .collect();

            // INV-TV-GUIDES-VISIBLE-RANGE: Clip to content-local row extent, not viewport.
            // The batcher shifts all coords inside a ScrollNode by -scroll_offset.
            // A viewport-relative clip (rect.y, rect.height) diverges from content-local
            // guide rects once scroll_offset > viewport_height, clipping them out.
            // Content-local clip stays aligned with guide content through any scroll.
            //
            // INV-CLIP-EDGE-ON-SEPARATOR: Right edge derived from the same canonical
            // separator X used by the separator paint pass (base_x + slot.x + slot.width - 1.0).
            let content_top = row_y_abs(0);
            let content_h = self.visible_rows.len() as f32 * row_height;
            let tree_col_sep_x = snap_to_physical(base_x + ts.x + ts.width - 1.0, sf);
            let tree_clip_w = (tree_col_sep_x - tree_col_x).max(0.0);
            let clip = RectF::new(tree_col_x, content_top, tree_clip_w, content_h);
            cx.scene.begin_clip(clip, None);
            #[cfg(debug_assertions)]
            {
                clip_depth += 1;
            }

            let guide_color = tree_paint::compute_guide_color(
                theme.colors.list.row_background,
                theme.colors.list.text,
            );
            let guide_rects: Vec<RectF> = tree_paint::build_tree_guide_rects(
                &row_projections,
                &self.visible_rows,
                tree_col_x,
                TREE_INDENT_STEP,
                GUIDE_LINE_WIDTH,
                rect.y,
                sf,
                // SPEC-PM-017 R1b: keep the dot pattern content-anchored.
                (scroll_y.floor() as i64).rem_euclid(2) as i32,
            );
            // Guide mesh at LAYER_CONTENT - 1: above background, below disclosure
            // toggles and icons (LAYER_CONTENT). Toggles must render above guides.
            cx.scene
                .push_rect_mesh(&guide_rects, guide_color, LAYER_CONTENT - 1);

            cx.scene.end_clip();
            #[cfg(debug_assertions)]
            {
                clip_depth -= 1;
            }
        }

        let mut integrated_edges = Vec::new();
        // --- Per-cell content (text labels only; disclosure toggles are child widgets) ---
        for (i, vr) in self.visible_rows.iter().enumerate() {
            let row_y = row_y_abs(i);
            let indent = vr.depth as f32 * TREE_INDENT_STEP;
            let is_selected = core.selection.is_selected(vr.node_id);
            let text_color = if is_selected {
                list_colors.text_selected
            } else {
                list_colors.text
            };

            #[cfg(debug_assertions)]
            self.paint_query_count.set(self.paint_query_count.get() + 4);

            for slot in &slots {
                let cell_x = base_x + slot.x;
                // INV-CLIP-EDGE-ON-SEPARATOR: Clip right edge matches separator X.
                let col_sep_x = snap_to_physical(base_x + slot.x + slot.width - 1.0, sf);
                let cell_clip_w = (col_sep_x - cell_x).max(0.0);
                let cell_rect = RectF::new(cell_x, row_y, cell_clip_w, row_height);
                if cx.has_cell_editor_edge(vr.node_id, slot.column_id) {
                    integrated_edges.push((vr.node_id, slot.column_id,
                        RectF::new(cell_x, row_y, slot.width, row_height), is_selected,
                        false));
                    continue;
                }
                if tree_slot.map_or(false, |s| s.column_id == slot.column_id) {
                    cx.scene.begin_clip(cell_rect, None);
                    #[cfg(debug_assertions)]
                    {
                        clip_depth += 1;
                    }
                    // --- Tree column: icon + text label (disclosure handled by child widget) ---
                    let icon_size = (row_height * 0.7).floor().clamp(12.0, 18.0);
                    let icon_gap = 3.0_f32;
                    let icon_x = cell_x + indent + TREE_INDENT_STEP;
                    let icon_y = row_y + (row_height - icon_size) / 2.0;

                    let icon_color = if vr.has_children {
                        cx.theme.colors.icons.folder
                    } else {
                        cx.theme.colors.icons.file
                    };

                    if vr.has_children {
                        let folder_id = if vr.expanded {
                            IconId::FOLDER_OPEN
                        } else {
                            IconId::FOLDER
                        };
                        if let Some(def) = cx.icon_registry.get(folder_id) {
                            cx.scene.push_icon(
                                def,
                                icon_x,
                                icon_y,
                                icon_size,
                                icon_color,
                                LAYER_CONTENT,
                            );
                        }
                    } else {
                        if let Some(def) = cx.icon_registry.get(IconId::FILE) {
                            cx.scene.push_icon(
                                def,
                                icon_x,
                                icon_y,
                                icon_size,
                                icon_color,
                                LAYER_CONTENT,
                            );
                        }
                    }

                    let text_x = cell_x + indent + TREE_INDENT_STEP + icon_size + icon_gap;
                    let max_w = slot.width - indent - TREE_INDENT_STEP - icon_size - icon_gap;
                    if max_w > 0.0 && !vr.label.is_empty() {
                        let run = cx.text_engine.shape(&vr.label, font_size);
                        let text_y = row_y + (row_height - run.height) / 2.0;
                        cx.text_engine.draw(
                            &run,
                            text_x,
                            text_y,
                            text_color,
                            LAYER_CONTENT,
                            cx.scene,
                        );
                    }
                    cx.scene.end_clip();
                    #[cfg(debug_assertions)]
                    {
                        clip_depth -= 1;
                    }
                } else if let Some(cell) = core.cms.realized_cell_value(vr.node_id, slot.column_id)
                {
                    // --- Non-tree column: realized cell value, painted by the
                    // shared substrate cell painter (self-clips to cell_rect). ---
                    core.cms.paint_one_cell(cx, cell, cell_rect, is_selected);
                }
            }
        }

        cx.paint_cell_editor_edges(integrated_edges.iter().copied());

        // --- Column separators (all scrollable, body in ScrollArea content space) ---
        // INV-RESIZE-VISUAL-CONSISTENCY: Separator X matches header separator X.
        // INV-SEP-Z-PRIORITY: Active > Hovered > Normal
        //
        // Slot source: header_slots() — same boundary definition as TreeView header.
        // (header_slots and body_slots return identical data; explicit for parity.)
        //
        // SPEC-PM-007: Separators at LAYER_CONTENT + 1, active/hover at +2.
        // Must be strictly above backgrounds to avoid clip-hash ordering flicker.
        //
        // Separator X: snap in content space. ScrollNode applies
        // -scroll_x automatically. Matches header snap order.
        {
            let inter_column_slots = &sep_slots[..sep_slots.len().saturating_sub(1)];

            // Normal pass: 1px, borders.subtle.
            for slot in inter_column_slots {
                let is_active = active_sep == Some(slot.column_id);
                let is_hovered = hovered_sep == Some(slot.column_id);
                if is_active || is_hovered {
                    continue;
                }
                let sep_x = snap_to_physical(base_x + slot.x + slot.width - 1.0, sf);
                // SPEC-PM-017 R1b: no scene Y translation — paint directly
                // at the viewport top; the counter-scroll trick is retired.
                let sep_rect = RectF::new(sep_x, rect.y, 1.0, rect.height);
                cx.paint_cell_grid_separator(
                    sep_rect,
                    cx.theme.colors.borders.subtle,
                    LAYER_CONTENT + 1,
                    &integrated_edges,
                );
            }

            // Hovered/active pass: 2px, borders.focus (active) or borders.default (hovered).
            for slot in inter_column_slots {
                let is_active = active_sep == Some(slot.column_id);
                let is_hovered = hovered_sep == Some(slot.column_id);
                if !is_active && !is_hovered {
                    continue;
                }
                let sep_x = snap_to_physical(base_x + slot.x + slot.width - 1.0, sf);
                let (sep_w, sep_color) = if is_active {
                    (2.0, cx.theme.colors.borders.focus)
                } else {
                    (2.0, cx.theme.colors.borders.default)
                };
                let sep_rect = RectF::new(sep_x, rect.y, sep_w, rect.height);
                cx.paint_cell_grid_separator(sep_rect, sep_color, LAYER_CONTENT + 2, &integrated_edges);
            }
        }

        drop(core);

        // Clip stack balance assertion.
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            clip_depth, 0,
            "Clip stack imbalance in TreeViewBody::paint()"
        );

        // INV-TV-3: Assert paint metadata queries bounded by O(n_visible).
        #[cfg(debug_assertions)]
        {
            let n = self.visible_rows.len();
            let count = self.paint_query_count.get();
            debug_assert!(
                count <= n * 4,
                "INV-TV-3 violation: paint metadata queries ({}) exceed 4 * visible_rows ({})",
                count,
                n * 4
            );
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Store widget_id in core for TreeView to invalidate us when column widths change.
        // Only needs to happen once (check avoids borrow_mut on every event).
        if self.core.borrow().body_widget_id.is_none() {
            self.core.borrow_mut().body_widget_id = Some(cx.widget_id);
        }

        // Target-only for non-drag events: when a click lands on a DisclosureToggle child,
        // that child is the Target (and consumes it). TreeViewBody only receives Target for
        // clicks directly on the body surface — no ad-hoc coordinate exclusion needed.
        // Exception: during an active drag-select, Capture-phase Move/Up events must
        // pass through (pointer is captured by this widget).
        let drag_active = self.core.borrow().selection_drag_active;
        let activation_active = self.core.borrow().activation_press.is_some();
        let activation_capture_event = activation_active
            && matches!(event, Event::Pointer(pe) if matches!(pe.kind,
                PointerEventKind::Move | PointerEventKind::DragStart | PointerEventKind::Up));
        if phase != EventPhase::Target
            && !((drag_active || activation_capture_event) && phase == EventPhase::Capture)
        {
            return false;
        }

        if let Event::Pointer(pe) = event {
            if activation_active && self.handle_activation_press(cx, pe) {
                return true;
            }
        }

        // The canonical router represents a second/third physical Down by
        // DoubleClick/TripleClick instead of also emitting an ordinary Down.
        // Arm that original cycle once; only its matching Up can activate.
        let multiclick_armed = if let Event::Pointer(pe) = event {
            matches!(
                pe.kind,
                PointerEventKind::DoubleClick | PointerEventKind::TripleClick
            ) && self.arm_activation_press(cx, pe)
        } else {
            false
        };

        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    let Some(rect) = cx.widget_global_rect() else {
                        return false;
                    };

                    // If click is on a column separator, don't consume — allow bubble
                    // to TreeView for resize arming (INV-RESIZE-ARM-FORWARDING).
                    let local_x = pe.global_pos.0 - rect.x;
                    if self.is_on_separator(local_x) {
                        return false;
                    }

                    let local_y = pe.global_pos.1 - rect.y;
                    if let Some(vi) = self.hit_test_row(local_y) {
                        let node_id = self.visible_rows[vi].node_id;
                        if pe.modifiers.shift {
                            // Commit the range immediately on press, then keep the
                            // drag armed so a subsequent move keeps extending from
                            // the same anchor. ExtendTo is replacing — it recomputes
                            // [anchor..target] from the persistent anchor on every
                            // call, so the press-time commit plus drag re-apply
                            // cannot double-count.
                            let mut core = self.core.borrow_mut();
                            core.selection_drag_anchor = Some(node_id);
                            core.selection_drag_active = true;
                            core.selection_drag_last = Some(node_id);
                            let core_ref = &mut *core;
                            reduce_ordered_selection(
                                &mut core_ref.selection,
                                OrderedSelectionAction::ExtendTo { key: node_id },
                                &*core_ref.cms.hts(),
                            );
                            drop(core);
                            cx.capture_pointer(PointerButton::Primary);
                            cx.invalidate_layout();
                        } else {
                            // Non-shift click: terminate any prior drag session.
                            let action = if pe.modifiers.ctrl || pe.modifiers.meta {
                                OrderedSelectionAction::Toggle { key: node_id }
                            } else {
                                OrderedSelectionAction::Select { key: node_id }
                            };
                            let mut core = self.core.borrow_mut();
                            core.selection_drag_anchor = None;
                            core.selection_drag_active = false;
                            core.selection_drag_last = None;
                            let core_ref = &mut *core;
                            reduce_ordered_selection(
                                &mut core_ref.selection,
                                action,
                                &*core_ref.cms.hts(),
                            );
                            drop(core);
                            self.arm_activation_press(cx, pe);
                            cx.invalidate_layout();
                        }
                        return true;
                    }
                    false
                }
                PointerEventKind::Move => {
                    // Resolve the row under the pointer once (visible-row index → node id).
                    let Some(rect) = cx.widget_global_rect() else {
                        return false;
                    };
                    let local_y = pe.global_pos.1 - rect.y;
                    let hit_vi = self.hit_test_row(local_y);
                    let hit_node = hit_vi.map(|vi| self.visible_rows[vi].node_id);

                    // PM-018: an input frame does not invalidate retained paint.
                    // Only a changed row identity dirties this body's fragment.
                    {
                        let mut core = self.core.borrow_mut();
                        if core.hovered_node != hit_node {
                            core.hovered_node = hit_node;
                            drop(core);
                            cx.invalidate_paint();
                        }
                    }

                    // Drag-select continuation (only while a drag session is active).
                    let core_ref = self.core.borrow();
                    if !core_ref.selection_drag_active {
                        return false;
                    }
                    if core_ref.selection_drag_anchor.is_none() {
                        return false;
                    }
                    drop(core_ref);

                    if let Some(current_id) = hit_node {
                        // Only dispatch when the pointer reaches a different row
                        // than the last one. ExtendTo is replacing — it recomputes
                        // [anchor..target] from the persistent anchor on every
                        // call, so the press-time commit plus re-apply here
                        // cannot double-count.
                        if self.core.borrow().selection_drag_last != hit_node {
                            let mut core = self.core.borrow_mut();
                            core.selection_drag_last = hit_node;
                            let core_ref = &mut *core;
                            reduce_ordered_selection(
                                &mut core_ref.selection,
                                OrderedSelectionAction::ExtendTo { key: current_id },
                                &*core_ref.cms.hts(),
                            );
                            drop(core);
                            cx.invalidate_layout();
                        }
                    }
                    true
                }
                PointerEventKind::Up => {
                    let mut core = self.core.borrow_mut();
                    if !core.selection_drag_active {
                        return false;
                    }

                    // Range gesture: selection was committed on press and during
                    // any drag-move. Just clear drag state.
                    core.selection_drag_anchor = None;
                    core.selection_drag_active = false;
                    core.selection_drag_last = None;
                    drop(core);
                    cx.release_pointer();
                    cx.invalidate_layout();
                    true
                }
                PointerEventKind::DoubleClick => {
                    if phase != EventPhase::Target {
                        return false;
                    }
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    let Some(rect) = cx.widget_global_rect() else {
                        return false;
                    };

                    // INV-SEPARATOR-DBLCLICK-NOT-CONSUMED: If on separator, bubble.
                    let local_x = pe.global_pos.0 - rect.x;
                    if self.is_on_separator(local_x) {
                        return false;
                    }

                    let local_y = pe.global_pos.1 - rect.y;
                    if let Some(vi) = self.hit_test_row(local_y) {
                        let vr = &self.visible_rows[vi];
                        if vr.has_children {
                            let node_id = vr.node_id;
                            // SPEC-HAL-337 INV-337-M1: enqueue; TreeView::update
                            // applies before this frame's layout. Focus
                            // choreography runs in the drain.
                            self.core
                                .borrow_mut()
                                .topology_intents
                                .push(TreeTopologyIntent::Toggle { node_id });
                            cx.invalidate_layout();
                            return true;
                        }
                    }

                    // Edit trigger: a double-click that did not toggle a parent row
                    // (e.g. a leaf row) enters edit mode on an editable, non-tree cell.
                    // The has_children toggle above is unchanged.
                    let scroll_x = self
                        .core
                        .borrow()
                        .horizontal_scroll_link
                        .as_ref()
                        .map_or(0.0, |l| l.get());
                    let hit = self.core.borrow().cms.hit_test_cell(
                        local_x,
                        local_y,
                        self.viewport_height,
                        self.first_sorted,
                        &self.visible_rows,
                        scroll_x,
                    );
                    if let Some((row_id, column_id)) = hit {
                        let core = self.core.borrow();
                        let cell = if core.tree_column_id == Some(column_id) {
                            None
                        } else {
                            core.cell_source
                                .as_ref()
                                .and_then(|src| src.borrow().cell(row_id, column_id))
                        };
                        drop(core);
                        if let Some(cell) = cell {
                            let entered = self.core.borrow_mut().cms.try_enter_edit(
                                row_id,
                                column_id,
                                &cell.value,
                                cell.editable,
                            );
                            if entered {
                                cx.invalidate_layout();
                                return true;
                            }
                        }
                    }
                    multiclick_armed
                }
                PointerEventKind::TripleClick => multiclick_armed,
                _ => false,
            },
            _ => false,
        }
    }

    /// Sync scroll position from ScrollArea to HTS.
    ///
    /// Dispatched by app.rs pre_layout_sync_scroll_to_lists / post_layout_sync_scroll_to_lists.
    /// ScrollArea is detected as scroll anchor automatically (widget_tree.rs:322-323).
    /// TreeViewBody receives sync_scroll as a ScrollArea descendant.
    ///
    /// Reports a change so the existing scroll-sync dispatcher can invalidate
    /// layout or paint as required.
    fn sync_scroll(&mut self, scroll_y: f32) -> bool {
        let core = self.core.borrow();
        let mut hts = core.cms.hts_mut();
        let prev = hts.scroll_offset();
        // SPEC-PM-017: the f32 inbound offset is a rung-3 limitation
        // (ScrollArea core is still f32); widen at the boundary.
        if (scroll_y as f64 - prev).abs() < 0.01 {
            return false;
        }
        let reused = scroll_y.is_finite()
            && core.cell_source.is_none()
            && core.cms.edit_session().is_none()
            && core.topology_intents.is_empty()
            && self
                .fixed_window
                .as_mut()
                .is_some_and(|(configuration, stamp)| {
                    if *configuration != core.layout_configuration {
                        return false;
                    }
                    let realized = self.first_sorted..self.first_sorted + self.visible_rows.len();
                    hts.scroll_within_fixed_window(scroll_y as f64, realized, stamp)
                });
        if !reused {
            self.fixed_window = None;
            hts.scroll_to(scroll_y as f64);
        }
        true
    }

    fn scroll_sync_invalidation(&self) -> ScrollSyncInvalidation {
        if self.fixed_window.is_some() {
            ScrollSyncInvalidation::Paint
        } else {
            ScrollSyncInvalidation::Layout
        }
    }

    /// Take the pending vertical scroll request.
    fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.core.borrow_mut().requested_scroll_y.take()
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        let mut core = self.core.borrow_mut();
        if core.hovered_node.is_some() {
            core.hovered_node = None;
            drop(core);
            cx.invalidate_paint();
        }
    }

    fn on_pointer_capture_cancel(
        &mut self,
        _cx: &mut EventContext,
        pointer: PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
        let mut core = self.core.borrow_mut();
        if core
            .activation_press
            .is_some_and(|press| press.pointer == pointer)
        {
            core.activation_press = None;
        }
    }

    fn on_unmount(&mut self) {
        self.fixed_window = None;
        self.core.borrow_mut().activation_press = None;
    }

    /// Return separator centerlines in content coordinates for the retained
    /// ScrollArea geometry projection. This does not authorize resize routing.
    fn separator_hit_bands(&self) -> Option<Vec<f32>> {
        let core = self.core.borrow();
        let hts = core.cms.hts();
        let layout = hts.try_column_layout()?;
        let slots: Vec<_> = layout.header_slots().collect();
        if slots.len() <= 1 {
            return Some(Vec::new());
        }
        let bands = slots[..slots.len() - 1]
            .iter()
            .map(|slot| slot.x + slot.width - 1.0)
            .collect();
        Some(bands)
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn needs_viewport_constraint(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "TreeViewBody"
    }

    fn clip_bounds(&self) -> Option<RectF> {
        self.tree_col_clip
    }
}

// Sealed WidgetDowncast impl (required by Widget trait).
impl crate::core::widget::WidgetDowncast for TreeViewBody {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
