// =============================================================================
// ListViewFooter: Structural No-Op Widget for Resize Completeness
// =============================================================================
//
// This widget exists for structural completeness. It enables resize arming
// and hover detection from the footer region, ensuring the column resize
// surface is geometrically complete across header, body, and footer.
//
// Default height is 0 (invisible). When height > 0, footer renders separator
// visuals aligned with header and body columns.
//
// ## Invariants
//
// INV-FOOTER-NO-SELECTION: Footer does not own or participate in row selection.
// INV-FOOTER-NO-SCROLL: Footer is fixed, does not scroll.
// INV-FOOTER-NO-RESIZE-LOGIC: Resize logic owned by ListView, not footer.
// INV-FOOTER-READONLY-STATE: Footer reads hovered_separator and
//   active_resize_separator but never mutates shared state.
// INV-FOOTER-STRUCTURAL: Removing footer must not change resize behavior.
//
// =============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::tabular_view::column::ColumnScrollDomain;
use crate::widgets::tabular_view::data_adapter::DataSource;

use super::core::ListViewCore;

/// Separator hit zone half-width (±3px from separator center).
/// Must match ListView::SEPARATOR_HIT_HALF_WIDTH for consistent behavior.
const SEPARATOR_HIT_HALF_WIDTH: f32 = 3.0;

/// Footer widget for ListView.
///
/// Structural completeness widget that participates in separator hit-testing
/// and resize arming. Default height is 0 (invisible, structural no-op).
///
/// ## What Footer Does
///
/// - Paints column separators (when height > 0)
/// - Allows separator hover detection (read-only shared state)
/// - Allows resize arming via event bubbling to ListView
///
/// ## What Footer Does NOT Do
///
/// - Own selection logic (no rows in footer)
/// - Own scroll behavior (footer is fixed)
/// - Own resize logic (ListView handles resize)
/// - Mutate any shared state
pub struct ListViewFooter<S: DataSource> {
    id: WidgetInternalId,
    /// Shared core state with ListView and ListViewBody.
    core: Rc<RefCell<ListViewCore<S>>>,
}

impl<S: DataSource> ListViewFooter<S> {
    /// Create a new footer widget with shared core state.
    pub fn new(core: Rc<RefCell<ListViewCore<S>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            core,
        }
    }

    /// Check if a local X coordinate is within a separator hit zone.
    ///
    /// INV-RESIZE-EXCLUSIVE: Used for yielding to resize arming.
    /// Pinned columns: test in viewport-local space (no scroll adjustment).
    /// Scrollable columns: convert to content-local by adding scroll_x.
    fn is_in_separator_hit_zone(&self, local_x: f32) -> bool {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        // INV-SEPARATOR-PLACEMENT: Only between adjacent columns, not after last.
        let hts = core.cms.hts();
        let Some(layout) = hts.try_column_layout() else {
            return false;
        };
        let all_slots: Vec<_> = layout.header_slots().collect();
        for slot in &all_slots[..all_slots.len().saturating_sub(1)] {
            let sep_x = slot.x + slot.width - 1.0;
            let test_x = if slot.scroll_domain != ColumnScrollDomain::Scrollable {
                local_x // pinned (left or right): viewport-local
            } else {
                local_x + scroll_x // scrollable: viewport → content
            };
            if (test_x - sep_x).abs() <= SEPARATOR_HIT_HALF_WIDTH {
                return true;
            }
        }
        false
    }
}

impl<S: DataSource + 'static> WidgetDowncast for ListViewFooter<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S: DataSource + 'static> Widget for ListViewFooter<S> {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Footer fills available width with height determined by constraints.
        // ListView sets footer height via constraints; default is 0.
        let size = Size::new(constraints.max_width, constraints.max_height);
        LayoutResult::new(size, size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Skip paint if footer has no height (structural no-op).
        if rect.height <= 0.0 {
            return;
        }

        let core = self.core.borrow();

        // Paint footer background.
        cx.scene
            .push_rect(rect, cx.theme.colors.list.header_background, 0, None);

        // INV-PAINT-HSCROLL-1: Read scroll_x for footer alignment with body.
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());

        let sf = cx.scene.scale_factor();
        let base_x = snap_to_physical(rect.x, sf);

        // =====================================================================
        // INV-SEP-Z-PRIORITY: Active > Hovered > Border > Normal separators
        //
        // Paint order for footer:
        // 1. Normal separators (not hovered, not active) - lowest
        // 2. Footer top border - above normal separators
        // 3. Hovered/active separators - highest (above border)
        //
        // INV-FOOTER-READONLY-STATE: Read-only access to shared separator state.
        // INV-RESIZE-VISUAL-CONSISTENCY: Same rendering as header/body.
        // =====================================================================

        // INV-PAINT-HSCROLL-2: Clip footer content to prevent overflow during scroll.
        cx.scene.begin_clip(rect, None);

        let pinned_width = core.cms.hts().column_layout().pinned_width();

        // Pinned background tint: color differentiation from scrollable columns.
        if pinned_width > 0.0 {
            let pinned_bg = RectF::new(rect.x, rect.y, pinned_width, rect.height);
            cx.scene
                .push_rect(pinned_bg, cx.theme.colors.list.pinned_tint, 0, None);
        }

        // INV-SEPARATOR-PLACEMENT: Only between adjacent columns, not after last.
        let sep_slots: Vec<_> = core.cms.hts().column_layout().header_slots().collect();
        let inter_column_slots = &sep_slots[..sep_slots.len().saturating_sub(1)];

        // Boundary separator: last PinnedLeft slot = pinned_left→scrollable transition.
        let boundary_left_column_id = inter_column_slots
            .iter()
            .rev()
            .find(|s| s.scroll_domain == ColumnScrollDomain::PinnedLeft)
            .map(|s| s.column_id);
        // Boundary: last scrollable slot before PinnedRight = scrollable→pinned_right transition.
        let boundary_right_column_id = inter_column_slots
            .iter()
            .find(|s| s.scroll_domain == ColumnScrollDomain::PinnedRight)
            .and_then(|_| {
                inter_column_slots
                    .iter()
                    .rev()
                    .find(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
                    .map(|s| s.column_id)
            });

        // ---- Scrollable separators: clip to area right of pinned boundary ----
        {
            let scrollable_clip_x = base_x + pinned_width;
            let scrollable_clip_w = (rect.x + rect.width - scrollable_clip_x).max(0.0);
            let scrollable_clip =
                RectF::new(scrollable_clip_x, rect.y, scrollable_clip_w, rect.height);
            cx.scene.begin_clip(scrollable_clip, None);

            // Phase 1: Normal scrollable separators (below border).
            for slot in inter_column_slots
                .iter()
                .filter(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
            {
                let is_active = core.active_resize_separator == Some(slot.column_id);
                let is_hovered = core.hovered_separator == Some(slot.column_id);
                if is_active || is_hovered {
                    continue;
                }
                let sep_x = snap_to_physical(base_x + slot.x + slot.width, sf) - 1.0 - scroll_x;
                let sep_rect = RectF::new(sep_x, rect.y, 1.0, rect.height);
                cx.scene
                    .push_rect(sep_rect, cx.theme.colors.borders.default, 1, None);
            }

            cx.scene.end_clip(); // End scrollable clip
        }

        // ---- Pinned separators: paint in pinned area (no scroll_x) ----
        for slot in inter_column_slots
            .iter()
            .filter(|s| s.scroll_domain != ColumnScrollDomain::Scrollable)
        {
            let is_active = core.active_resize_separator == Some(slot.column_id);
            let is_hovered = core.hovered_separator == Some(slot.column_id);
            if is_active || is_hovered {
                continue;
            }
            let sep_x = snap_to_physical(base_x + slot.x + slot.width, sf) - 1.0;
            let is_boundary = boundary_left_column_id == Some(slot.column_id)
                || boundary_right_column_id == Some(slot.column_id);
            let sep_color = if is_boundary {
                cx.theme.colors.borders.strong
            } else {
                cx.theme.colors.borders.default
            };
            let sep_rect = RectF::new(sep_x, rect.y, 1.0, rect.height);
            cx.scene.push_rect(sep_rect, sep_color, 1, None);
        }

        // Phase 2: Footer top border (above normal separators).
        let border_rect = RectF::new(rect.x, rect.y, rect.width, 1.0);
        cx.scene
            .push_rect(border_rect, cx.theme.colors.borders.default, 1, None);

        // Phase 3: Hovered/active separators (above border).
        // ---- Scrollable hovered/active: clip to scrollable area ----
        {
            let scrollable_clip_x = base_x + pinned_width;
            let scrollable_clip_w = (rect.x + rect.width - scrollable_clip_x).max(0.0);
            let scrollable_clip =
                RectF::new(scrollable_clip_x, rect.y, scrollable_clip_w, rect.height);
            cx.scene.begin_clip(scrollable_clip, None);

            for slot in inter_column_slots
                .iter()
                .filter(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
            {
                let is_active = core.active_resize_separator == Some(slot.column_id);
                let is_hovered = core.hovered_separator == Some(slot.column_id);
                if !is_active && !is_hovered {
                    continue;
                }
                let sep_x = snap_to_physical(base_x + slot.x + slot.width, sf) - 1.0 - scroll_x;
                let (sep_width, sep_color) = if is_active {
                    (2.0, cx.theme.colors.borders.focus)
                } else {
                    (2.0, cx.theme.colors.borders.default)
                };
                let sep_rect = RectF::new(sep_x, rect.y, sep_width, rect.height);
                cx.scene.push_rect(sep_rect, sep_color, 2, None);
            }

            cx.scene.end_clip();
        }

        // ---- Pinned hovered/active: paint in pinned area ----
        for slot in inter_column_slots
            .iter()
            .filter(|s| s.scroll_domain != ColumnScrollDomain::Scrollable)
        {
            let is_active = core.active_resize_separator == Some(slot.column_id);
            let is_hovered = core.hovered_separator == Some(slot.column_id);
            if !is_active && !is_hovered {
                continue;
            }
            let sep_x = snap_to_physical(base_x + slot.x + slot.width, sf) - 1.0;
            let (sep_width, sep_color) = if is_active {
                (2.0, cx.theme.colors.borders.focus)
            } else {
                (2.0, cx.theme.colors.borders.default)
            };
            let sep_rect = RectF::new(sep_x, rect.y, sep_width, rect.height);
            cx.scene.push_rect(sep_rect, sep_color, 2, None);
        }

        cx.scene.end_clip();
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // INV-FOOTER-NO-RESIZE-LOGIC: Footer does not own resize.
        // Events bubble to ListView for resize arming.
        //
        // However, footer must check separator hit zone to avoid consuming
        // events that should bubble for resize arming.

        if let Event::Pointer(pe) = event {
            // Only handle Target phase - let events bubble to ListView.
            if phase != EventPhase::Target {
                return false;
            }

            let Some(rect) = cx.widget_global_rect() else {
                return false;
            };

            // INV-RESIZE-EXCLUSIVE: No local interaction while resize is active.
            if self.core.borrow().active_resize_separator.is_some() {
                return false;
            }

            // Check separator hit zone.
            let local_x = pe.global_pos.0 - rect.x;

            if self.is_in_separator_hit_zone(local_x) {
                // In separator zone: return false to bubble to ListView for resize arming.
                return false;
            }

            // Outside separator: no footer-specific behavior.
            // Return false to allow bubbling.
            false
        } else {
            false
        }
    }

    fn hit_testable(&self) -> bool {
        // Footer is hit-testable to receive Target phase events.
        // Events then bubble to ListView for resize handling.
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "ListViewFooter"
    }
}
