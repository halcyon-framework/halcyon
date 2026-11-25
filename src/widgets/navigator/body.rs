// SPEC-HAL-310: NavigatorBody — Item painter for the grid.
//
// Phases 1-6: Static grid, selection, keyboard nav, hover, drag-to-reorder,
// multi-item drag.

use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::ordered_selection::{OrderedSelectionAction, reduce_ordered_selection};
use crate::core::paint::{Color, LAYER_CONTENT, RoundedCorners, ShadowMode};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, ScrollSyncInvalidation, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::navigation_host::NavigationIntent;
use crate::utils::id::WidgetInternalId;
use crate::widgets::tabular_view::row::RowId;

use super::data_source::NavigatorDataSource;
use super::navigator::{DragState, GridAlignment, NavigatorEvent, NavigatorState, RowIdOracle};

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// Snapshot of config values needed during paint, extracted from NavigatorState
/// to avoid holding the RefCell borrow during painter callbacks.
struct PaintSnapshot {
    cols: usize,
    item_count: usize,
    col_stride: f32,
    row_stride: f32,
    left_pad: f32,
    item_width: f32,
    item_height: f32,
    row_ids: Vec<RowId>,
    selected_keys: HashSet<RowId>,
    focus_key: Option<RowId>,
    focused: bool,
    hovered_index: Option<usize>,
    viewport_height: f32,
    scroll_y: f32,
    /// Indices of all dragged cards (empty if no drag).
    drag_source_indices: Vec<usize>,
    /// Primary dragged RowId (first in dragged_items).
    drag_primary_row_id: Option<RowId>,
    /// All dragged RowIds.
    drag_items: Vec<RowId>,
    drag_pointer: Option<(f32, f32)>,
    drag_grab_offset: Option<(f32, f32)>,
    shift_offsets: Vec<f32>,
    painter: Rc<dyn super::item_painter::ItemPainter>,
}

pub struct NavigatorBody<S: NavigatorDataSource> {
    id: WidgetInternalId,
    pub(crate) state: Rc<RefCell<NavigatorState>>,
    source: Rc<RefCell<S>>,
}

impl<S: NavigatorDataSource> NavigatorBody<S> {
    pub(crate) fn new(state: Rc<RefCell<NavigatorState>>, source: Rc<RefCell<S>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state,
            source,
        }
    }

    /// Hit test: given a local position, return the item index if hit.
    fn hit_test_item(&self, px: f32, py: f32, state: &NavigatorState) -> Option<usize> {
        let cfg = &state.config;
        let cols = state.columns.max(1);
        let col_stride = cfg.item_width + cfg.gap_x;
        let row_stride = cfg.item_height + cfg.gap_y;

        let total_w = cols as f32 * cfg.item_width + (cols as f32 - 1.0).max(0.0) * cfg.gap_x;
        let left_pad = match cfg.alignment {
            GridAlignment::Center => ((state.available_width - total_w) / 2.0).max(0.0),
            GridAlignment::Left => 0.0,
        };

        let col = ((px - left_pad) / col_stride).floor() as isize;
        let row = (py / row_stride).floor() as isize;

        if col < 0 || col >= cols as isize || row < 0 {
            return None;
        }

        let item_x = left_pad + col as f32 * col_stride;
        let item_y = row as f32 * row_stride;
        if px < item_x || px > item_x + cfg.item_width {
            return None;
        }
        if py < item_y || py > item_y + cfg.item_height {
            return None;
        }

        let idx = row as usize * cols + col as usize;
        if idx < state.row_ids.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Compute item rect for an index (in body-local coordinates).
    /// Single source of horizontal item geometry. `avail` is the width to lay
    /// items into — paint passes the true painted `rect.width`; hit-test and
    /// keyboard nav pass `state.available_width`. Returns (item_width, left_pad).
    /// SPEC-HAL-310 INV-310-13: fluid fill stretches each item uniformly across
    /// `cols`, never below the configured `item_width`. `fill_width == false`
    /// preserves fixed width + GridAlignment. Removes SPEC-PM-009 dual-source
    /// geometry drift between paint and hit-test.
    fn resolve_item_x(state: &NavigatorState, cols: usize, avail: f32) -> (f32, f32) {
        let cfg = &state.config;
        if cfg.item_width <= 0.0 {
            // Full-width modes: single column spans the row.
            return (avail, 0.0);
        }
        if cfg.fill_width {
            let gaps = (cols as f32 - 1.0).max(0.0) * cfg.gap_x;
            (((avail - gaps) / cols as f32).max(cfg.item_width), 0.0)
        } else {
            let total_w = cols as f32 * cfg.item_width + (cols as f32 - 1.0).max(0.0) * cfg.gap_x;
            let pad = match cfg.alignment {
                GridAlignment::Center => ((avail - total_w) / 2.0).max(0.0),
                GridAlignment::Left => 0.0,
            };
            (cfg.item_width, pad)
        }
    }

    fn item_rect_for_index(&self, idx: usize, state: &NavigatorState) -> RectF {
        let cfg = &state.config;
        let cols = state.columns.max(1);
        let row_stride = cfg.item_height + cfg.gap_y;

        // SPEC-HAL-310 INV-310-13: shared geometry source (see resolve_item_x).
        let (item_w, left_pad) = Self::resolve_item_x(state, cols, state.available_width);
        let col_stride = item_w + cfg.gap_x;

        let r = idx / cols;
        let c = idx % cols;
        RectF::new(
            left_pad + c as f32 * col_stride,
            r as f32 * row_stride,
            item_w,
            cfg.item_height,
        )
    }

    /// Compute insertion index from pointer position during drag.
    fn compute_insert_index(&self, px: f32, py: f32, state: &NavigatorState) -> usize {
        let cfg = &state.config;
        let cols = state.columns.max(1);
        let row_stride = cfg.item_height + cfg.gap_y;

        // SPEC-HAL-310 INV-310-13: shared geometry source (see resolve_item_x),
        // so drag-insert mapping matches the painted tiles under fill.
        let (item_w, left_pad) = Self::resolve_item_x(state, cols, state.available_width);
        let col_stride = item_w + cfg.gap_x;

        let col = ((px - left_pad) / col_stride)
            .floor()
            .clamp(0.0, (cols - 1) as f32) as usize;
        let row = (py / row_stride).floor().max(0.0) as usize;
        let idx = row * cols + col;
        idx.min(state.row_ids.len())
    }
}

impl<S: NavigatorDataSource + 'static> WidgetDowncast for NavigatorBody<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S: NavigatorDataSource + 'static> Widget for NavigatorBody<S> {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let state = self.state.borrow();
        let cfg = &state.config;
        let item_count = state.row_ids.len();
        let width = constraints.max_width;
        // SPEC-HAL-086: content is laid out at the FULL viewport width — the
        // vertical scrollbar overlays the right edge, it does NOT reserve width.
        // cols comes from Navigator's full-width computation (== content width);
        // content_h reuses that cols so rows/content_h stay consistent with paint.
        let cols = state.columns.max(1);
        let rows = (item_count + cols - 1) / cols;
        let item_h = if cfg.item_height > 0.0 {
            cfg.item_height
        } else {
            24.0 // Fallback row height for modes not yet wired
        };
        let content_h = if rows == 0 {
            0.0
        } else {
            rows as f32 * (item_h + cfg.gap_y) - cfg.gap_y
        };
        LayoutResult::new(Size::new(width, content_h), Size::new(width, content_h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Snap to physical pixel boundary for crisp borders at any DPR
        let dpr = cx.scene.scale_factor();
        let snap_px = |v: f32| -> f32 { (v * dpr).round() / dpr };

        let snap = {
            let state = self.state.borrow();
            let cfg = &state.config;
            let cols = state.columns.max(1);
            let item_count = state.row_ids.len();
            // SPEC-HAL-310 INV-310-13: shared geometry source (see resolve_item_x).
            // Paint stretches to the true painted width (rect.width).
            let (resolved_item_w, resolved_left_pad) =
                Self::resolve_item_x(&state, cols, rect.width);
            let col_stride = snap_px(resolved_item_w + cfg.gap_x);
            let row_stride = snap_px(cfg.item_height + cfg.gap_y);
            let left_pad = snap_px(resolved_left_pad);

            let (drag_src_indices, drag_primary, drag_items_vec, drag_ptr, drag_grab) =
                match &state.drag {
                    Some(d) => (
                        d.source_indices.clone(),
                        d.dragged_items.first().copied(),
                        d.dragged_items.clone(),
                        Some(d.pointer),
                        Some(d.grab_offset),
                    ),
                    None => (Vec::new(), None, Vec::new(), None, None),
                };

            PaintSnapshot {
                cols,
                item_count,
                col_stride,
                row_stride,
                left_pad,
                item_width: snap_px(resolved_item_w),
                item_height: if cfg.item_height > 0.0 {
                    snap_px(cfg.item_height)
                } else {
                    snap_px(24.0)
                },
                row_ids: state.row_ids.clone(),
                selected_keys: state.selection.selected_keys.clone(),
                focus_key: state.selection.focus_key,
                focused: state.focused,
                hovered_index: state.hovered_index,
                viewport_height: state.viewport_height,
                scroll_y: state.scroll_y,
                drag_source_indices: drag_src_indices,
                drag_primary_row_id: drag_primary,
                drag_items: drag_items_vec,
                drag_pointer: drag_ptr,
                drag_grab_offset: drag_grab,
                shift_offsets: state.shift_offsets.clone(),
                painter: state.painter.clone(),
            }
        };

        let source = self.source.borrow();

        // Fix 7: Virtualization — only paint rows overlapping the viewport.
        // Scroll offset is applied at scene-graph level (ScrollNode), so rect.y
        // does NOT reflect scroll position. Use state.scroll_y instead.
        // During drag, paint all rows (shift offsets may move cards across rows).
        let is_dragging = !snap.drag_source_indices.is_empty();
        let scroll_top = snap.scroll_y.max(0.0);
        let scroll_bottom = scroll_top + snap.viewport_height;
        let first_row = if is_dragging {
            0
        } else {
            (scroll_top / snap.row_stride).floor() as usize
        };
        let last_row = if is_dragging {
            (snap.item_count + snap.cols - 1) / snap.cols.max(1)
        } else {
            ((scroll_bottom / snap.row_stride).ceil() as usize).saturating_add(1)
        };
        let first_item = first_row * snap.cols;
        let last_item = ((last_row) * snap.cols).min(snap.item_count);

        // Snap the body origin to physical pixel boundary. All item positions derived from this.
        let origin_x = snap_px(rect.x);
        let origin_y = snap_px(rect.y);

        // GPU precision fix: at large scroll positions (row 185000 → Y ≈ 24M),
        // f32 can't represent sub-pixel positions accurately. Tessellation of
        // rounded corners produces blocky artifacts. Fix: paint cards at small
        // offsets relative to first_row, and use a ScrollNode to position them.
        let base_y_offset = first_row as f32 * snap.row_stride;
        cx.scene.begin_scroll((0.0, -base_y_offset));

        for item_idx in first_item..last_item {
            let row = item_idx / snap.cols;
            let col = item_idx % snap.cols;

            // Positions relative to first_row — keeps values small for GPU precision
            let base_x = origin_x + snap.left_pad + col as f32 * snap.col_stride;
            let base_y = origin_y + (row - first_row) as f32 * snap.row_stride;

            // If this is any dragged item's original slot, paint placeholder
            if snap.drag_source_indices.contains(&item_idx) {
                let item_rect = RectF::new(base_x, base_y, snap.item_width, snap.item_height);
                cx.scene.push_rect(
                    item_rect,
                    cx.theme.colors.borders.subtle.with_alpha(0.3),
                    LAYER_CONTENT,
                    None,
                );
                continue;
            }

            // Apply shift offset for drag animation
            let shift = snap.shift_offsets.get(item_idx).copied().unwrap_or(0.0);
            let (item_x, item_y) = if shift.abs() > 0.01 {
                let shifted_linear = item_idx as f32 + shift;
                let shifted_col = shifted_linear % snap.cols as f32;
                let shifted_row = shifted_linear / snap.cols as f32;
                (
                    origin_x + snap.left_pad + shifted_col * snap.col_stride,
                    origin_y + (shifted_row.floor() - first_row as f32) * snap.row_stride,
                )
            } else {
                (base_x, base_y)
            };
            let item_rect = RectF::new(item_x, item_y, snap.item_width, snap.item_height);

            let is_hovered = snap.hovered_index == Some(item_idx);
            let row_id = snap.row_ids[item_idx];
            let is_selected = snap.selected_keys.contains(&row_id);
            let is_focused = snap.focused && snap.focus_key == Some(row_id);

            // ItemPainter — full item rect, painter owns all visuals
            snap.painter.paint_item(
                cx,
                item_rect,
                row_id,
                is_selected,
                is_hovered,
                is_focused,
                &*source,
            );
        }

        cx.scene.end_scroll();

        // Paint ghost item(s) during drag (Phase 5/6)
        if let (Some(ptr), Some(grab), Some(primary_rid)) = (
            snap.drag_pointer,
            snap.drag_grab_offset,
            snap.drag_primary_row_id,
        ) {
            let ghost_x = (origin_x + ptr.0 - grab.0).round();
            let ghost_y = (origin_y + ptr.1 - grab.1).round();
            let drag_count = snap.drag_items.len();

            // Multi-item: paint stacked shadow cards behind the main ghost
            if drag_count > 1 {
                let stack_offset = 4.0;
                for i in (1..drag_count.min(3)).rev() {
                    let offset = i as f32 * stack_offset;
                    let stack_rect = RectF::new(
                        ghost_x + offset,
                        ghost_y + offset,
                        snap.item_width,
                        snap.item_height,
                    );
                    cx.scene.push_rect(
                        stack_rect,
                        cx.theme.colors.surfaces.level1.with_alpha(0.6),
                        LAYER_CONTENT + 9,
                        None,
                    );
                }
            }

            let ghost_rect = RectF::new(ghost_x, ghost_y, snap.item_width, snap.item_height);

            // Drop shadow
            cx.scene.push_shadow(
                ghost_rect,
                0.0,
                ShadowMode::Drop,
                (0.0, 4.0),
                0.0,
                12.0,
                Color::from_srgb(0.0, 0.0, 0.0, 0.4),
                LAYER_CONTENT + 10,
                None,
            );

            // Ghost background (elevated)
            cx.scene.push_rect(
                ghost_rect,
                cx.theme.colors.surfaces.level2,
                LAYER_CONTENT + 11,
                None,
            );

            // ItemPainter for ghost — full rect
            snap.painter
                .paint_item(cx, ghost_rect, primary_rid, true, false, false, &*source);

            // Count badge for multi-item drag
            if drag_count > 1 {
                let badge_text = format!("{}", drag_count);
                let (bw, _) = cx.text_engine.measure(&badge_text, 11.0);
                let badge_w = bw + 10.0;
                let badge_h = 18.0;
                let badge_x = ghost_x + snap.item_width - badge_w - 4.0;
                let badge_y = ghost_y - badge_h / 2.0;

                cx.scene.push_rounded_rect(
                    RectF::new(badge_x, badge_y, badge_w, badge_h),
                    cx.theme.colors.accents.primary,
                    RoundedCorners::uniform(badge_h / 2.0),
                    LAYER_CONTENT + 12,
                    None,
                );
                let run = cx.text_engine.shape(&badge_text, 11.0);
                cx.text_engine.draw(
                    &run,
                    badge_x + 5.0,
                    badge_y + 2.0,
                    cx.theme.colors.typography.inverse,
                    LAYER_CONTENT + 13,
                    cx.scene,
                );
            }
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                let state = self.state.borrow();
                if state.policy.selection_mode
                    == crate::core::ordered_selection::SelectionMode::None
                {
                    drop(state);
                    if let PointerEventKind::Move = pe.kind {
                        let state = self.state.borrow();
                        let new_hover = self.hit_test_item(pe.local_pos.0, pe.local_pos.1, &state);
                        drop(state);
                        let mut state = self.state.borrow_mut();
                        state.hovered_index = new_hover;
                    }
                    return false;
                }
                let hit_index = self.hit_test_item(pe.local_pos.0, pe.local_pos.1, &state);
                let is_dragging = state.drag.is_some();
                let is_reorderable = false; // Reorder deferred (SPEC-HAL-310 §14)
                drop(state);

                match pe.kind {
                    PointerEventKind::Move => {
                        // During active drag: update pointer + insertion index
                        if is_dragging {
                            let state_ref = self.state.borrow();
                            let new_insert = self.compute_insert_index(
                                pe.local_pos.0,
                                pe.local_pos.1,
                                &state_ref,
                            );
                            drop(state_ref);
                            let mut state = self.state.borrow_mut();
                            if let Some(ref mut drag) = state.drag {
                                drag.pointer = pe.local_pos;
                                drag.insert_index = new_insert;
                            }
                            return true;
                        }

                        // Check drag arm threshold
                        if is_reorderable {
                            let state_r = self.state.borrow();
                            if let Some((armed_idx, down_pos)) = state_r.drag_arm {
                                let dx = pe.local_pos.0 - down_pos.0;
                                let dy = pe.local_pos.1 - down_pos.1;
                                if dx * dx + dy * dy > 25.0 {
                                    let row_id = state_r.row_ids[armed_idx];
                                    let item_rect = self.item_rect_for_index(armed_idx, &state_r);
                                    let len = state_r.row_ids.len();

                                    let dragged_items: Vec<RowId> =
                                        if state_r.selection.is_selected(row_id) {
                                            state_r
                                                .row_ids
                                                .iter()
                                                .filter(|r| state_r.selection.is_selected(**r))
                                                .copied()
                                                .collect()
                                        } else {
                                            vec![row_id]
                                        };
                                    let source_indices: Vec<usize> = dragged_items
                                        .iter()
                                        .filter_map(|rid| {
                                            state_r.row_ids.iter().position(|r| r == rid)
                                        })
                                        .collect();
                                    drop(state_r);

                                    let mut state = self.state.borrow_mut();
                                    if dragged_items.len() == 1
                                        && !state.selection.is_selected(row_id)
                                    {
                                        let row_ids_snapshot = state.row_ids.clone();
                                        let oracle = RowIdOracle {
                                            row_ids: &row_ids_snapshot,
                                        };
                                        reduce_ordered_selection(
                                            &mut state.selection,
                                            OrderedSelectionAction::Select { key: row_id },
                                            &oracle,
                                        );
                                    }
                                    state.drag = Some(DragState {
                                        dragged_items,
                                        source_indices,
                                        pointer: pe.local_pos,
                                        insert_index: armed_idx,
                                        grab_offset: (
                                            pe.local_pos.0 - item_rect.x,
                                            pe.local_pos.1 - item_rect.y,
                                        ),
                                    });
                                    state.drag_arm = None;
                                    state.shift_offsets = vec![0.0; len];
                                    drop(state);
                                    _cx.capture_pointer(PointerButton::Primary);
                                    return true;
                                }
                            }
                            drop(state_r);
                        }

                        // Normal hover tracking
                        let state = self.state.borrow();
                        let new_hover = self.hit_test_item(pe.local_pos.0, pe.local_pos.1, &state);
                        drop(state);
                        let mut state = self.state.borrow_mut();
                        state.hovered_index = new_hover;
                        false
                    }
                    PointerEventKind::Up => {
                        // Drop during drag
                        if is_dragging {
                            let mut state = self.state.borrow_mut();
                            if let Some(_drag) = state.drag.take() {
                                // Reorder events deferred (SPEC-HAL-310 §14)
                                state.shift_offsets.clear();
                                state.drag_arm = None;
                            }
                            drop(state);
                            _cx.release_pointer();
                            return true;
                        }

                        // Clear drag arm on up
                        let mut state = self.state.borrow_mut();
                        state.drag_arm = None;
                        false
                    }
                    PointerEventKind::Down => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if let Some(idx) = hit_index {
                            let mut state = self.state.borrow_mut();
                            let row_id = state.row_ids[idx];

                            // Arm drag if Reorderable
                            if is_reorderable {
                                state.drag_arm = Some((idx, pe.local_pos));
                            }

                            // Double-click detection
                            let now = web_time::Instant::now();
                            let is_double = state
                                .last_click
                                .map(|(prev_idx, prev_time)| {
                                    prev_idx == idx
                                        && now.duration_since(prev_time).as_millis() < 400
                                })
                                .unwrap_or(false);

                            if is_double {
                                state
                                    .pending_events
                                    .push(NavigatorEvent::ItemDoubleClicked(row_id));

                                // Drill into navigable item on double-click
                                if state.policy.navigation_enabled {
                                    if let Some(ref nav) = state.navigation {
                                        let host_rc = nav.host.clone();
                                        let source = self.source.borrow();
                                        if let Some(node_id) = source.navigation_node(row_id) {
                                            drop(source);
                                            drop(state);
                                            host_rc.borrow_mut().apply(
                                                NavigationIntent::NavigateTo(node_id.into()),
                                            );
                                            let mut state = self.state.borrow_mut();
                                            state
                                                .pending_events
                                                .push(NavigatorEvent::Navigated(node_id));
                                            state.last_click = None;
                                            return true;
                                        }
                                    }
                                }

                                state.last_click = None;
                                return true;
                            }
                            state.last_click = Some((idx, now));

                            // Normal click selection logic
                            let row_ids_snapshot = state.row_ids.clone();
                            let oracle = RowIdOracle {
                                row_ids: &row_ids_snapshot,
                            };

                            let action = if pe.modifiers.ctrl || pe.modifiers.meta {
                                OrderedSelectionAction::Toggle { key: row_id }
                            } else if pe.modifiers.shift {
                                OrderedSelectionAction::ExtendTo { key: row_id }
                            } else {
                                OrderedSelectionAction::Select { key: row_id }
                            };

                            let result =
                                reduce_ordered_selection(&mut state.selection, action, &oracle);

                            if result.selection_changed {
                                state.pending_events.push(NavigatorEvent::SelectionChanged);
                            }
                            state
                                .pending_events
                                .push(NavigatorEvent::ItemClicked(row_id));

                            true
                        } else {
                            // Empty space: clear hover, selection, and focus
                            let mut state = self.state.borrow_mut();
                            state.hovered_index = None;
                            state.selection.selected_keys.clear();
                            state.selection.focus_key = None;
                            state.pending_events.push(NavigatorEvent::SelectionChanged);
                            true // consume — visual state changed
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn on_hover_leave(&mut self, _cx: &mut EventContext) {
        self.state.borrow_mut().hovered_index = None;
    }

    fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.state.borrow_mut().pending_scroll_to.take()
    }

    fn sync_scroll(&mut self, scroll_y: f32) -> bool {
        let mut state = self.state.borrow_mut();
        let old = state.scroll_y;
        // Clear committed target when animation converges
        if let Some(target) = state.scroll_target {
            if (scroll_y - target).abs() < 1.0 {
                state.scroll_target = None;
            }
        }
        state.scroll_y = scroll_y;
        (old - scroll_y).abs() > 0.5
    }

    fn scroll_sync_invalidation(&self) -> ScrollSyncInvalidation {
        ScrollSyncInvalidation::Paint
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "NavigatorBody"
    }
}
