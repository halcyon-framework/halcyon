// =============================================================================
// SPEC-HAL-260A §4.1, §6.1: PropertyEditorBody — body region content widget
// =============================================================================
//
// PropertyEditorBody is the row-content widget inside the ScrollArea inside
// the PE Outer composite (§4.1). It owns layout, paint, and body pointer
// events. PE Outer owns keyboard handling, column resize, focus scope, and
// capture coordination — Body does NOT.
//
// Body shares state with PE Outer through `Rc<RefCell<PropertyEditorCore>>`
// (§8.3). All HTS topology, selection, edit session, and policy live in
// `core`; Body holds only layout-cached fields and child-pool storage.
//
// Child mounting strategy (SPEC §6.1):
//
//   | Mechanism                  | Storage                                  |
//   |----------------------------|------------------------------------------|
//   | Disclosure-toggle pool     | HashMap<u64, NodeId>                     |
//   | Always-mounted Bool editor | HashMap<(u64, ColumnId), NodeId>         |
//   | Active edit session        | core.edit_session: Option<EditSession>   |
//
// `current_edit_node` tracks the NodeId of the currently-mounted edit
// editor child, used by B6's edit-session reconciliation to remove the
// editor when `core.edit_session` clears (deferred tree-structural
// mutation — INV-PE-TREE-DEFERRED).
//
// Structural parallel: TreeViewBody (`src/widgets/tree_view/body.rs:134`).
// PE Body diverges by carrying two pools (TreeView has one — disclosure)
// and an edit-session tracking field.
//
// B1 establishes only the struct and Widget trait skeleton. Real bodies
// for layout, paint, child reconciliation, and pointer events land in
// B2–B7.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::ordered_selection::{OrderedIndex, OrderedSelectionAction};
use crate::core::scene::{LAYER_CONTENT, LAYER_FOCUS};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetKey};
use crate::engines::cell_source::PropertyValue;
use crate::engines::hierarchical::VisibleRow;
use crate::utils::id::WidgetInternalId;
use crate::widgets::disclosure_toggle::DisclosureToggle;
use crate::widgets::tabular_view::column::ColumnId;

use super::core::{ColumnIntent, EditIntent, PEIntent, PropertyEditorCore, TreeIntent};
use crate::engines::cell_mechanics::EditOutcome;
use crate::engines::cell_source::Cell;

use super::types::{ColumnKind, PropertyNodeKind};
use crate::engines::cell_source::EditExitReason;

/// SPEC-HAL-260A §14.3: tree indentation per depth level (pixels).
/// Matches the established PE visual cadence carried over from SPEC-HAL-260.
const TREE_INDENT_STEP: f32 = 20.0;

/// SPEC-HAL-260A §14.3 / §6.1: width reserved for the disclosure-toggle
/// child widget in the tree column. Body reserves this horizontal slot in
/// every row's tree cell; the `DisclosureToggle` child (mounted by B4) is
/// positioned within it. Body does not render the disclosure indicator
/// itself.
const TREE_CONTROL_WIDTH: f32 = 16.0;

/// SPEC-HAL-260A §4.1, §6.1: row-content widget inside PE Outer's
/// ScrollArea. Shares `PropertyEditorCore` with PE Outer through
/// `Rc<RefCell<...>>`. Owns layout, paint, body pointer events; does NOT
/// own keyboard handling, focus scope, or capture coordination.
pub struct PropertyEditorBody {
    id: WidgetInternalId,
    core: Rc<RefCell<PropertyEditorCore>>,

    // ------------------------------------------------------------------------
    // Cached from layout (§14.2). Read by paint, hit-test, and child
    // reconciliation. Populated in B2.
    // ------------------------------------------------------------------------
    /// Visible rows snapshot from the last layout pass.
    visible_rows: Vec<VisibleRow>,
    /// Index of the first visible row in the sorted enumeration, computed
    /// during layout from `scroll_offset / row_height`. Read by paint and
    /// hit-test (B3 / B7) — never recomputed from scroll offset.
    first_sorted: usize,
    /// Viewport height from the last layout pass.
    viewport_height: f32,
    /// Content height (total scrollable extent) from the last layout pass.
    content_height: f32,
    /// Tree-column clip rect in widget-local coordinates. Used by
    /// `clip_bounds()` to constrain disclosure-toggle children to the tree
    /// column boundary. Populated in B2.
    tree_col_clip: Option<RectF>,

    // ------------------------------------------------------------------------
    // Child pools (SPEC §6.1). Reconciled every layout pass in B4 / B5.
    // ------------------------------------------------------------------------
    /// Disclosure-toggle child widget NodeIds keyed by stable node_id.
    /// Mounted when the row has children AND is visible; unmounted otherwise.
    /// Identical pattern to TreeViewBody. Reconciled in B4.
    disclosure_toggles: HashMap<u64, NodeId>,
}

impl PropertyEditorBody {
    /// Construct a new PropertyEditorBody sharing the given core with PE
    /// Outer. Called from PE Outer's constructor in Phase O.
    pub(crate) fn new(core: Rc<RefCell<PropertyEditorCore>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            core,
            visible_rows: Vec::new(),
            first_sorted: 0,
            viewport_height: 0.0,
            content_height: 0.0,
            tree_col_clip: None,
            disclosure_toggles: HashMap::new(),
        }
    }

    /// Hit-test: viewport-local Y → visible-row index.
    ///
    /// Delegates to `CellMechanicsSubstrate::hit_test_row`, supplying the
    /// body's layout-cached window (`viewport_height`, `first_sorted`,
    /// `visible_rows.len()`). CMS reads `row_height` / `scroll_offset` from
    /// its owned HTS.
    fn hit_test_row(&self, local_y: f32) -> Option<usize> {
        let core = self.core.borrow();
        core.cms.hit_test_row(
            local_y,
            self.viewport_height,
            self.first_sorted,
            self.visible_rows.len(),
        )
    }

    /// Hit-test: viewport-local (X, Y) → (node_id, column_id).
    ///
    /// Delegates to `CellMechanicsSubstrate::hit_test_cell`, supplying the
    /// body's layout-cached window and the horizontal scroll offset.
    /// Returns `None` over band rows and outside every column slot.
    fn hit_test_cell(&self, local_x: f32, local_y: f32) -> Option<(u64, ColumnId)> {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        core.cms.hit_test_cell(
            local_x,
            local_y,
            self.viewport_height,
            self.first_sorted,
            &self.visible_rows,
            scroll_x,
        )
    }

    /// Check whether viewport-local X falls on a column separator.
    ///
    /// Delegates to `CellMechanicsSubstrate::is_on_separator` with the
    /// horizontal scroll offset. PointerDown on a separator bubbles to
    /// outer/header for resize-arming (INV-RESIZE-ARM-FORWARDING).
    fn is_on_separator(&self, local_x: f32) -> bool {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        core.cms.is_on_separator(local_x, scroll_x)
    }
}

impl Widget for PropertyEditorBody {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let available_width = constraints.max_width;
        let available_height = constraints.max_height;

        // SPEC §14.2: drain the editor-exit outcomes the substrate's owned
        // edit path recorded and route each through the intent funnel, before
        // the rest of layout runs — so a committed value is applied (and the
        // HTS projection rebuilt) before cell realization, and the edit
        // session is cleared before the reconcile pass below unmounts the
        // editor.
        {
            let mut core = self.core.borrow_mut();
            let outcomes = core.cms.drain_edit_outcomes();
            for outcome in outcomes {
                if let Some(intent) = pe_intent_from_edit_outcome(outcome) {
                    core.dispatch_intent(intent);
                }
            }
        }

        // SPEC §14.2 steps 1–3 + step 7 (partial): pull all HTS-derived
        // values inside one mut borrow, derive cached state outside.
        //
        // PE Outer (Phase O) is responsible for calling
        // `hts.resolve_geometry(width, scale_factor)` BEFORE this layout
        // runs (mirrors TreeView's composite-level resolve at
        // `tree_view.rs:805`). `column_layout()` and `total_width()` would
        // panic in debug otherwise (assert_resolved_for_epoch).
        let core = self.core.borrow_mut();
        core.cms.hts_mut().set_viewport_height(available_height);
        // SPEC-PM-017: engine_first published but not yet consumed here —
        // PE adopts single-source first_sorted at its ladder rung.
        let (visible, total_count, _engine_first) = core.cms.hts_mut().visible_rows_with_total();
        let row_height = core.cms.hts().row_height();
        // SPEC-PM-017: f32 shim — PE rung.
        let scroll_offset = core.cms.hts().scroll_offset() as f32;
        let content_width = core.cms.hts().column_layout().total_width();
        let tree_col_id = core.tree_column_id;
        let tree_slot_geom = core
            .cms
            .hts()
            .column_layout()
            .body_slots()
            .find(|s| Some(s.column_id) == tree_col_id)
            .map(|s| (s.x, s.width));
        drop(core);

        // SPEC §14.2 step 3: derive cached state.
        self.visible_rows = visible;
        self.viewport_height = available_height;
        let content_height = total_count as f32 * row_height;
        self.content_height = content_height;
        self.first_sorted = if row_height > 0.0 {
            (scroll_offset / row_height).floor() as usize
        } else {
            0
        };

        // SPEC-PM-016-R1: realize per-cell content for the current visible
        // window. Layout owns realization; paint reads only the CMS cache.
        // Pre-PM-016 PE paint called `cell()` per visible cell per paint
        // pass (body.rs pre-this-task line ~1096); the cache eliminates
        // that source traffic on the paint path.
        self.core
            .borrow_mut()
            .cms
            .realize_cell_values(&self.visible_rows);

        // SPEC §14.2 step 4: disclosure-toggle child reconciliation.
        // Mirrors TreeViewBody's pattern (tree_view/body.rs:268-369).
        // HTS is sole topology authority (SPEC-HAL-259); toggle widgets
        // request expansion via `core.cms.hts_mut().toggle_by_id(node_id)` and PE
        // re-reads the projection on the next layout pass.
        //
        // PE preset `TreeChromeConfig.disclosure_click_enabled` is false
        // (row double-click is the SPEC-preferred expand path, wired by
        // B7), but wiring `on_toggle` on the widget keeps a direct click
        // on the indicator zone working — matches TreeView pattern. The
        // preset is a hint, not a hard click block.
        let self_node_id = cx
            .current_node()
            .expect("PropertyEditorBody must have current_node during layout");

        if let Some((tree_slot_x, tree_slot_width)) = tree_slot_geom {
            // Eligibility: row has children AND the toggle's right edge
            // fits inside the tree-column clip boundary. Matches
            // TreeViewBody's effective_right derivation exactly so paint
            // (B3a) and reconciliation agree on which toggles render.
            let effective_right = tree_slot_x + tree_slot_width - 1.0;

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

            // Remove stale toggles (INV-CHILD-BOUNDED: pool stays bounded
            // by eligibility, not by historical mount state).
            let stale: Vec<u64> = self
                .disclosure_toggles
                .keys()
                .filter(|id| !eligible_ids.contains(id))
                .copied()
                .collect();
            for id in stale {
                if let Some(child_node) = self.disclosure_toggles.remove(&id) {
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

                let child_node = if let Some(&existing) = self.disclosure_toggles.get(&node_id) {
                    // Sync expanded state from HTS projection — HTS is
                    // sole topology authority; widget mirrors.
                    if let Some(n) = cx.tree.node_mut(existing) {
                        if let Some(dt) = n.widget.as_any_mut().downcast_mut::<DisclosureToggle>() {
                            dt.set_expanded(vr.expanded);
                        }
                    }
                    existing
                } else {
                    // Create new toggle. Callback routes the click
                    // through the intent funnel — SPEC §12.1
                    // INV-PE-INTENT-FUNNEL: the permission gate must
                    // run for tree topology mutations. Captures
                    // `node_id` by value (Copy) — no aliasing into
                    // per-row state. `row_index` is resolved at
                    // callback time, NOT at mount time, because rows
                    // above this one may have expanded or collapsed
                    // between mount and click, shifting the index.
                    let core_rc = self.core.clone();
                    let dt = DisclosureToggle::new(vr.expanded).on_toggle(move || {
                        let row_index_opt = core_rc.borrow().cms.hts().index_of(node_id);
                        if let Some(row_index) = row_index_opt {
                            core_rc.borrow_mut().dispatch_intent(PEIntent::Tree(
                                TreeIntent::ToggleExpand { row_index },
                            ));
                        }
                    });
                    // Key encoding: disclosure tag = bit 0 = 0
                    // (`node_id << 1`). Cell-editor pool uses bit 0 = 1
                    // (`cell_widget_key` — node_id high bits, column
                    // index mid bits, tag bit 0 = 1). The opposing tag
                    // bits make the two pools structurally disjoint, so
                    // no key collision is possible at any node_id range.
                    //
                    // SPEC §7.3 historically specified both pools with
                    // tag = 1, which produced real collisions at
                    // `node_id_d == (node_id_c << 15) | col` (triggered
                    // by any PE with node_ids ≥ 2^15). This implementation
                    // fixes the bug; SPEC text amendment pending (META).
                    let key = Some(WidgetKey::U64(node_id << 1));
                    let child = cx.tree.add_child(self_node_id, Box::new(dt), key, 0);
                    self.disclosure_toggles.insert(node_id, child);
                    child
                };

                cx.layout_child(child_node, disclosure_constraints);

                // Position in content-local coords. Tree column slot's
                // origin + depth indent + horizontal centering inside
                // TREE_INDENT_STEP. Y is `(first_sorted + i) * row_height`
                // (content-local, the ScrollNode applies scroll offset
                // at paint time).
                let indent = vr.depth as f32 * TREE_INDENT_STEP;
                let bx = tree_slot_x + indent + (TREE_INDENT_STEP - TREE_CONTROL_WIDTH) * 0.5;
                let by = (self.first_sorted + i) as f32 * row_height;
                cx.set_child_rect(
                    child_node,
                    RectF::new(bx, by, TREE_CONTROL_WIDTH, row_height),
                );
            }
        } else {
            // Tree column not registered in body_slots yet (partial
            // setup before Phase O composes PE). Drain any toggles that
            // somehow accumulated; nothing to mount.
            for (_, child_node) in self.disclosure_toggles.drain() {
                cx.tree.remove_node(child_node);
            }
        }

        // SPEC §14.2 step 5: always-mounted Checkbox reconciliation for
        // Bool cells. CMS owns the mount pool, per-cell sync, and disabled
        // state; the toggle callback PE supplies routes through
        // `dispatch_intent` (INV-PE-INTENT-FUNNEL). The intent funnel
        // itself moves into CMS in a later extraction step.
        let core_rc = self.core.clone();
        self.core.borrow_mut().cms.reconcile_always_mounted_editors(
            cx,
            self_node_id,
            &self.visible_rows,
            move |node_id, col_id, new_val| {
                let intent = PEIntent::Edit(EditIntent::Commit {
                    row_id: node_id,
                    column_id: col_id,
                    new_value: PropertyValue::Bool(new_val),
                });
                core_rc.borrow_mut().dispatch_intent(intent);
            },
        );

        // SPEC §14.2 step 6: edit-session editor reconciliation. The
        // substrate owns the mount-on-demand state machine and builds the
        // editor from its registered factory; editor exits land on the
        // substrate's outcome queue, drained at the top of this layout pass
        // and routed through `dispatch_intent` (INV-PE-INTENT-FUNNEL).
        self.core.borrow_mut().cms.reconcile_edit_session_owned(
            cx,
            self_node_id,
            &self.visible_rows,
        );

        // SPEC §14.2 step 7: tree_col_clip in widget-local coordinates.
        // Height = content_height (not viewport) because the clip lives
        // inside ScrollArea's ScrollNode and must cover the full Y range;
        // the batcher intersects with ScrollArea's viewport clip. None if
        // the tree column has not been registered yet — Phase O guards
        // this in production composition; safe-empty during partial setup.
        self.tree_col_clip = tree_slot_geom.map(|(x, w)| RectF::new(x, 0.0, w, content_height));

        // SPEC §14.2 step 8: return LayoutResult — size is viewport-
        // constrained; content_size is total extent for ScrollArea's
        // scroll domain.
        let size = Size::new(available_width, available_height);
        let content_size = Size::new(content_width, content_height);
        LayoutResult::new(size, content_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let theme = cx.theme;
        let font_size = theme.typography.font_size;
        let sf = cx.scale_factor;

        // Full content width for row backgrounds — spans the entire
        // scrollable extent (not just the viewport). Resolved before the
        // early-empty return so the default background paints even when
        // there are no visible rows.
        let content_width = self.core.borrow().cms.hts().column_layout().total_width();
        let row_bg_width = content_width.max(rect.width);

        // SPEC §14.3 pass 1: default row background at LAYER_CONTENT − 2.
        // Underlies all per-row passes; height covers the full content
        // extent (or viewport, whichever is larger) so empty space below
        // visible rows still gets the body's neutral background.
        cx.scene.push_rect(
            RectF::new(
                rect.x,
                rect.y,
                row_bg_width,
                self.content_height.max(rect.height),
            ),
            theme.colors.list.row_background,
            LAYER_CONTENT - 1,
            None,
        );

        if self.visible_rows.is_empty() {
            return;
        }

        // --- State extraction (one borrow, drop) ---
        // Per-visible-row queries (is_section, is_selected, is_focused)
        // are precomputed under the borrow so paint loops below don't
        // re-enter the RefCell.
        let core = self.core.borrow();
        let row_height = core.cms.hts().row_height();
        let tree_col_id = core.tree_column_id;
        let slots: Vec<_> = core.cms.hts().column_layout().body_slots().collect();
        let focused = core.focused;
        let row_meta: Vec<(PropertyNodeKind, bool, bool)> = self
            .visible_rows
            .iter()
            .map(|vr| {
                // Archetype is canonical here — paint sites derive both
                // the "is this a band?" predicate and the "which band
                // color?" choice from `kind`. Default to Data when the
                // row is absent from property_nodes (data not yet
                // loaded or stale id).
                let kind = core
                    .property_nodes
                    .get(&vr.node_id)
                    .map_or(PropertyNodeKind::Data, |n| n.kind);
                let is_selected = core.selection.is_selected(vr.node_id);
                let is_focused = core.selection.is_focused(vr.node_id);
                (kind, is_selected, is_focused)
            })
            .collect();
        // Snapshot `hovered_row` here (rather than only in the B3b
        // second-borrow block below) so pass 2 can paint
        // `band_hover_overlay` on the matching band row. T6 introduced
        // `hovered_row` as the whole-band hover signal, distinct from
        // per-cell `hovered_cell` (the latter snapshots in the B3b
        // block below for Data-row cell hover paint).
        let hovered_band_row = core.hovered_row;
        drop(core);

        let base_x = rect.x;
        let snapped_y = snap_to_physical(rect.y, sf);
        let first_sorted = self.first_sorted;
        // INV: canonical row-Y derivation. Every per-row primitive uses
        // this — no per-pass drift, no visible-index-only variant.
        let row_y_abs = |vi: usize| -> f32 { snapped_y + (first_sorted + vi) as f32 * row_height };

        let list_colors = &theme.colors.list;
        let tree_slot = slots
            .iter()
            .find(|s| Some(s.column_id) == tree_col_id)
            .copied();

        // SPEC-HAL-260A §14.3 pass 2: per-row archetype paint. The only
        // structural row-level fork in the paint pipeline
        // (INV-CELL-PAINT-UNIFORM, §9.1). Bands paint their full content
        // here — background, overlays, AND label — and skip pass 4
        // (the per-cell loop, Data rows only). Data rows paint only the
        // selection background here; their tree-cell label and Data-cell
        // content live in pass 4.
        //
        //  - Section    → `header_background` (prominent band) +
        //                 band label (full-band-width clip).
        //  - SubSection → `subheader_background` (subordinate band) +
        //                 band label (full-band-width clip).
        //  - Data       → `row_selected[_inactive]` when selected;
        //                 otherwise no paint (default row_background
        //                 from pass 1 shows through).
        //
        // Band overlays layered on top of the base band color at the
        // same Z (LAYER_CONTENT - 1), insertion order:
        //
        //  - `band_hover_overlay` when the pointer hovers the band
        //    (whole-band hover keyed by `core.hovered_row`, T6).
        //  - `band_focused_overlay` when PE focus is on this band
        //    (`PeFocus::Band` row matches `selection.focus_key`) AND
        //    PE Outer has focus. Distinct from data-row selection
        //    blue: bands use monochrome modulation per the band-paint
        //    feedback design (§5.5.1).
        //
        // INV-PE-CELL-FOCUS-ONLY (§9.1): no row-spanning cell focus
        // border. Cell focus is the per-cell border in pass 4c
        // (LAYER_FOCUS); bands have no per-cell focus — their
        // `focused_overlay` IS the active-state indicator.
        let hovered_row: Option<u64> = hovered_band_row;
        for (i, vr) in self.visible_rows.iter().enumerate() {
            let (kind, is_selected, is_focused_row) = row_meta[i];
            let row_y = row_y_abs(i);
            let row_rect = RectF::new(base_x, row_y, row_bg_width, row_height);
            let row_id = vr.node_id;

            match kind {
                PropertyNodeKind::Section => {
                    cx.scene.push_rect(
                        row_rect,
                        list_colors.header_background,
                        LAYER_CONTENT - 1,
                        None,
                    );
                    if hovered_row == Some(row_id) {
                        cx.scene.push_rect(
                            row_rect,
                            list_colors.band_hover_overlay,
                            LAYER_CONTENT - 1,
                            None,
                        );
                    }
                    if is_focused_row && focused {
                        cx.scene.push_rect(
                            row_rect,
                            list_colors.band_focused_overlay,
                            LAYER_CONTENT - 1,
                            None,
                        );
                    }
                }
                PropertyNodeKind::SubSection => {
                    cx.scene.push_rect(
                        row_rect,
                        list_colors.subheader_background,
                        LAYER_CONTENT - 1,
                        None,
                    );
                    if hovered_row == Some(row_id) {
                        cx.scene.push_rect(
                            row_rect,
                            list_colors.band_hover_overlay,
                            LAYER_CONTENT - 1,
                            None,
                        );
                    }
                    if is_focused_row && focused {
                        cx.scene.push_rect(
                            row_rect,
                            list_colors.band_focused_overlay,
                            LAYER_CONTENT - 1,
                            None,
                        );
                    }
                }
                PropertyNodeKind::Data => {
                    if is_selected {
                        let sel_color = if focused {
                            list_colors.row_selected
                        } else {
                            list_colors.row_selected_inactive
                        };
                        cx.scene
                            .push_rect(row_rect, sel_color, LAYER_CONTENT - 1, None);
                    }
                    // Data-row focus overlay — whole-row signal for
                    // keyboard-navigation orientation. Painted only when
                    // the row holds PE focus AND PE Outer has focus
                    // (matches `band_focused_overlay` gating). Layered
                    // after the selection bg at the same Z so the
                    // overlay modulates both selected and unselected
                    // focused rows in insertion order. Pairs with the
                    // per-cell focus border at pass 4c — the overlay
                    // is the row-axis signal, the cell border is the
                    // column-axis signal.
                    if is_focused_row && focused {
                        cx.scene.push_rect(
                            row_rect,
                            list_colors.row_focused_overlay,
                            LAYER_CONTENT - 1,
                            None,
                        );
                    }
                }
            }

            // SPEC-HAL-260A §14.3 pass 2: band label. Folded into the
            // same iteration as background + overlays so Section /
            // SubSection rows are fully painted in pass 2 and skip
            // pass 4 entirely. Full-band-width clip — the band reads
            // as a continuous strip with no truncation at the
            // tree-column edge. Bands are not selectable per §5.5.1,
            // so the label color is always `list_colors.text`.
            if matches!(
                kind,
                PropertyNodeKind::Section | PropertyNodeKind::SubSection
            ) {
                if let Some(ts) = tree_slot {
                    let tree_col_x = snap_to_physical(base_x + ts.x, sf);
                    let label_clip_w = (row_bg_width - (tree_col_x - base_x)).max(0.0);
                    let label_rect = RectF::new(tree_col_x, row_y, label_clip_w, row_height);
                    cx.scene.begin_clip(label_rect, None);
                    let indent = vr.depth as f32 * TREE_INDENT_STEP;
                    let text_x =
                        tree_col_x + indent + TREE_CONTROL_WIDTH + theme.list_geom.padding_x;
                    let max_w =
                        label_clip_w - indent - TREE_CONTROL_WIDTH - theme.list_geom.padding_x;
                    if max_w > 0.0 && !vr.label.is_empty() {
                        let run = cx.text_engine.shape(&vr.label, font_size);
                        let metrics = cx.text_engine.font_metrics(font_size);
                        let baseline_y = row_y
                            + compute_control_text_baseline_y(
                                row_height,
                                metrics.ascent,
                                metrics.descent,
                                theme.typography.baseline_offset,
                            );
                        let text_y = baseline_y - metrics.ascent;
                        cx.text_engine.draw(
                            &run,
                            text_x,
                            text_y,
                            list_colors.text,
                            LAYER_CONTENT,
                            cx.scene,
                        );
                    }
                    cx.scene.end_clip();
                }
            }
        }

        // ====================================================================
        // SPEC-HAL-260A §14.3 pass 4: unified per-cell loop (Data rows
        // only). Passes 5 + 6: column separators (subtle / hovered+active).
        //
        // Second borrow scoped to pass-4 reads. `data_source` is cloned
        // (Rc clone — cheap) so per-cell `cell()` lookups happen without
        // holding the core borrow.
        // ====================================================================
        // Hold `core` across the per-cell loop: paint reads cell content
        // from `core.cms.realized_cell_values` and that borrow must remain
        // live for the duration of the read loop. Paint is `&self`, so
        // no mutable re-borrow exists in this scope.
        let core = self.core.borrow();
        let column_kinds: HashMap<ColumnId, ColumnKind> = core.column_kinds.clone();
        let focused_column = core.cms.focus().col_id();
        let focus_key = core.selection.focused_key();
        let edit_session_cell = core.cms.edit_session().map(|es| (es.row_id, es.column_id));
        let cms_current_edit_cell = core.cms.current_edit_cell();
        let hovered_cell = core.hovered_cell;
        let hovered_sep = core.hovered_separator;
        let mut integrated_edges = Vec::new();
        let active_sep = core.active_resize_separator;
        // SPEC-PM-017: f32 shim — PE rung.
        let scroll_y = core.cms.hts().scroll_offset() as f32;
        let sep_slots: Vec<_> = core.cms.hts().column_layout().header_slots().collect();

        // SPEC-HAL-260A §14.3: cell text insets per cell-kind.
        //
        //  - Data-kind cells use `theme.geometry.text_field.padding_x` to
        //    match editor children mounted in the same cell rect
        //    (TextField, ComboBox embedded TextField, KeybindEditor).
        //    Aligning inline drawing to the editor-side inset eliminates
        //    the horizontal jump between display and edit states.
        //    Compact density: 3.0 (text_field).
        //  - Tree-kind cells use `theme.list_geom.padding_x` (6.0). No
        //    editor mounts in the tree column. The pass-2 band-label
        //    paint uses the same value for consistency.
        let tree_text_padding = theme.list_geom.padding_x;

        // --------------------------------------------------------------------
        // SPEC-HAL-260A §14.3 pass 4: unified per-cell loop (Data rows
        // only). Section / SubSection content is fully painted in pass 2.
        //
        // INV-CELL-PAINT-UNIFORM (§9.1) + FORBID-PE-COLUMN-ID-CARVE-OUT
        // (§9.2): iterates every column slot uniformly — no `continue`
        // on a specific ColumnId at this seam — and dispatches cell
        // content by the column's registered `ColumnKind` inside the
        // loop body. Hover (4a), kind-dispatched content (4b), and cell
        // focus border (4c) all live in the same iteration.
        //
        // Tree-kind sub-dispatch: today's `ColumnKind::NonData` has one
        // registered member (the tree column, identified by id-equality
        // INSIDE the kind branch — intra-kind disambiguation, not a
        // structural seam). Future NonData kinds (Indicator, Action, …)
        // dispatch alongside without restructuring the outer match.
        // --------------------------------------------------------------------
        for (i, vr) in self.visible_rows.iter().enumerate() {
            let (kind, is_selected, _is_focused) = row_meta[i];
            // Pass 4 is Data only — bands paint their full content in pass 2.
            if !matches!(kind, PropertyNodeKind::Data) {
                continue;
            }
            let row_y = row_y_abs(i);
            let row_id = vr.node_id;

            for slot in &slots {
                let cell_x = snap_to_physical(base_x + slot.x, sf);
                let col_sep_x = snap_to_physical(cell_x + slot.width - 1.0, sf);
                let cell_clip_w = (col_sep_x - cell_x).max(0.0);
                let cell_rect = RectF::new(cell_x, row_y, cell_clip_w, row_height);

                // 4a — hover overlay. Uniform across every cell-kind of
                // every Data row. The tree-cell hover gap (carve-out
                // before this refactor) closes here as a natural
                // consequence of the unified loop: pointer over the
                // tree-col cell sets `core.hovered_cell` to the tree
                // (row, col_id) pair, and this branch paints the overlay.
                if hovered_cell == Some((row_id, slot.column_id)) {
                    cx.scene
                        .push_rect(cell_rect, list_colors.row_hover, LAYER_CONTENT, None);
                }

                // Per-slot editor / Bool predicates. Hoisted from 4b's
                // Data-kind arm so 4c (cell focus border) can read
                // them for INV-PE-CELL-FOCUS-ONLY (§9.1) suppression:
                // when an editor child or the always-mounted Bool
                // Checkbox owns the cell, the child paints its own
                // focus ring (per SPEC-HAL-328 §12.3) and PE emits no
                // per-cell border to avoid stacking two indicators on
                // the same cell. `data_source.cell()` returns `None`
                // for non-Data columns, so both predicates are `false`
                // for the tree-kind slot — its focus border continues
                // to paint normally.
                //
                // INV-PE-TREE-DEFERRED gap: between a commit/cancel
                // event and the next layout pass that unmounts the
                // editor child, `edit_session_cell` reads `None` but
                // `current_edit_cell` still names the cell — and
                // `paint_node` will still recurse into the mounted
                // editor child. `is_edit_cell` OR's the two so both
                // inline content (4b's Data arm) AND the cell focus
                // border (4c) are suppressed for the entire gap.
                //
                // SPEC-PM-016-R1/R2: cell content read from the CMS
                // realized-cell cache populated by layout; never
                // re-queried from the source during paint.
                let cell_opt: Option<&Cell> = core.cms.realized_cell_value(row_id, slot.column_id);
                let is_edit_cell = edit_session_cell == Some((row_id, slot.column_id))
                    || cms_current_edit_cell == Some((row_id, slot.column_id));
                let is_bool_cell =
                    matches!(cell_opt.map(|c| &c.value), Some(PropertyValue::Bool(_)));

                // 4b — cell content, dispatched on the column's
                // registered `ColumnKind`. The kind-dispatch seam.
                match column_kinds.get(&slot.column_id).copied() {
                    Some(ColumnKind::NonData) if Some(slot.column_id) == tree_col_id => {
                        // Tree-kind cell: depth indent + reserved
                        // TREE_CONTROL_WIDTH for the disclosure-toggle
                        // child widget + label clipped to the cell rect.
                        // The disclosure widget paints itself at the
                        // same X band; Body reserves the width here but
                        // does not render the indicator.
                        //
                        // SPEC-HAL-092 §8A: control text uses
                        // ascent+descent baseline so the label aligns
                        // vertically with editor children mounted in
                        // adjacent Data cells.
                        cx.scene.begin_clip(cell_rect, None);
                        let text_color = if is_selected {
                            list_colors.text_selected
                        } else {
                            list_colors.text
                        };
                        let indent = vr.depth as f32 * TREE_INDENT_STEP;
                        let text_x = cell_x + indent + TREE_CONTROL_WIDTH + tree_text_padding;
                        let max_w = cell_clip_w - indent - TREE_CONTROL_WIDTH - tree_text_padding;
                        if max_w > 0.0 && !vr.label.is_empty() {
                            let run = cx.text_engine.shape(&vr.label, font_size);
                            let metrics = cx.text_engine.font_metrics(font_size);
                            let baseline_y = row_y
                                + compute_control_text_baseline_y(
                                    row_height,
                                    metrics.ascent,
                                    metrics.descent,
                                    theme.typography.baseline_offset,
                                );
                            let text_y = baseline_y - metrics.ascent;
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
                    }
                    Some(ColumnKind::Data) => {
                        // Data-kind cell: per-variant paint dispatched by
                        // CMS. Inline paint is suppressed when the cell
                        // hosts an editor child or the always-mounted
                        // Bool Checkbox — `is_edit_cell` / `is_bool_cell`
                        // gate both this call and the cell focus border
                        // at 4c (INV-PE-CELL-FOCUS-ONLY §9.1).
                        if !is_edit_cell && !is_bool_cell {
                            if let Some(cell) = cell_opt {
                                core.cms.paint_one_cell(cx, cell, cell_rect, is_selected);
                            }
                        }
                    }
                    Some(ColumnKind::NonData) | None => {
                        // Future NonData kinds (Indicator, Action, …)
                        // dispatch here when introduced. `None`: an
                        // unregistered column id — skipped silently.
                    }
                }

                // 4c — cell focus border at LAYER_FOCUS.
                // INV-PE-CELL-FOCUS-ONLY (§9.1): exactly one focus
                // indicator per cursor state. When the cell hosts an
                // editor child (`is_edit_cell`) or the always-mounted
                // Bool Checkbox (`is_bool_cell`), the child paints
                // its own focus ring per SPEC-HAL-328 §12.3 — PE
                // emits no per-cell border to avoid stacking two
                // indicators on the same cell. Otherwise: a single
                // cell-sized rectangle at the focused cell, both X
                // edges already snapped to physical pixel
                // (cell_x, col_sep_x). Tree-kind cells receive the
                // same indicator as Data-kind cells — the indicator
                // is kind-agnostic. Painted at LAYER_FOCUS which
                // renders above the column-separator passes
                // (LAYER_CONTENT + 1 / + 2) emitted later in this
                // paint pass, so the rectangle is unbroken regardless
                // of paint order.
                let integrated = cx.has_cell_editor_edge(row_id, slot.column_id);
                if integrated {
                    let right = snap_to_physical(base_x + slot.x + slot.width, sf);
                    integrated_edges.push((row_id, slot.column_id,
                        RectF::new(cell_x, row_y, right - cell_x, row_height), is_selected,
                        hovered_cell == Some((row_id, slot.column_id))));
                }
                if !integrated && Some(slot.column_id) == focused_column
                    && Some(row_id) == focus_key
                    && !is_edit_cell
                    && !is_bool_cell
                {
                    cx.scene.push_border_rect(
                        cell_rect,
                        theme.colors.borders.focus,
                        1.0,
                        LAYER_FOCUS,
                        None,
                    );
                }
            }
        }

        // --------------------------------------------------------------------
        cx.paint_cell_editor_edges(integrated_edges.iter().copied());

        // Column separators — SPEC-HAL-260A §14.3 passes 5 + 6.
        //
        // Two passes, both iterate inter-column slots (skip last — no
        // separator after the rightmost column). Pass 5 paints subtle 1px
        // separators at LAYER_CONTENT + 1 for non-hovered, non-active
        // columns. Pass 6 paints 2px focus / default separators at
        // LAYER_CONTENT + 2 for the active or hovered column.
        //
        // SPEC-PM-007: passes strictly above backgrounds to avoid
        // clip-hash ordering flicker.
        //
        // Separator X uses content-space snap; ScrollNode applies the
        // horizontal scroll offset. Y derived as rect.y + scroll_y so
        // separators always span the visible viewport regardless of
        // scroll position (TreeView / old PE pattern).
        //
        // SPEC §10.2: section rows are full-width strip headers with no
        // per-column content. The separator MUST skip their Y bands so
        // the strips read as continuous gray bands, not carved-up rows.
        // The layer-lifting alternative (paint section bg above
        // separators) is forbidden because DisclosureToggle paints at
        // LAYER_CONTENT and would be hidden by any section bg painted
        // higher than the separators. Geometry fix only.
        // --------------------------------------------------------------------
        {
            let inter_column_slots = &sep_slots[..sep_slots.len().saturating_sub(1)];

            // Build the Y segments to paint for each separator. Start
            // with the full viewport Y range, subtract section row Y
            // bands. visible_rows is in render order (top→bottom), so
            // section bands appear in Y-ascending order without sorting.
            //
            // No core publication. PE Outer's `hit_test_separator`
            // calls `core.is_section_at_body_y` which derives the same
            // predicate on demand from `hts` + `is_section_row`. Paint
            // and hit-test share the predicate primitives, not a
            // stored intermediate.
            let sep_y_start = rect.y + scroll_y;
            let sep_y_end = sep_y_start + rect.height;

            let section_bands: Vec<(f32, f32)> = self
                .visible_rows
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    matches!(
                        row_meta[*i].0,
                        PropertyNodeKind::Section | PropertyNodeKind::SubSection
                    )
                })
                .map(|(i, _)| {
                    let y = row_y_abs(i);
                    (y, y + row_height)
                })
                .collect();

            let mut sep_segments: Vec<(f32, f32)> = Vec::new();
            {
                let mut cursor = sep_y_start;
                for (band_start, band_end) in &section_bands {
                    if *band_end <= cursor {
                        continue; // section entirely above cursor (shouldn't
                        // happen given visible_rows order, but
                        // skip defensively)
                    }
                    if *band_start > cursor {
                        sep_segments.push((cursor, *band_start));
                    }
                    if *band_end > cursor {
                        cursor = *band_end;
                    }
                }
                if cursor < sep_y_end {
                    sep_segments.push((cursor, sep_y_end));
                }
            }

            for slot in inter_column_slots {
                let is_active = active_sep == Some(slot.column_id);
                let is_hovered = hovered_sep == Some(slot.column_id);
                if is_active || is_hovered {
                    continue;
                }
                let sep_x = snap_to_physical(base_x + slot.x + slot.width - 1.0, sf);
                for (seg_y, seg_end) in &sep_segments {
                    cx.paint_cell_grid_separator(
                        RectF::new(sep_x, *seg_y, 1.0, seg_end - seg_y),
                        theme.colors.borders.subtle,
                        LAYER_CONTENT + 1,
                        &integrated_edges,
                    );
                }
            }

            for slot in inter_column_slots {
                let is_active = active_sep == Some(slot.column_id);
                let is_hovered = hovered_sep == Some(slot.column_id);
                if !is_active && !is_hovered {
                    continue;
                }
                let sep_x = snap_to_physical(base_x + slot.x + slot.width - 1.0, sf);
                let (sep_w, sep_color) = if is_active {
                    (2.0, theme.colors.borders.focus)
                } else {
                    (2.0, theme.colors.borders.default)
                };
                for (seg_y, seg_end) in &sep_segments {
                    cx.paint_cell_grid_separator(
                        RectF::new(sep_x, *seg_y, sep_w, seg_end - seg_y),
                        sep_color,
                        LAYER_CONTENT + 2,
                        &integrated_edges,
                    );
                }
            }
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Phase filter: Target-only for non-drag events. During an active
        // drag, the pointer is captured by this widget and Move/Up arrive
        // in Capture phase — those must pass through too. B7c will add
        // DoubleClick (Target-only).
        let drag_active = self.core.borrow().selection_drag_active;
        if phase != EventPhase::Target && !(drag_active && phase == EventPhase::Capture) {
            return false;
        }

        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    let Some(rect) = cx.widget_global_rect() else {
                        return false;
                    };
                    let local_x = pe.global_pos.0 - rect.x;

                    // INV-RESIZE-ARM-FORWARDING: clicks on column
                    // separators bubble to outer/header for resize arming.
                    if self.is_on_separator(local_x) {
                        return false;
                    }

                    let local_y = pe.global_pos.1 - rect.y;
                    let vi = match self.hit_test_row(local_y) {
                        Some(vi) => vi,
                        None => return false,
                    };
                    let node_id = self.visible_rows[vi].node_id;

                    // SPEC-HAL-331 §6.2 click-routing precedence rule.
                    // Compute cell hit once; reused by both the precedence
                    // check below and the plain-click branch's column
                    // dispatch (which previously called hit_test_cell at
                    // line 1115 — that call is removed in this edit).
                    let cell_hit = self.hit_test_cell(local_x, local_y);

                    // INV-331-7: cells in edit state suppress PE Body's
                    // pointer dispatch on their bounds. Read the session
                    // snapshot under a short borrow so the cross-cell
                    // Cancel below can take a fresh borrow_mut without
                    // aliasing.
                    let edit_session_snapshot = self
                        .core
                        .borrow()
                        .cms
                        .edit_session()
                        .map(|s| (s.row_id, s.column_id));
                    if let Some((sess_row, sess_col)) = edit_session_snapshot {
                        let same_cell = cell_hit
                            .map(|(r, c)| r == sess_row && c == sess_col)
                            .unwrap_or(false);
                        if same_cell {
                            // INV-331-7: editor owns clicks on its own
                            // bounds. The event was already delivered to
                            // the editor child (deeper hit target) before
                            // bubbling here; PE Body emits nothing.
                            return false;
                        }
                        // Cross-cell click while edit_session active.
                        // INV-PE-SINGLE-EDIT (body.rs:550-553) requires
                        // the active session to be cleared before any
                        // new-cell Selection / Edit dispatch. SPEC-HAL-260A
                        // §11 click_elsewhere_disposition is hardcoded to
                        // Cancel here pending force_commit infrastructure
                        // (SPEC-HAL-331 followup task).
                        self.core.borrow_mut().dispatch_intent(PEIntent::Edit(
                            EditIntent::Cancel {
                                row_id: sess_row,
                                column_id: sess_col,
                            },
                        ));
                    }

                    if pe.modifiers.shift {
                        // Shift+Click: commit the range immediately on press
                        // through the intent funnel, then keep the drag armed
                        // so a subsequent move keeps extending from the same
                        // anchor. ExtendTo is replacing — it recomputes
                        // [anchor..target] from the persistent anchor on every
                        // call, so the press-time commit plus drag re-apply
                        // cannot double-count.
                        let mut core = self.core.borrow_mut();
                        core.selection_drag_anchor = Some(node_id);
                        core.selection_drag_active = true;
                        core.selection_drag_last = Some(node_id);
                        core.dispatch_intent(PEIntent::Selection(
                            OrderedSelectionAction::ExtendTo { key: node_id },
                        ));
                        drop(core);
                        cx.capture_pointer(PointerButton::Primary);
                        cx.invalidate_layout();
                    } else {
                        // Plain or Ctrl/Meta click. Clear any prior drag
                        // state (defensive — should already be clear),
                        // then dispatch Select or Toggle through the
                        // intent funnel (INV-PE-INTENT-FUNNEL — the
                        // permission gate runs first).
                        let action = if pe.modifiers.ctrl || pe.modifiers.meta {
                            OrderedSelectionAction::Toggle { key: node_id }
                        } else {
                            OrderedSelectionAction::Select { key: node_id }
                        };

                        // SPEC-HAL-260A INV-CELL-2 (pointer pathway):
                        // pointer clicks are row-axis focus changes; the
                        // invariant requires that after any such change,
                        // `focused_column` is a member of
                        // `navigable_columns_for_row(new_row)`. Identify
                        // the clicked column via `hit_test_cell` BEFORE
                        // taking the `core.borrow_mut()` below, so the
                        // internal `core.borrow()` inside `hit_test_cell`
                        // does not conflict with the outer mut borrow.
                        // The cached `local_x` / `local_y` here are the
                        // same viewport-local coordinates already
                        // computed for `hit_test_row` (lines 1060, 1068).
                        // Cell hit-test returning None (click landed in
                        // horizontal gap past the last column) leaves
                        // `focused_column` at whatever it was — matches
                        // the keyboard pathway's "no change when no
                        // navigable target" behavior.
                        let cell_col = cell_hit.map(|(_, c)| c);

                        let mut core = self.core.borrow_mut();
                        core.selection_drag_anchor = None;
                        core.selection_drag_active = false;
                        core.selection_drag_last = None;
                        core.dispatch_intent(PEIntent::Selection(action));
                        if let Some(column_id) = cell_col {
                            core.dispatch_intent(PEIntent::Column(ColumnIntent::To { column_id }));
                        }
                        drop(core);
                        cx.invalidate_layout();
                    }
                    true
                }
                PointerEventKind::Move => {
                    // Drag-active: route to drag-extend. Otherwise: hover
                    // routing (B7a).
                    let (move_drag_active, anchor_opt) = {
                        let core = self.core.borrow();
                        (core.selection_drag_active, core.selection_drag_anchor)
                    };

                    if move_drag_active {
                        if anchor_opt.is_none() {
                            return false;
                        }
                        let Some(rect) = cx.widget_global_rect() else {
                            return false;
                        };
                        let local_y = pe.global_pos.1 - rect.y;
                        if let Some(vi) = self.hit_test_row(local_y) {
                            let current_id = self.visible_rows[vi].node_id;
                            // Only dispatch when the pointer reaches a row
                            // different from the last one. ExtendTo is
                            // replacing — it recomputes [anchor..target] from
                            // the persistent anchor on every call, so the
                            // press-time commit plus re-apply here cannot
                            // double-count.
                            if self.core.borrow().selection_drag_last != Some(current_id) {
                                let mut core = self.core.borrow_mut();
                                core.selection_drag_last = Some(current_id);
                                core.dispatch_intent(PEIntent::Selection(
                                    OrderedSelectionAction::ExtendTo { key: current_id },
                                ));
                                drop(core);
                                cx.invalidate_layout();
                            }
                        }
                        true
                    } else {
                        // B7a hover routing. Resolves the pointer to at
                        // most one signal — `hovered_cell` for Data-row
                        // cell hits, `hovered_row` for whole-band hits
                        // on Section / SubSection rows. Mutually
                        // exclusive: each PointerMove sets one and
                        // clears the other, so paint never sees both
                        // signals active.
                        let Some(rect) = cx.widget_global_rect() else {
                            return false;
                        };
                        let local_x = pe.global_pos.0 - rect.x;
                        let local_y = pe.global_pos.1 - rect.y;

                        let (new_hovered_cell, new_hovered_row) = {
                            let cell_hit = self.hit_test_cell(local_x, local_y);
                            if cell_hit.is_some() {
                                (cell_hit, None)
                            } else if let Some(vi) = self.hit_test_row(local_y) {
                                let node_id = self.visible_rows[vi].node_id;
                                let is_band = self.core.borrow().is_section_row(node_id);
                                if is_band {
                                    (None, Some(node_id))
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            }
                        };

                        let mut core = self.core.borrow_mut();
                        if core.hovered_cell != new_hovered_cell
                            || core.hovered_row != new_hovered_row
                        {
                            core.hovered_cell = new_hovered_cell;
                            core.hovered_row = new_hovered_row;
                            drop(core);
                            cx.invalidate_layout();
                        }
                        false
                    }
                }
                PointerEventKind::Up => {
                    if !drag_active {
                        return false;
                    }

                    // Range gesture: selection was committed on press and
                    // during any drag-move. Just clear drag state.
                    let mut core = self.core.borrow_mut();
                    core.selection_drag_anchor = None;
                    core.selection_drag_active = false;
                    core.selection_drag_last = None;
                    drop(core);

                    cx.release_pointer();
                    cx.invalidate_layout();
                    true
                }
                PointerEventKind::DoubleClick => {
                    // SPEC §10.5: activation behavior. Forks on row
                    // archetype:
                    // - Band (Section / SubSection) + row has children
                    //   → ToggleExpand via the intent funnel. The
                    //   pointer's X position within the band is
                    //   irrelevant — bands activate as whole-row units.
                    // - Data + NonData column (tree column) + row has
                    //   children → ToggleExpand. Matches the band
                    //   behavior but localized to the tree-column area
                    //   on Data rows.
                    // - Data + Data column + cell editable + value not
                    //   Bool / ReadOnly → EditIntent::Begin through the
                    //   funnel. Permission gate runs; on accept,
                    //   apply_edit_intent::Begin sets edit_session and
                    //   B6's reconciliation mounts the editor next pass.
                    if phase != EventPhase::Target {
                        return false;
                    }
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    let Some(rect) = cx.widget_global_rect() else {
                        return false;
                    };
                    let local_x = pe.global_pos.0 - rect.x;
                    if self.is_on_separator(local_x) {
                        return false;
                    }
                    let local_y = pe.global_pos.1 - rect.y;

                    // Resolve the row first. Bands activate as whole
                    // units (no column-level resolution needed); Data
                    // rows fall through to the cell-resolution path
                    // below.
                    let vi = match self.hit_test_row(local_y) {
                        Some(vi) => vi,
                        None => return false,
                    };
                    let node_id_row = self.visible_rows[vi].node_id;
                    let has_children_row = self.visible_rows[vi].has_children;
                    let is_band = self.core.borrow().is_section_row(node_id_row);

                    if is_band {
                        if !has_children_row {
                            return false;
                        }
                        let row_index_opt = self.core.borrow().cms.hts().index_of(node_id_row);
                        if let Some(row_index) = row_index_opt {
                            self.core.borrow_mut().dispatch_intent(PEIntent::Tree(
                                TreeIntent::ToggleExpand { row_index },
                            ));
                        }
                        cx.invalidate_layout();
                        return true;
                    }

                    // Data row: resolve to a specific cell for the
                    // tree-column-expand vs data-column-edit branch
                    // below.
                    let (node_id, col_id) = match self.hit_test_cell(local_x, local_y) {
                        Some(pair) => pair,
                        None => return false,
                    };

                    // SPEC-HAL-331 §6.2 / INV-331-7: DoubleClick on a cell
                    // in edit state is the editor's gesture (e.g. text
                    // field word-selection), not a re-trigger of
                    // edit-begin. PE Body suppresses.
                    //
                    // Different-cell DoubleClick during an active session
                    // is unreachable in practice — the preceding
                    // PointerDown already issued the cross-cell Cancel
                    // (see Edit 1) and apply_edit_intent::Cancel cleared
                    // edit_session before DoubleClick fires. Defensive
                    // guard remains in case the permission gate denies
                    // the Cancel: emit Cancel again so INV-PE-SINGLE-EDIT
                    // is not violated by the subsequent Begin dispatch
                    // below.
                    let edit_session_snapshot = self
                        .core
                        .borrow()
                        .cms
                        .edit_session()
                        .map(|s| (s.row_id, s.column_id));
                    if let Some((sess_row, sess_col)) = edit_session_snapshot {
                        if sess_row == node_id && sess_col == col_id {
                            return false;
                        }
                        self.core.borrow_mut().dispatch_intent(PEIntent::Edit(
                            EditIntent::Cancel {
                                row_id: sess_row,
                                column_id: sess_col,
                            },
                        ));
                    }

                    let has_children = self
                        .visible_rows
                        .iter()
                        .find(|vr| vr.node_id == node_id)
                        .map(|vr| vr.has_children)
                        .unwrap_or(false);
                    let column_kind = self.core.borrow().column_kinds.get(&col_id).copied();
                    match column_kind {
                        Some(ColumnKind::NonData) => {
                            if !has_children {
                                return false;
                            }
                            // INV-PE-INTENT-FUNNEL: route topology
                            // mutation through `dispatch_intent` so the
                            // permission gate (Tree action, no column)
                            // runs for tree-cell DoubleClick. Resolve
                            // node_id → row_index dynamically; if the
                            // row is no longer in the sorted set (stale
                            // hit-test result) the dispatch is skipped.
                            let row_index_opt = self.core.borrow().cms.hts().index_of(node_id);
                            if let Some(row_index) = row_index_opt {
                                self.core.borrow_mut().dispatch_intent(PEIntent::Tree(
                                    TreeIntent::ToggleExpand { row_index },
                                ));
                            }
                            cx.invalidate_layout();
                            true
                        }
                        Some(ColumnKind::Data) => {
                            // Determine eligibility: cell exists, is
                            // editable, value is not Bool (always-mounted
                            // Checkbox owns Bool toggle per §6.2) and not
                            // ReadOnly (structurally non-editable). The
                            // permission gate runs inside dispatch_intent;
                            // we do not pre-check it.
                            let editable_non_bool = {
                                let core = self.core.borrow();
                                let ds = core.data_source.clone();
                                drop(core);
                                ds.as_ref()
                                    .and_then(|ds| {
                                        ds.borrow().cell(node_id, col_id).map(|c| {
                                            c.editable
                                                && !matches!(
                                                    c.value,
                                                    PropertyValue::Bool(_)
                                                        | PropertyValue::ReadOnly(_)
                                                )
                                        })
                                    })
                                    .unwrap_or(false)
                            };
                            if !editable_non_bool {
                                return false;
                            }
                            self.core.borrow_mut().dispatch_intent(PEIntent::Edit(
                                EditIntent::Begin {
                                    row_id: node_id,
                                    column_id: col_id,
                                },
                            ));
                            cx.invalidate_layout();
                            true
                        }
                        None => false,
                    }
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// SPEC-PM-002 §10 contract: sync ScrollArea's scroll position into
    /// HTS so `core.cms.hts().scroll_offset()` reflects the live viewport
    /// state during hit-test and visible-row enumeration.
    ///
    /// Dispatched by `app.rs` pre_layout_sync_scroll_to_lists /
    /// post_layout_sync_scroll_to_lists. The ScrollArea ancestor is
    /// detected as a scroll anchor automatically (widget_tree.rs
    /// scroll_anchor registration); PropertyEditorBody receives
    /// sync_scroll as a ScrollArea descendant. Without this override
    /// the Widget-trait default no-ops, HTS.scroll_y stays at 0, and
    /// `hit_test_row` treats viewport-local Y as content-Y — clicks
    /// after scroll resolve to rows above the actual pointer position.
    ///
    /// Returns true if the offset changed (caller marks layout dirty).
    /// Mirror of `TreeViewBody::sync_scroll` at
    /// `src/widgets/tree_view/body.rs:910-918`.
    fn sync_scroll(&mut self, scroll_y: f32) -> bool {
        let core = self.core.borrow_mut();
        let prev = core.cms.hts().scroll_offset();
        // SPEC-PM-017: the f32 inbound offset is a rung-3 limitation
        // (ScrollArea core is still f32); widen at the boundary.
        if (scroll_y as f64 - prev).abs() < 0.01 {
            return false;
        }
        core.cms.hts_mut().scroll_to(scroll_y as f64);
        true
    }

    /// SPEC-HAL-038 Stage 7: pointer left this widget's bounds (moved to
    /// a different widget or out of the window). Clears both the
    /// per-cell hover signal (`hovered_cell`, Data-row cells) and the
    /// whole-row band hover signal (`hovered_row`, Section / SubSection
    /// rows) so all hover-state paint stops. NOT called when the pointer
    /// leaves via PointerDown on this widget (capture takes over) —
    /// that case is handled in B7b.
    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        let mut core = self.core.borrow_mut();
        if core.hovered_cell.is_some() || core.hovered_row.is_some() {
            core.hovered_cell = None;
            core.hovered_row = None;
            drop(core);
            cx.invalidate_layout();
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn needs_viewport_constraint(&self) -> bool {
        // Body is a content widget inside ScrollArea; needs the viewport
        // height to drive HTS visible-row enumeration in B2.
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "PropertyEditorBody"
    }

    fn clip_bounds(&self) -> Option<RectF> {
        // SPEC-HAL-260A: Body has three categories of children —
        // disclosure toggles (tree column), always-mounted Bool
        // Checkboxes (§6.1, Data columns), and inline editors (§6.3,
        // Data columns). A widget-wide clip cannot serve all three.
        //
        // Disclosure-toggle horizontal containment is provided by the
        // eligibility filter at the disclosure-reconciliation site
        // (layout step 4): a toggle is filtered out before mount
        // whenever `bx + TREE_CONTROL_WIDTH > effective_right`. That
        // filter is the primary defense; the previous body-wide
        // `tree_col_clip` was a redundant secondary defense whose
        // destructive side effect was to suppress the §6.1 / §6.3
        // children that live outside the tree column.
        //
        // `self.tree_col_clip` is still populated in layout step 7 in
        // case a future per-child clip site needs it; intentionally
        // not returned here.
        None
    }
}

impl WidgetDowncast for PropertyEditorBody {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Map a substrate edit-outcome to the PE intent that applies it. Mirrors
/// the routing the edit-session editor callbacks used before the substrate
/// owned the edit path: Commit / CommitAndAdvance carry the new value;
/// Cancel and ForcedCancel discard. A Commit / CommitAndAdvance with no
/// value, or any other combination, yields no intent.
fn pe_intent_from_edit_outcome(outcome: EditOutcome) -> Option<PEIntent> {
    let EditOutcome {
        row,
        col,
        reason,
        value,
    } = outcome;
    match (reason, value) {
        (EditExitReason::Commit, Some(v)) => Some(PEIntent::Edit(EditIntent::Commit {
            row_id: row,
            column_id: col,
            new_value: v,
        })),
        (EditExitReason::Cancel | EditExitReason::ForcedCancel, _) => {
            Some(PEIntent::Edit(EditIntent::Cancel {
                row_id: row,
                column_id: col,
            }))
        }
        (EditExitReason::CommitAndAdvance { direction }, Some(v)) => {
            Some(PEIntent::Edit(EditIntent::CommitAndAdvance {
                row_id: row,
                column_id: col,
                new_value: v,
                direction,
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod text_z_order_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::scene::{RectNode, SceneNode, TextNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::engines::cell_source::{CellSource, PropertyValue};
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::property_editor::{PropertyDataSource, PropertyEditor, PropertyNode};
    use crate::widgets::scroll_area::ScrollArea;

    struct SectionSource;

    impl CellSource for SectionSource {
        fn cell(&self, _row_id: u64, _column_id: ColumnId) -> Option<Cell> {
            None
        }

        fn commit_cell(
            &mut self,
            _row_id: u64,
            _column_id: ColumnId,
            _new_value: PropertyValue,
        ) -> bool {
            false
        }

        fn revision(&self) -> u64 {
            0
        }
    }

    impl PropertyDataSource for SectionSource {
        fn nodes(&self) -> Vec<PropertyNode> {
            vec![PropertyNode {
                node_id: 1,
                label: "Section".to_string(),
                parent_id: None,
                kind: PropertyNodeKind::Section,
            }]
        }
    }

    fn collect<'a>(
        nodes: &'a [SceneNode],
        texts: &mut Vec<&'a TextNode>,
        rects: &mut Vec<&'a RectNode>,
    ) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => texts.push(text),
                SceneNode::Rect(rect) => rects.push(rect),
                SceneNode::RetainedFragment(fragment) => collect(&fragment.children, texts, rects),
                SceneNode::Clip(clip) => collect(&clip.children, texts, rects),
                SceneNode::Layer(layer) => collect(&layer.children, texts, rects),
                SceneNode::Scroll(scroll) => collect(&scroll.children, texts, rects),
                _ => {}
            }
        }
    }

    #[test]
    fn explicit_text_z_property_section_label_is_above_band_fill() {
        let mut editor = PropertyEditor::new();
        editor.add_name_column("Property");
        editor.set_data_source(Rc::new(RefCell::new(SectionSource)));
        let body = editor.create_body_widget();

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(editor), None, 0);
        let scroll_area = tree.add_child(
            root,
            Box::new(ScrollArea::new_unwired().with_scrollbars(false)),
            None,
            0,
        );
        let body = tree.add_child(scroll_area, Box::new(body), None, 0);
        tree.node_mut(scroll_area)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body);

        let mut harness = RetainedSceneTestHarness::new();
        let mut text_engine = FontManagerTextEngine::new();
        let frame = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(360.0, 160.0)),
            &Theme::default(),
            &mut text_engine,
            &IconRegistry::new(&[]),
        );

        let mut texts = Vec::new();
        let mut rects = Vec::new();
        collect(&frame.scene_root, &mut texts, &mut rects);
        let section = texts
            .iter()
            .find(|text| text.text == "Section")
            .expect("property editor body must paint the section label");
        assert_eq!(section.z_index, LAYER_CONTENT);
        assert!(rects.iter().any(|rect| rect.z_index == LAYER_CONTENT - 1));
        assert!(LAYER_CONTENT - 1 < section.z_index);
    }
}
