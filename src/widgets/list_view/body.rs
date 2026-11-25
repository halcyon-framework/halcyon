// =============================================================================
// ListViewBody: Row content widget for ListView body region
// =============================================================================
//
// This widget paints rows and is the content of ScrollArea.
// It shares state with ListView via Rc<RefCell<ListViewCore>>.
//
// ScrollArea handles:
// - Scroll state and smooth scrolling
// - Scroll event handling
// - Clipping
// - Scrollbar rendering
//
// ListViewBody handles:
// - Row painting at layout positions (NO scroll math - ScrollArea does that)
// - Row hit-testing
// - Cell rendering
// - Focus highlight rendering
// =============================================================================

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::core::composite_text_edit::CompositeEditorId;
use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{LAYER_CONTENT, LAYER_FOCUS};
use crate::core::widget::{
    CompositeChoiceOwnerHandle, EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::{NodeId, WidgetKey};
use crate::engines::cell_mechanics::{
    CompositeChoiceManifest, CompositeChoiceMountFacts, CompositeChoiceOwner, CompositeChoiceRole,
    CompositeChoiceRoleKey, CompositeChoiceSettlement, CompositeChoiceWork,
};
use crate::engines::hierarchical::VisibleRow;
use crate::model::tabular::row_expansion::ExpandedHeightPolicy;
use crate::utils::id::WidgetInternalId;
use crate::widgets::tabular_view::column::{ColumnId, ColumnScrollDomain};
use crate::widgets::tabular_view::data_adapter::{CellValue, DataSource, ValueState};
use crate::widgets::tabular_view::expandable_row_host::ExpandableRowHost;
use crate::widgets::tabular_view::intent_policy::{IntentContext, IntentInput};
use crate::widgets::tabular_view::intents::{SelectionIntent, TabularIntent};
use crate::widgets::tabular_view::row::RowProjection;

use super::core::{ListViewCore, RowId};

/// Separator hit zone half-width (±3px from separator center).
/// Must match ListView::SEPARATOR_HIT_HALF_WIDTH for consistent behavior.
const SEPARATOR_HIT_HALF_WIDTH: f32 = 3.0;
const BODY_ROW_STATE_LAYER: u32 = 2;
const BODY_SEPARATOR_LAYER: u32 = 4;
const BODY_INTERACTION_SEPARATOR_LAYER: u32 = LAYER_FOCUS;
const BODY_EXPANSION_BACKGROUND_LAYER: u32 = LAYER_CONTENT - 1;

/// Body widget that renders rows inside ScrollArea.
///
/// This widget paints at layout positions WITHOUT scroll arithmetic.
/// ScrollArea handles all scroll transforms via its scroll_offset().
pub struct ListViewBody<S: DataSource> {
    id: WidgetInternalId,
    /// Shared core state with ListView.
    core: Rc<RefCell<ListViewCore<S>>>,

    // =========================================================================
    // SPEC-HAL-135: Drag Selection State (mirrors List)
    // Uses RowId (not index) for invariant-safety under reordering.
    //
    // INVARIANTS (P0 hardening):
    //
    // INV-DRAG-RANGE: Drag-range selection ONLY occurs when the policy
    //   interprets the click as a range operation (ExtendToRow).
    //   Plain drag is inert after click selection completes.
    //   Enforced: Move handler checks `drag_active && drag_is_range`.
    //
    // INV-ANCHOR-AUTHORITY: Substrate owns the selection anchor.
    //   - Plain click → action_select_row() → SelectRow sets anchor
    //   - Range click (no drag) → action_extend_to_row() extends FROM substrate anchor
    //   - Range drag → first Move calls action_select_row(drag_anchor) to set anchor,
    //     then action_extend_to_row() extends to current row
    //   - Keyboard extend → uses same substrate anchor via ExtendToRow
    //
    // INV-CAPTURE-DISCIPLINE: Drag logic requires:
    //   - Pointer captured (via cx.capture_pointer on range Down)
    //   - drag_active == true
    //   - drag_is_range == true
    //   On finalize: all 4 drag fields cleared deterministically.
    //
    // =========================================================================
    /// Drag selection active.
    drag_active: bool,
    /// Drag anchor row (row where drag started).
    drag_anchor: Option<RowId>,
    /// Last row during drag (for change detection).
    drag_last: Option<RowId>,
    /// Whether this drag is a range operation (policy returned ExtendToRow).
    drag_is_range: bool,

    /// Currently hovered row (identity). Set on pointer Move, read by paint to draw
    /// the row-hover band. Identity-based (RowId), robust under virtualization —
    /// mirrors `drag_anchor` and row_id-based selection. `None` when the pointer is
    /// not over a row.
    hovered_row: Option<RowId>,

    /// SPEC-HAL-313 §3.4: Manages per-row widget subtrees for expanded rows.
    host: ExpandableRowHost,

    /// Cached visible HTS rows from last layout.
    visible_rows: Vec<VisibleRow>,
    /// Cached ListView row projections from last layout.
    realized_rows: Vec<RowProjection>,
    /// Sorted index of the first visible row.
    first_sorted: usize,
    /// Viewport height from last layout.
    viewport_height: f32,
    /// Cached content height from last layout.
    content_height: f32,
    /// Current Layout's pure requested editor roles; never evidence of readiness.
    choice_role_keys: HashSet<CompositeChoiceRoleKey>,
}

impl<S: DataSource> ListViewBody<S> {
    pub fn new(core: Rc<RefCell<ListViewCore<S>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            core,
            drag_active: false,
            drag_anchor: None,
            drag_last: None,
            drag_is_range: false,
            hovered_row: None,
            host: ExpandableRowHost::new(),
            visible_rows: Vec::new(),
            realized_rows: Vec::new(),
            first_sorted: 0,
            viewport_height: 0.0,
            content_height: 0.0,
            choice_role_keys: HashSet::new(),
        }
    }

    // =========================================================================
    // Hit Testing
    // =========================================================================

    /// Hit-test to find the RowId at a given local Y coordinate.
    /// Returns None if outside content.
    fn hit_test_row(&self, local_y: f32) -> Option<RowId> {
        let core = self.core.borrow();
        // SPEC-PM-017: f32 shim — rung 2 converts ListView hit-testing.
        let content_y = local_y + core.cms.hts().scroll_offset() as f32;
        drop(core);
        self.realized_rows
            .iter()
            .find(|row| content_y >= row.y_offset && content_y < row.y_offset + row.height)
            .map(|row| row.row_id)
    }

    /// Check if a local X coordinate is within a separator hit zone.
    ///
    /// INV-RESIZE-EXCLUSIVE: PointerDown on separator must NOT trigger row interaction.
    /// This check allows row interaction logic to yield to resize arming.
    ///
    /// Domain-aware: widget_global_rect() returns absolute_paint_rect (no scroll baked in),
    /// so local_x = pe.global_pos.0 - rect.x is viewport-local, NOT content-local.
    /// - Pinned separators: viewport-local matches pinned position (no adjustment)
    /// - Scrollable separators: add scroll_x to convert viewport → content
    fn is_in_separator_hit_zone(&self, local_x: f32) -> bool {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        // INV-SEPARATOR-PLACEMENT: Only between adjacent columns, not after last.
        // Guard hoisted (INV-337-A1): try_column_layout borrows from the hts
        // guard; the slots are consumed on the next statement.
        let hts = core.cms.hts();
        let Some(layout) = hts.try_column_layout() else {
            return false;
        };
        let all_slots: Vec<_> = layout.header_slots().collect();
        for slot in &all_slots[..all_slots.len().saturating_sub(1)] {
            let sep_x = slot.x + slot.width - 1.0;
            let test_x = if slot.scroll_domain != ColumnScrollDomain::Scrollable {
                local_x // pinned (left or right): viewport-local matches pinned position
            } else {
                local_x + scroll_x // scrollable: viewport → content
            };
            if (test_x - sep_x).abs() <= SEPARATOR_HIT_HALF_WIDTH {
                return true;
            }
        }
        false
    }

    // =========================================================================
    // Drag State Cleanup
    // =========================================================================

    /// Finalize drag and clean up transient drag state.
    fn finalize_drag(&mut self) {
        self.drag_active = false;
        self.drag_anchor = None;
        self.drag_last = None;
        self.drag_is_range = false;
    }

    /// Check if a viewport-local Y position falls within the expansion
    /// region of an expanded row (below the collapsed strip).
    ///
    /// Callers pass viewport-relative Y (global_y - rect.y).
    /// Row offsets from realized_rows are content-relative.
    /// We add scroll_y to convert viewport → content space.
    fn is_y_in_expansion(&self, viewport_local_y: f32) -> bool {
        let core = self.core.borrow();
        let collapsed_h = core.item_height;
        // SPEC-PM-017: f32 shim — rung 2.
        let scroll_y = core.cms.hts().scroll_offset() as f32;
        let content_y = viewport_local_y + scroll_y;
        drop(core);
        for row in &self.realized_rows {
            if content_y >= row.y_offset && content_y < row.y_offset + row.height {
                // Found the row containing this Y.
                if row.height > collapsed_h {
                    // Row is expanded. Is Y in the expansion part?
                    return content_y >= row.y_offset + collapsed_h;
                }
                return false;
            }
        }
        false
    }

    /// Paint a separator as segments, skipping expansion Y-ranges.
    ///
    /// Instead of one tall rect, paints multiple rects that skip
    /// expansion regions where separators should not be visible.
    fn paint_segmented_separator(
        cx: &mut PaintContext,
        sep_x: f32,
        body_y: f32,
        body_height: f32,
        sep_width: f32,
        color: crate::core::paint::Color,
        z: u32,
        expansion_ranges: &[(f32, f32)],
        cell_edges: &[(u64, ColumnId, RectF, bool, bool)],
    ) {
        if expansion_ranges.is_empty() {
            // No expansions: single rect (fast path).
            cx.paint_cell_grid_separator(
                RectF::new(sep_x, body_y, sep_width, body_height),
                color,
                z,
                cell_edges,
            );
            return;
        }

        // Paint segments between expansion gaps.
        let body_bottom = body_y + body_height;
        let mut seg_top = body_y;

        for &(exp_top_offset, exp_bottom_offset) in expansion_ranges {
            let exp_top = body_y + exp_top_offset;
            let exp_bottom = body_y + exp_bottom_offset;

            if exp_top > seg_top {
                // Paint segment from seg_top to exp_top.
                let h = exp_top - seg_top;
                if h > 0.5 {
                    cx.paint_cell_grid_separator(RectF::new(sep_x, seg_top, sep_width, h), color, z, cell_edges);
                }
            }
            seg_top = exp_bottom;
        }

        // Paint final segment after last expansion.
        if seg_top < body_bottom {
            let h = body_bottom - seg_top;
            if h > 0.5 {
                cx.paint_cell_grid_separator(RectF::new(sep_x, seg_top, sep_width, h), color, z, cell_edges);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_hovered_row(&self) -> Option<RowId> {
        self.hovered_row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_separator_layers_keep_separators_above_row_state() {
        assert!(BODY_ROW_STATE_LAYER < BODY_SEPARATOR_LAYER);
        assert!(BODY_SEPARATOR_LAYER < BODY_INTERACTION_SEPARATOR_LAYER);
        assert_eq!(BODY_SEPARATOR_LAYER, LAYER_FOCUS - 1);
        assert_eq!(BODY_INTERACTION_SEPARATOR_LAYER, LAYER_FOCUS);
    }
}

impl<S: DataSource + 'static> ListViewBody<S> {
    fn layout_content(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        settle: Option<&mut dyn FnMut(Size) -> f32>,
    ) -> LayoutResult {
        let available_width = constraints.max_width;
        let available_height = constraints.max_height;

        {
            let mut core = self.core.borrow_mut();
            core.sync_data_source();
        }

        // SPEC-HAL-313 §3.4: Transfer factory from core to host (once).
        // ListView::set_row_widget_factory stores on core; body takes it here
        // so the host owns it for the rest of the widget's lifetime.
        {
            let mut core = self.core.borrow_mut();
            if let Some(factory) = core.row_widget_factory.take() {
                self.host.set_factory(factory);
            }
        }

        let body_node = cx
            .current_node()
            .expect("SPEC-HAL-313: ListViewBody must have current_node set during layout");
        if let Some(widget_id) = cx.tree.node(body_node).map(|node| node.id) {
            let mut core = self.core.borrow_mut();
            if core.body_widget_id != Some(widget_id) {
                core.body_widget_id = Some(widget_id);
            }
        }
        let expansion_changes = {
            let core = self.core.borrow();
            let hts = core.cms.hts();
            let expansion = hts.expansion();
            self.host.sync(expansion, cx.tree, body_node)
        };

        if !expansion_changes.is_empty() {
            let mut core = self.core.borrow_mut();
            let collapsed_h = core.item_height;
            let height_policy = core.cms.hts().expansion().height_policy();
            let expanded_add = match height_policy {
                ExpandedHeightPolicy::Fixed(h) => h,
                ExpandedHeightPolicy::Cap(cap) => cap,
                ExpandedHeightPolicy::Natural => 200.0,
            };

            for &(row_id, is_mounted) in &expansion_changes {
                let new_height = if is_mounted {
                    collapsed_h + expanded_add
                } else {
                    collapsed_h
                };
                core.set_row_height_at(row_id, new_height);
            }
            drop(core);

            if matches!(height_policy, ExpandedHeightPolicy::Natural) {
                let left_indent = 24.0_f32;
                for &(row_id, is_mounted) in &expansion_changes {
                    if !is_mounted {
                        continue;
                    }
                    if let Some(child_node) = self.host.mounted_node(row_id) {
                        let child_w = (available_width - left_indent).max(0.0);
                        let probe = Constraints::new(child_w, child_w, 0.0, f32::INFINITY);
                        let result = cx.layout_child(child_node, probe);
                        let natural_h = result.height;
                        let real_height = collapsed_h + natural_h;

                        let mut core = self.core.borrow_mut();
                        core.set_row_height_at(row_id, real_height);
                    }
                }
            }
        }

        let content_size = {
            let core = self.core.borrow_mut();
            let sf = core.scale_factor;
            core.cms.hts_mut().set_viewport_height(available_height);
            core.cms.hts_mut().resolve_geometry(available_width, sf);
            Size::new(core.content_width(), core.content_height())
        };
        if let Some(settle) = settle {
            let presented_y = settle(content_size);
            if self.sync_scroll(presented_y) {
                cx.tree.mark_paint_dirty(body_node);
            }
        }

        let (visible_rows, choice_manifest, choice_paint_reservations) = {
            let mut core = self.core.borrow_mut();
            let visible = core.visible_rows_for_viewport(available_height);
            core.realize_visible_cells(&visible);

            let body_slot_ids: Vec<ColumnId> = core
                .cms
                .hts()
                .column_layout()
                .body_slots()
                .map(|slot| slot.column_id)
                .collect();
            let mut editable_cells = HashSet::new();
            for row in &visible {
                let row_id = RowId::from_raw(row.node_id);
                for column_id in &body_slot_ids {
                    let capabilities = core.adapter.capabilities(row_id, *column_id);
                    if capabilities.visible
                        && capabilities.readable
                        && capabilities.editable
                        && capabilities.committable
                    {
                        editable_cells.insert((row.node_id, *column_id));
                    }
                }
            }

            let core_rc = self.core.clone();
            core.cms.reconcile_always_mounted_editors_with(
                cx,
                body_node,
                &visible,
                |node_id, column_id, _, _| editable_cells.contains(&(node_id, column_id)),
                move |node_id, column_id, new_value| {
                    let mut core = core_rc.borrow_mut();
                    let row_id = RowId::from_raw(node_id);
                    if core
                        .adapter
                        .commit_cell(row_id, column_id, CellValue::Bool(new_value))
                    {
                        core.sync_data_source();
                    }
                },
            );
            // Layout describes realization and geometry. Phase 2 qualifies
            // residency and synchronization against current owner facts.
            let row_height = core.cms.hts().row_height();
            let scroll_y = core.cms.hts().scroll_offset() as f32;
            let first_sorted = if row_height > 0.0 {
                (scroll_y / row_height).floor() as usize
            } else {
                0
            };
            let slots: Vec<_> = core
                .cms
                .hts()
                .column_layout()
                .body_slots()
                .map(|slot| (slot.column_id, slot.x, slot.width))
                .collect();
            let mut roles = Vec::new();
            let mut choice_paint_reservations = HashSet::new();
            for (vi, row) in visible.iter().enumerate() {
                for (slot_idx, (column_id, x, width)) in slots.iter().enumerate() {
                    if !core.combo_box_options.contains_key(column_id)
                        || matches!(
                            core.realized_cell_value_state(row.node_id, *column_id),
                            ValueState::Ready(value) if !matches!(value, CellValue::String(_))
                        )
                    {
                        continue;
                    }
                    let key = CompositeChoiceRoleKey {
                        row_id: row.node_id,
                        column_id: *column_id,
                    };
                    // Reserve paint for prospective editors under the existing
                    // policy. Already mounted editors also claim their own cell.
                    if editable_cells.contains(&(row.node_id, *column_id)) {
                        choice_paint_reservations.insert(key);
                    }
                    roles.push(CompositeChoiceRole {
                        key,
                        widget_key: WidgetKey::U64(
                            ((row.node_id & 0x0000_FFFF_FFFF_FFFF) << 16)
                                | ((slot_idx as u64 & 0x3FFF) << 2)
                                | 0b10,
                        ),
                        rect: RectF::new(
                            *x,
                            (first_sorted + vi) as f32 * row_height,
                            *width,
                            row_height,
                        ),
                    });
                }
            }
            core.cms.layout_composite_choices(cx, &roles);
            let manifest = CompositeChoiceManifest {
                provider_revision: core.adapter.order_revision(),
                roles,
            };
            if let Some(dragged_column_id) = core.dragged_column_id {
                core.cms
                    .park_always_mounted_editors_for_column(cx, dragged_column_id);
            }
            (visible, manifest, choice_paint_reservations)
        };

        self.choice_role_keys = choice_paint_reservations;
        cx.publish_composite_choice_manifest(choice_manifest);
        self.visible_rows = visible_rows;
        self.first_sorted = self
            .visible_rows
            .first()
            .map(|row| row.row_index)
            .unwrap_or(0);
        self.viewport_height = available_height;
        self.content_height = content_size.height;
        self.realized_rows = self.core.borrow().project_visible_rows(&self.visible_rows);

        if self.host.mounted_count() > 0 {
            let core = self.core.borrow();
            let collapsed_h = core.item_height;
            let height_policy = core.cms.hts().expansion().height_policy();

            let left_indent = 24.0_f32;
            let mounted_entries: Vec<(RowId, NodeId)> = self.host.mounted_entries().collect();
            let mut height_corrections: Vec<(RowId, f32)> = Vec::new();

            for (row_id, child_node) in mounted_entries {
                if let Some(row_projection) =
                    self.realized_rows.iter().find(|rp| rp.row_id == row_id)
                {
                    let expanded_h = row_projection.height - collapsed_h;
                    if expanded_h <= 0.0 {
                        continue;
                    }

                    let child_w = (available_width - left_indent).max(0.0);

                    // Lay out with unconstrained height to measure natural size.
                    let natural_constraints =
                        Constraints::new(child_w, child_w, 0.0, f32::INFINITY);
                    let result = cx.layout_child(child_node, natural_constraints);
                    let natural_h = result.height;

                    // Determine display height based on policy.
                    let display_h = match height_policy {
                        ExpandedHeightPolicy::Fixed(h) => h,
                        ExpandedHeightPolicy::Cap(cap) => natural_h.min(cap),
                        ExpandedHeightPolicy::Natural => natural_h,
                    };

                    let child_rect = RectF::new(
                        left_indent,
                        row_projection.y_offset + collapsed_h,
                        child_w,
                        display_h,
                    );
                    cx.set_child_rect(child_node, child_rect);

                    // If the row height doesn't match, schedule a correction.
                    let expected_row_h = collapsed_h + display_h;
                    if (row_projection.height - expected_row_h).abs() > 1.0 {
                        height_corrections.push((row_id, expected_row_h));
                    }
                } else {
                    // Non-visible row: minimal layout to satisfy epoch assertion.
                    let zero_constraints = Constraints::new(0.0, 0.0, 0.0, 0.0);
                    cx.layout_child(child_node, zero_constraints);
                    cx.set_child_rect(child_node, RectF::new(0.0, -10000.0, 0.0, 0.0));
                }
            }

            // Apply height corrections outside the core borrow.
            if !height_corrections.is_empty() {
                drop(core);
                let mut core = self.core.borrow_mut();
                for (row_id, new_h) in height_corrections {
                    core.set_row_height_at(row_id, new_h);
                }
            }
        }

        let size = Size::new(available_width, available_height);
        LayoutResult::new(size, content_size)
    }
}

impl<S: DataSource + 'static> WidgetDowncast for ListViewBody<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S: DataSource + 'static> CompositeChoiceOwner for ListViewBody<S> {
    fn mounted_composite_choice_targets(&self) -> Vec<(CompositeChoiceRoleKey, CompositeEditorId)> {
        self.core.borrow().cms.mounted_composite_choice_targets()
    }

    fn collect_composite_choice_work(
        &mut self,
        manifest: Option<CompositeChoiceManifest>,
        mounts: &HashMap<CompositeChoiceRoleKey, CompositeChoiceMountFacts>,
    ) -> Vec<CompositeChoiceWork> {
        if manifest
            .as_ref()
            .is_none_or(|manifest| manifest.roles.is_empty())
            && !self.core.borrow().cms.has_composite_choice_roles()
        {
            return Vec::new();
        }
        let core = self.core.clone();
        let on_commit = Rc::new(move |role: CompositeChoiceRoleKey, value: String| {
            let mut core = core.borrow_mut();
            if core.adapter.commit_cell(
                RowId::from_raw(role.row_id),
                role.column_id,
                CellValue::String(value),
            ) {
                core.sync_data_source();
            }
        });
        self.core
            .borrow_mut()
            .collect_composite_choice_work(manifest, mounts, on_commit)
    }

    fn settle_composite_choice_work(&mut self, settlement: CompositeChoiceSettlement) {
        self.core
            .borrow_mut()
            .cms
            .settle_composite_choice(settlement);
    }
}

impl<S: DataSource + 'static> Widget for ListViewBody<S> {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn composite_choice_owner_mut(&mut self) -> Option<CompositeChoiceOwnerHandle<'_>> {
        Some(CompositeChoiceOwnerHandle::new(self))
    }

    fn needs_viewport_constraint(&self) -> bool {
        true
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.layout_content(cx, constraints, None)
    }

    fn layout_scroll_content(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
        settle: &mut dyn FnMut(Size) -> f32,
    ) -> Option<LayoutResult> {
        Some(self.layout_content(cx, constraints, Some(settle)))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let core = self.core.borrow();

        let list_colors = &cx.theme.colors.list;
        let realized_rows = &self.realized_rows;

        // Whether the ListView has keyboard focus (affects selection appearance).
        let list_focused = core.focused;

        // Read scroll_x for pinned column counteraction and scrollable clip.
        // Body is inside ScrollArea which applies -scroll_x to everything.
        // Pinned columns must counteract with +scroll_x to stay fixed.
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let hts = core.cms.hts();
        let column_layout = hts.try_column_layout();
        let row_bg_width = column_layout
            .as_ref()
            .map_or(rect.width, |layout| layout.total_width().max(rect.width));

        // PIXEL-SNAP: Floor the column base X so geometry-snapped column positions
        // land on pixel boundaries. Prevents sub-pixel jitter during window resize.
        let sf = cx.scene.scale_factor();
        let phys_px = 1.0 / sf;
        let base_x = snap_to_physical(rect.x, sf);

        // SPEC-HAL-313: Collapsed row height for cell painting.
        // Cells always occupy the collapsed strip. Expanded rows have height =
        // collapsed_h + expanded_h, but cell painting must NOT stretch into the
        // expanded region (FORBID-313-SELECTION-OVERLAY-BLEED-1).
        let collapsed_h = core.item_height;
        let font_size = cx.theme.typography.font_size;
        let text_padding = cx.theme.list_geom.padding_x;
        // Body paint is in ScrollArea content space. ScrollArea owns viewport
        // clipping; body-internal column clips must span logical content.
        let body_clip_height = self
            .content_height
            .max(self.viewport_height)
            .max(rect.height);

        let substrate_y = realized_rows
            .first()
            .map_or(rect.y, |row| rect.y + row.y_offset);
        let visible_span_h = match (realized_rows.first(), realized_rows.last()) {
            (Some(first), Some(last)) => last.y_offset + last.height - first.y_offset,
            _ => 0.0,
        };
        let substrate_h = visible_span_h.max(self.viewport_height).max(0.0);
        if substrate_h > 0.0 {
            cx.scene.push_rect(
                RectF::new(rect.x, substrate_y, row_bg_width, substrate_h),
                list_colors.row_background,
                0,
                None,
            );
        }

        // =====================================================================
        // Pass 1: Row backgrounds and focus outlines (full width, no domain clip)
        // SPEC-PM-002: Paint at layout-assigned positions.
        // =====================================================================
        for row_projection in realized_rows {
            let row_y = rect.y + row_projection.y_offset;
            let row_height = row_projection.height;
            // SPEC-HAL-313: Cell strip uses collapsed height; expanded region is
            // painted by the child widget, not by cell painter backgrounds.
            let cell_h = if self.host.is_mounted(row_projection.row_id) {
                collapsed_h
            } else {
                row_height
            };
            let row_rect = RectF::new(rect.x, row_y, row_bg_width, cell_h);

            let is_selected = core.is_selected(row_projection.row_id);
            let is_focused = core.is_row_focused(row_projection.row_id);
            let is_hovered = Some(row_projection.row_id) == self.hovered_row;

            let bg_color = if is_selected {
                if list_focused {
                    list_colors.row_selected
                } else {
                    list_colors.row_selected_inactive
                }
            } else if is_hovered {
                list_colors.row_hover
            } else if row_projection.row_index % 2 == 1 {
                list_colors.row_background_alt
            } else {
                list_colors.row_background
            };
            cx.scene.push_rect(row_rect, bg_color, 0, None);

            if is_focused {
                cx.scene
                    .push_border_rect(row_rect, cx.theme.colors.borders.focus, 1.0, 4, None);
            }
        }

        let Some(column_layout) = column_layout else {
            return;
        };
        let pinned_width = column_layout.pinned_width();
        let scrollable_slots: Vec<_> = column_layout.scrollable_slots().collect();
        let pinned_slots: Vec<_> = column_layout.pinned_slots().collect();
        let sep_slots: Vec<_> = column_layout.header_slots().collect();

        // Build list of expansion Y-ranges for separator segmentation.
        // Separators must NOT paint through expansion regions.
        let expansion_ranges: Vec<(f32, f32)> = realized_rows
            .iter()
            .filter_map(|rp| {
                if self.host.is_mounted(rp.row_id) && rp.height > collapsed_h {
                    let exp_top = rp.y_offset + collapsed_h;
                    let exp_bottom = rp.y_offset + rp.height;
                    Some((exp_top, exp_bottom))
                } else {
                    None
                }
            })
            .collect();

        // =====================================================================
        // Pass 2: Scrollable cells (clipped to area right of pinned boundary)
        //
        // In content space, the pinned boundary maps to viewport pinned_width.
        // Scrollable clip starts at rect.x + pinned_width + scroll_x so that
        // in viewport coordinates (after ScrollArea's -scroll_x) the clip is
        // at rect.x + pinned_width.
        // =====================================================================
        let mut cell_edges = Vec::new();
        {
            let scrollable_clip_x = base_x + pinned_width + scroll_x;
            let scrollable_clip_w = (rect.x + rect.width - base_x - pinned_width).max(0.0);
            let scrollable_clip = RectF::new(
                scrollable_clip_x,
                rect.y,
                scrollable_clip_w,
                body_clip_height,
            );
            cx.scene.begin_clip(scrollable_clip, None);

            for row_projection in realized_rows {
                let row_y = rect.y + row_projection.y_offset;
                let row_height = row_projection.height;
                let cell_h = if self.host.is_mounted(row_projection.row_id) {
                    collapsed_h
                } else {
                    row_height
                };
                let is_selected = core.is_selected(row_projection.row_id);

                for slot in &scrollable_slots {
                    // Skip dragged column — suppressed during drag-reorder.
                    if core.dragged_column_id == Some(slot.column_id) {
                        continue;
                    }
                    let visual_x = core
                        .drag_visual_offsets
                        .as_ref()
                        .and_then(|o| o.get(&slot.column_id).copied())
                        .unwrap_or(slot.x);
                    let cell_x = base_x + visual_x;
                    // Compute clip rect trimmed to exclude ghost overlap area.
                    let text_clip = if let Some((gs, ge)) = core.ghost_occlude_range {
                        let cs = visual_x;
                        let ce = visual_x + slot.width;
                        if cs >= ge || ce <= gs {
                            None
                        } else if cs >= gs && ce <= ge {
                            continue;
                        } else if cs < gs {
                            Some(RectF::new(cell_x, row_y, gs - cs, cell_h))
                        } else {
                            Some(RectF::new(cell_x + (ge - cs), row_y, ce - ge, cell_h))
                        }
                    } else {
                        None
                    };
                    let cell_rect = RectF::new(cell_x, row_y, slot.width, cell_h);
                    if cx.has_cell_editor_edge(row_projection.row_id.raw(), slot.column_id) {
                        cell_edges.push((row_projection.row_id.raw(), slot.column_id, cell_rect, is_selected,
                            false));
                    }
                    cx.scene.begin_clip(text_clip.unwrap_or(cell_rect), None);
                    if let Some(cell) = core
                        .cms
                        .realized_cell_value(row_projection.row_id.raw(), slot.column_id)
                    {
                        if !self.choice_role_keys.contains(&CompositeChoiceRoleKey {
                            row_id: row_projection.row_id.raw(),
                            column_id: slot.column_id,
                        }) && !core
                            .cms
                            .has_always_mounted_editor(row_projection.row_id.raw(), slot.column_id)
                        {
                            if core.custom_cell_painter {
                                let value = core.realized_cell_value_state(
                                    row_projection.row_id.raw(),
                                    slot.column_id,
                                );
                                core.cell_painter.paint_cell(
                                    cx,
                                    cell_rect,
                                    row_projection.row_id,
                                    slot.column_id,
                                    value,
                                    None,
                                    is_selected,
                                    font_size,
                                    text_padding,
                                );
                            } else {
                                core.cms.paint_one_cell(cx, cell, cell_rect, is_selected);
                            }
                        }
                    }
                    cx.scene.end_clip();
                }
            }

            cx.paint_cell_editor_edges(cell_edges.iter().copied());
            cx.scene.end_clip(); // End scrollable clip
        }

        // =====================================================================
        // Pass 3: Pinned cells (counteract ScrollArea's -scroll_x)
        //
        // Painted after scrollable cells so pinned content is on top in the
        // pinned area. Scrollable clip already prevents scrollable content
        // from appearing in the pinned area.
        // =====================================================================
        let pinned_tint = cx.theme.colors.list.pinned_tint;
        let mut pinned_cell_edges = Vec::new();
        for row_projection in realized_rows {
            let row_y = rect.y + row_projection.y_offset;
            let row_height = row_projection.height;
            let cell_h = if self.host.is_mounted(row_projection.row_id) {
                collapsed_h
            } else {
                row_height
            };
            let is_selected = core.is_selected(row_projection.row_id);

            // Pinned background tint per row: after row background, before cell text.
            // Selection/focus overlays from Pass 1 remain dominant underneath.
            if pinned_width > 0.0 {
                let tint_rect = RectF::new(base_x + scroll_x, row_y, pinned_width, cell_h);
                cx.scene.push_rect(tint_rect, pinned_tint, 0, None);
            }

            for slot in &pinned_slots {
                // Skip dragged column — suppressed during drag-reorder.
                if core.dragged_column_id == Some(slot.column_id) {
                    continue;
                }
                let visual_x = core
                    .drag_visual_offsets
                    .as_ref()
                    .and_then(|o| o.get(&slot.column_id).copied())
                    .unwrap_or(slot.x);
                let cell_x = base_x + visual_x + scroll_x;
                // Compute clip rect trimmed to exclude ghost overlap area.
                let text_clip = if let Some((gs, ge)) = core.ghost_occlude_range {
                    let cs = visual_x;
                    let ce = visual_x + slot.width;
                    if cs >= ge || ce <= gs {
                        None
                    } else if cs >= gs && ce <= ge {
                        continue;
                    } else if cs < gs {
                        Some(RectF::new(cell_x, row_y, gs - cs, cell_h))
                    } else {
                        Some(RectF::new(cell_x + (ge - cs), row_y, ce - ge, cell_h))
                    }
                } else {
                    None
                };
                let cell_rect = RectF::new(cell_x, row_y, slot.width, cell_h);
                if cx.has_cell_editor_edge(row_projection.row_id.raw(), slot.column_id) {
                    pinned_cell_edges.push((row_projection.row_id.raw(), slot.column_id, cell_rect, is_selected,
                        false));
                }
                cx.scene.begin_clip(text_clip.unwrap_or(cell_rect), None);
                if let Some(cell) = core
                    .cms
                    .realized_cell_value(row_projection.row_id.raw(), slot.column_id)
                {
                    if !self.choice_role_keys.contains(&CompositeChoiceRoleKey {
                        row_id: row_projection.row_id.raw(),
                        column_id: slot.column_id,
                    }) && !core
                        .cms
                        .has_always_mounted_editor(row_projection.row_id.raw(), slot.column_id)
                    {
                        if core.custom_cell_painter {
                            let value = core.realized_cell_value_state(
                                row_projection.row_id.raw(),
                                slot.column_id,
                            );
                            core.cell_painter.paint_cell(
                                cx,
                                cell_rect,
                                row_projection.row_id,
                                slot.column_id,
                                value,
                                None,
                                is_selected,
                                font_size,
                                text_padding,
                            );
                        } else {
                            core.cms.paint_one_cell(cx, cell, cell_rect, is_selected);
                        }
                    }
                }
                cx.scene.end_clip();
            }
        }

        cx.paint_cell_editor_edges(pinned_cell_edges.iter().copied());

        // =====================================================================
        // Separators: scrollable in clip, pinned outside
        //
        // INV-SEP-Z-PRIORITY: Active > Hovered > Normal
        // INV-RESIZE-SHARED-READONLY: Body observes resize state but never mutates it.
        // INV-RESIZE-VISUAL-CONSISTENCY: Header, body, footer render identically.
        // =====================================================================
        let inter_column_slots = &sep_slots[..sep_slots.len().saturating_sub(1)];

        // Boundary separator: last PinnedLeft slot = pinned→scrollable transition.
        let boundary_left_column_id = inter_column_slots
            .iter()
            .rev()
            .find(|s| s.scroll_domain == ColumnScrollDomain::PinnedLeft)
            .map(|s| s.column_id);
        // Boundary separator: first PinnedRight slot = scrollable→pinned transition.
        let boundary_right_column_id = inter_column_slots
            .iter()
            .find(|s| s.scroll_domain == ColumnScrollDomain::PinnedRight)
            .and_then(|_| {
                // The boundary is the scrollable slot just BEFORE the first PinnedRight
                inter_column_slots
                    .iter()
                    .rev()
                    .find(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
                    .map(|s| s.column_id)
            });

        // ---- Scrollable separators (inside scrollable clip) ----
        {
            let scrollable_clip_x = base_x + pinned_width + scroll_x;
            let scrollable_clip_w = (rect.x + rect.width - base_x - pinned_width).max(0.0);
            let scrollable_clip = RectF::new(
                scrollable_clip_x,
                rect.y,
                scrollable_clip_w,
                body_clip_height,
            );
            cx.scene.begin_clip(scrollable_clip, None);

            for slot in inter_column_slots
                .iter()
                .filter(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
            {
                // Skip dragged column's separator during drag-reorder.
                if core.dragged_column_id == Some(slot.column_id) {
                    continue;
                }
                let is_active = core.active_resize_separator == Some(slot.column_id);
                let is_hovered = core.hovered_separator == Some(slot.column_id);
                if is_active || is_hovered {
                    continue;
                }
                let visual_x = core
                    .drag_visual_offsets
                    .as_ref()
                    .and_then(|o| o.get(&slot.column_id).copied())
                    .unwrap_or(slot.x);
                let sep_x = snap_to_physical(base_x + visual_x + slot.width, sf) - phys_px;
                Self::paint_segmented_separator(
                    cx,
                    sep_x,
                    rect.y,
                    body_clip_height,
                    phys_px,
                    cx.theme.colors.borders.subtle,
                    BODY_SEPARATOR_LAYER,
                    &expansion_ranges,
                    &cell_edges,
                );
            }
            for slot in inter_column_slots
                .iter()
                .filter(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
            {
                if core.dragged_column_id == Some(slot.column_id) {
                    continue;
                }
                let is_active = core.active_resize_separator == Some(slot.column_id);
                let is_hovered = core.hovered_separator == Some(slot.column_id);
                if !is_active && !is_hovered {
                    continue;
                }
                let visual_x = core
                    .drag_visual_offsets
                    .as_ref()
                    .and_then(|o| o.get(&slot.column_id).copied())
                    .unwrap_or(slot.x);
                let sep_x = snap_to_physical(base_x + visual_x + slot.width, sf) - phys_px;
                let (sep_width, sep_color) = if is_active {
                    (2.0 * phys_px, cx.theme.colors.borders.focus)
                } else {
                    (2.0 * phys_px, cx.theme.colors.borders.default)
                };
                Self::paint_segmented_separator(
                    cx,
                    sep_x,
                    rect.y,
                    body_clip_height,
                    sep_width,
                    sep_color,
                    BODY_INTERACTION_SEPARATOR_LAYER,
                    &expansion_ranges,
                    &cell_edges,
                );
            }

            cx.scene.end_clip(); // End scrollable separator clip
        }

        // ---- Pinned separators (outside scrollable clip) ----
        for slot in inter_column_slots
            .iter()
            .filter(|s| s.scroll_domain != ColumnScrollDomain::Scrollable)
        {
            if core.dragged_column_id == Some(slot.column_id) {
                continue;
            }
            let is_active = core.active_resize_separator == Some(slot.column_id);
            let is_hovered = core.hovered_separator == Some(slot.column_id);
            if is_active || is_hovered {
                continue;
            }
            let visual_x = core
                .drag_visual_offsets
                .as_ref()
                .and_then(|o| o.get(&slot.column_id).copied())
                .unwrap_or(slot.x);
            let sep_x = snap_to_physical(base_x + visual_x + slot.width + scroll_x, sf) - phys_px;
            let is_boundary = boundary_left_column_id == Some(slot.column_id)
                || boundary_right_column_id == Some(slot.column_id);
            let sep_color = if is_boundary {
                cx.theme.colors.borders.strong
            } else {
                cx.theme.colors.borders.default
            };
            Self::paint_segmented_separator(
                cx,
                sep_x,
                rect.y,
                body_clip_height,
                phys_px,
                sep_color,
                BODY_SEPARATOR_LAYER,
                &expansion_ranges,
                &pinned_cell_edges,
            );
        }
        for slot in inter_column_slots
            .iter()
            .filter(|s| s.scroll_domain != ColumnScrollDomain::Scrollable)
        {
            if core.dragged_column_id == Some(slot.column_id) {
                continue;
            }
            let is_active = core.active_resize_separator == Some(slot.column_id);
            let is_hovered = core.hovered_separator == Some(slot.column_id);
            if !is_active && !is_hovered {
                continue;
            }
            let visual_x = core
                .drag_visual_offsets
                .as_ref()
                .and_then(|o| o.get(&slot.column_id).copied())
                .unwrap_or(slot.x);
            let sep_x = snap_to_physical(base_x + visual_x + slot.width + scroll_x, sf) - phys_px;
            let (sep_width, sep_color) = if is_active {
                (2.0 * phys_px, cx.theme.colors.borders.focus)
            } else {
                (2.0 * phys_px, cx.theme.colors.borders.default)
            };
            Self::paint_segmented_separator(
                cx,
                sep_x,
                rect.y,
                body_clip_height,
                sep_width,
                sep_color,
                BODY_INTERACTION_SEPARATOR_LAYER,
                &expansion_ranges,
                &pinned_cell_edges,
            );
        }

        // =====================================================================
        // Pass 5a: Selected row overlay
        //
        // Re-paints selected/hovered row backgrounds AFTER the separator
        // section at BODY_ROW_STATE_LAYER so they survive clipped
        // cell/separator compositing. Separators use a strictly higher layer
        // so column boundaries remain visible through row highlights.
        // =====================================================================
        for row_projection in realized_rows {
            let is_selected = core.is_selected(row_projection.row_id);
            let is_hovered = Some(row_projection.row_id) == self.hovered_row;
            // Re-paint selected OR hovered row backgrounds at z=2 so they survive
            // the clip-layer compositing of the cell/separator passes, which lands
            // on top of the z=0 Pass-1 backgrounds regardless of z-index. Without
            // this re-paint the hover band (painted only at z=0 in Pass 1) is buried
            // and only appears intermittently depending on geometry. Selection has
            // visual priority over hover (mirrors Pass 1 precedence).
            let overlay_color = if is_selected {
                if list_focused {
                    list_colors.row_selected
                } else {
                    list_colors.row_selected_inactive
                }
            } else if is_hovered {
                list_colors.row_hover
            } else {
                continue;
            };
            let row_y = rect.y + row_projection.y_offset;
            let cell_h = if self.host.is_mounted(row_projection.row_id) {
                collapsed_h
            } else {
                row_projection.height
            };
            let row_rect = RectF::new(rect.x, row_y, row_bg_width, cell_h);
            cx.scene
                .push_rect(row_rect, overlay_color, BODY_ROW_STATE_LAYER, None);
        }

        // =====================================================================
        // Pass 5: Expansion visual structure
        //
        // Paint after the cell passes, with the background below child content
        // in the final strict-Z render order.
        // =====================================================================
        for row_projection in realized_rows {
            let row_y = rect.y + row_projection.y_offset;
            let row_height = row_projection.height;

            if self.host.is_mounted(row_projection.row_id) && row_height > collapsed_h {
                let expansion_y = row_y + collapsed_h;
                let expansion_h = row_height - collapsed_h;
                let sep_y = snap_to_physical(expansion_y, sf);

                // Background fill.
                let expansion_rect = RectF::new(rect.x, sep_y, row_bg_width, expansion_h);
                cx.scene.push_rect(
                    expansion_rect,
                    cx.theme.colors.surfaces.content,
                    BODY_EXPANSION_BACKGROUND_LAYER,
                    None,
                );

                cx.scene.push_rect(
                    RectF::new(rect.x, sep_y, row_bg_width, phys_px * 2.0),
                    cx.theme.colors.borders.subtle,
                    5,
                    None,
                );

                // Per-column expansion decoration via CellPainter.
                let exp_slots = &sep_slots;
                for slot in exp_slots {
                    let col_x = if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                        base_x + slot.x
                    } else {
                        base_x + slot.x + scroll_x
                    };
                    let col_rect = RectF::new(col_x, sep_y, slot.width, expansion_h);
                    core.cell_painter.paint_expansion(
                        cx,
                        col_rect,
                        row_projection.row_id,
                        slot.column_id,
                    );
                }

                // First column separator through expansion region.
                if let Some(first_slot) = exp_slots.first() {
                    if exp_slots.len() > 1 {
                        let visual_x = core
                            .drag_visual_offsets
                            .as_ref()
                            .and_then(|o| o.get(&first_slot.column_id).copied())
                            .unwrap_or(first_slot.x);
                        let sep_x = if first_slot.scroll_domain == ColumnScrollDomain::Scrollable {
                            snap_to_physical(base_x + visual_x + first_slot.width, sf) - phys_px
                        } else {
                            snap_to_physical(base_x + visual_x + first_slot.width + scroll_x, sf)
                                - phys_px
                        };
                        cx.scene.push_rect(
                            RectF::new(sep_x, sep_y, phys_px, expansion_h),
                            cx.theme.colors.borders.subtle,
                            4,
                            None,
                        );
                    }
                }

                // Bottom border.
                let bottom_sep_y = snap_to_physical(expansion_y + expansion_h - phys_px, sf);
                cx.scene.push_rect(
                    RectF::new(rect.x, bottom_sep_y, row_bg_width, phys_px * 2.0),
                    cx.theme.colors.borders.subtle,
                    5,
                    None,
                );
            }
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Store widget_id in core for ListView to invalidate us when column widths change.
        // Only needs to happen once (check avoids borrow_mut on every event).
        if self.core.borrow().body_widget_id.is_none() {
            self.core.borrow_mut().body_widget_id = Some(cx.widget_id);
        }

        // A descendant popup remains on this logical event path, but its screen
        // coordinates must never be reinterpreted as interaction with covered rows.
        if let Event::Pointer(pe) = event {
            if cx.pointer_route_is_overlay() && phase != EventPhase::Target {
                if matches!(pe.kind, PointerEventKind::Move) {
                    self.core.borrow_mut().pointer_in_expansion_region = false;
                    if self.hovered_row.take().is_some() {
                        cx.invalidate_paint();
                    }
                }
                return false;
            }
        }

        let Some(rect) = cx.widget_global_rect() else {
            return false;
        };

        // Handle pointer events for selection and drag.
        if let Event::Pointer(pe) = event {
            // Compute local coordinates for hit-testing.
            let _local_x = pe.global_pos.0 - rect.x;
            let local_y = pe.global_pos.1 - rect.y;

            // Set expansion region flag for ListView separator suppression.
            {
                let in_expansion = self.is_y_in_expansion(local_y);
                self.core.borrow_mut().pointer_in_expansion_region = in_expansion;
            }

            match pe.kind {
                // =============================================================
                // Move: Drag selection update (Capture + Target phases)
                // INV-DRAG-RANGE: Only process if policy indicated range at drag start.
                // INV-RESIZE-EXCLUSIVE: No row interaction while resize is active.
                // =============================================================
                PointerEventKind::Move => {
                    if phase == EventPhase::Capture || phase == EventPhase::Target {
                        // INV-RESIZE-EXCLUSIVE: No row interaction while resize is active.
                        if self.core.borrow().active_resize_separator.is_some() {
                            return false;
                        }

                        // Hover tracking (mirrors List/TreeView). Identity-based (RowId),
                        // robust under virtualization. Rendering is input-driven, so updating
                        // the field is enough — the input frame repaints and paint reads it.
                        let hovered = self.hit_test_row(local_y);
                        if self.hovered_row != hovered {
                            self.hovered_row = hovered;
                            cx.invalidate_paint();
                        }

                        if self.drag_active && self.drag_is_range {
                            let hit_row = self.hit_test_row(local_y);
                            if let (Some(_anchor), Some(current_row)) = (self.drag_anchor, hit_row)
                            {
                                if self.drag_last != hit_row {
                                    let intents = {
                                        let core = self.core.borrow();
                                        let context = IntentContext {
                                            focused: core.focused,
                                            has_anchor: core.selection.anchor_key.is_some(),
                                            is_editing: false,
                                            focused_row: core.focused_row(),
                                            focused_row_expanded: core
                                                .cms
                                                .hts()
                                                .expansion()
                                                .is_expanded(current_row),
                                        };
                                        core.intent_policy.interpret(
                                            &IntentInput::DragMove {
                                                target: current_row,
                                            },
                                            &context,
                                        )
                                    };
                                    if !intents.is_empty() {
                                        let callbacks =
                                            self.core.borrow_mut().dispatch_intents_and_focus(
                                                &intents,
                                                Some(current_row),
                                            );
                                        if callbacks.presentation_changed() {
                                            cx.invalidate_paint();
                                        }
                                        callbacks.invoke();
                                    }
                                    self.drag_last = hit_row;
                                }
                            }
                        }
                    }
                    false
                }

                // =============================================================
                // Down: Start drag session (Target phase for ListViewBody)
                //
                // INV-RESIZE-EXCLUSIVE: If PointerDown is in separator hit zone,
                //   skip all row interaction. Resize arming takes precedence.
                // =============================================================
                PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                    if phase == EventPhase::Target {
                        // INV-RESIZE-EXCLUSIVE: No row interaction while resize is active.
                        // Defensive check - normally we won't receive Target events during
                        // capture, but this ensures correctness.
                        if self.core.borrow().active_resize_separator.is_some() {
                            return false;
                        }

                        // Check separator hit zone BEFORE row interaction.
                        // INV-RESIZE-EXCLUSIVE: Separator clicks must not trigger row selection.
                        let local_x = pe.global_pos.0 - rect.x;
                        if self.is_in_separator_hit_zone(local_x) {
                            // In separator zone: yield to resize arming (handled by ListView).
                            // Return false to allow event to bubble to ListView.
                            return false;
                        }

                        // Suppress row interaction in expansion content.
                        if self.is_y_in_expansion(local_y) {
                            return false;
                        }

                        let hit_row = self.hit_test_row(local_y);

                        if let Some(row_id) = hit_row {
                            // SPEC-HAL-200: Interpret click through policy (no modifier checks).
                            let (policy, context) = {
                                let core = self.core.borrow();
                                let policy = core.intent_policy;
                                let context = IntentContext {
                                    focused: core.focused,
                                    has_anchor: false,
                                    is_editing: false,
                                    focused_row: core.focused_row(),
                                    focused_row_expanded: core
                                        .cms
                                        .hts()
                                        .expansion()
                                        .is_expanded(row_id),
                                };
                                (policy, context)
                            };
                            let input = IntentInput::Click {
                                target: row_id,
                                modifiers: pe.modifiers,
                                is_subcontrol: false,
                            };
                            let intents = policy.interpret(&input, &context);

                            // Check if policy indicates a range operation (arm drag).
                            let is_range = intents.iter().any(|i| {
                                matches!(
                                    i,
                                    TabularIntent::Selection(SelectionIntent::ExtendToRow { .. })
                                )
                            });

                            if is_range {
                                // Range operation: capture pointer for potential drag.
                                cx.capture_pointer(PointerButton::Primary);
                            }

                            let callbacks = self
                                .core
                                .borrow_mut()
                                .dispatch_intents_and_focus(&intents, Some(row_id));
                            if callbacks.presentation_changed() {
                                cx.invalidate_paint();
                            }
                            callbacks.invoke();

                            if is_range {
                                // Commit the range immediately on press, then arm
                                // the drag so a subsequent move keeps extending
                                // from the same anchor.
                                self.drag_active = true;
                                self.drag_anchor = Some(row_id);
                                self.drag_last = Some(row_id);
                                self.drag_is_range = true;
                            }
                        } else {
                            // Clicked empty space - finalize any drag session.
                            self.finalize_drag();
                        }
                    }
                    false
                }

                // =============================================================
                // Up: End drag session (Capture + Target phases)
                // =============================================================
                PointerEventKind::Up if pe.button == Some(PointerButton::Primary) => {
                    if phase == EventPhase::Capture || phase == EventPhase::Target {
                        if self.drag_active {
                            let was_range = self.drag_is_range;

                            // Range gesture: selection was committed on press and
                            // during any drag-move. Just clear drag state.
                            self.finalize_drag();

                            // Range gestures never activate, return consumed.
                            if was_range {
                                return true;
                            }
                        }
                    }
                    false
                }

                // =============================================================
                // DoubleClick: Expansion toggle (SPEC-HAL-313 §9)
                //
                // IntentPolicy handles the decision: if expansion_enabled,
                // emits ExpansionIntent::Toggle; otherwise falls back to
                // ActivationIntent::Activate (if activation_mode == OnDoubleClick).
                // =============================================================
                PointerEventKind::DoubleClick if pe.button == Some(PointerButton::Primary) => {
                    if phase != EventPhase::Target {
                        return false;
                    }

                    // INV-RESIZE-EXCLUSIVE: No row interaction while resize is active.
                    if self.core.borrow().active_resize_separator.is_some() {
                        return false;
                    }

                    // Separator zone check: yield to ListView's auto-size handler.
                    let local_x = pe.global_pos.0 - rect.x;
                    if self.is_in_separator_hit_zone(local_x) {
                        return false;
                    }

                    let hit_row = self.hit_test_row(local_y);
                    let Some(row_id) = hit_row else {
                        return false;
                    };

                    // Suppress expansion toggle in expansion content region.
                    // Double-clicking the expansion area must NOT collapse the row.
                    if self.is_y_in_expansion(local_y) {
                        return false;
                    }

                    // Interpret through policy.
                    let (policy, context) = {
                        let core = self.core.borrow();
                        let policy = core.intent_policy;
                        let context = IntentContext {
                            focused: core.focused,
                            has_anchor: false,
                            is_editing: false,
                            focused_row: core.focused_row(),
                            focused_row_expanded: core.cms.hts().expansion().is_expanded(row_id),
                        };
                        (policy, context)
                    };
                    let input = IntentInput::DoubleClick { target: row_id };
                    let intents = policy.interpret(&input, &context);

                    if intents.is_empty() {
                        return false;
                    }

                    let callbacks = self
                        .core
                        .borrow_mut()
                        .dispatch_intents_and_focus(&intents, None);
                    if callbacks.presentation_changed() {
                        cx.invalidate_paint();
                    }
                    callbacks.invoke();

                    let mut needs_invalidation = false;
                    if intents
                        .iter()
                        .any(|intent| matches!(intent, TabularIntent::Activation(_)))
                    {
                        needs_invalidation = true;
                    }

                    // Expansion intents require subtree invalidation.
                    for intent in &intents {
                        if matches!(intent, TabularIntent::Expansion(_)) {
                            needs_invalidation = true;
                            break;
                        }
                    }

                    if needs_invalidation {
                        cx.invalidate_subtree();
                    }

                    true
                }

                _ => false,
            }
        } else {
            false
        }
    }

    fn debug_type_name(&self) -> &'static str {
        "ListViewBody"
    }

    /// Sync scroll position from ScrollArea to substrate.
    ///
    /// Uses the non-token scroll setter: updates scroll state without producing
    /// a VisibleRangeToken. Row realization happens in resolve_layout_geometry()
    /// during the layout pass. Returning true causes app.rs to mark layout dirty,
    /// which guarantees resolve_layout_geometry() runs this frame.
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

    /// Take requested scroll Y for focus-driven auto-scroll.
    fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.core.borrow_mut().take_requested_scroll_y()
    }

    fn has_requested_scroll(&self) -> bool {
        self.core.borrow().requested_scroll_y.is_some()
    }

    /// Return separator X positions in content-space coordinates.
    ///
    /// Post-layout sync copies these centerlines into ScrollArea's compatibility cache.
    /// ListView uses its current separator geometry for resize admission.
    ///
    /// Domain-aware: pinned separators are at fixed viewport positions, which in
    /// content-space shift with scroll_x. Scrollable separators are at fixed
    /// content-space positions.
    fn separator_hit_bands(&self) -> Option<Vec<f32>> {
        let core = self.core.borrow();
        // Guard: geometry must be resolved for the current epoch before reads.
        // Post-layout sync may call this before TabularSubstrate::layout() has run.
        let hts = core.cms.hts();
        let layout = hts.try_column_layout()?;
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        // INV-SEPARATOR-PLACEMENT: Only between adjacent columns, not after last.
        let all_slots: Vec<_> = layout.header_slots().collect();
        let bands: Vec<f32> = all_slots[..all_slots.len().saturating_sub(1)]
            .iter()
            .map(|slot| {
                if slot.scroll_domain != ColumnScrollDomain::Scrollable {
                    slot.x + slot.width - 1.0 + scroll_x // pinned viewport → content-space
                } else {
                    slot.x + slot.width - 1.0 // already content-space
                }
            })
            .collect();

        if bands.is_empty() { None } else { Some(bands) }
    }
}
