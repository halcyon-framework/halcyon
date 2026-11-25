// =============================================================================
// Column Configuration Overlay Panel
// =============================================================================
//
// Ephemeral overlay for column visibility, reorder, and pin management.
// Follows MenuSurfaceOverlay pattern: painted in paint_overlay(), uses
// OverlayHitRegion for event routing.
//
// Not a tree widget. Geometry cached in Cell for cross-phase access.
// =============================================================================

use std::cell::Cell;

use crate::core::event::{Event, KeyCode, PointerButton, PointerEventKind};
use crate::core::math::RectF;
use crate::core::paint::{LAYER_OVERLAY, TextBackgroundSpan};
use crate::core::widget::PaintContext;
use crate::core::widget_tree::OverlayHitRegion;
use crate::utils::id::TreeWidgetId;
use crate::widgets::tabular_view::column::{ColumnId, ColumnScrollDomain};
use crate::widgets::tabular_view::column_config::ColumnMetaSnapshot;

const OVERLAY_TEXT_LAYER: u32 = LAYER_OVERLAY + 2;

/// Row height for each column entry in the overlay panel.
const ROW_HEIGHT: f32 = 24.0;

/// Padding inside the panel.
const PANEL_PADDING: f32 = 4.0;

/// Width of the checkbox area.
const CHECKBOX_WIDTH: f32 = 20.0;

/// Width of pin indicator area.
const PIN_WIDTH: f32 = 20.0;

/// Width of arrow buttons area.
const ARROW_WIDTH: f32 = 32.0;

/// Height of the reset/close button row.
const BUTTON_ROW_HEIGHT: f32 = 28.0;

/// Actions that the overlay panel can emit.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConfigAction {
    ToggleVisibility(ColumnId),
    MoveUp(ColumnId),
    MoveDown(ColumnId),
    PinLeft(ColumnId),
    PinRight(ColumnId),
    Unpin(ColumnId),
    ResetAll,
    Close,
}

/// Overlay panel for column configuration.
///
/// Ephemeral overlay — geometry cached in Cells for cross-phase access.
/// Not a tree widget.
pub struct ColumnConfigOverlayPanel {
    cached_panel_rect: Cell<RectF>,
    hovered_index: Cell<Option<usize>>,
    cached_row_height: Cell<f32>,
    cached_offset_x: Cell<f32>,
    cached_offset_y: Cell<f32>,
}

impl ColumnConfigOverlayPanel {
    pub fn new() -> Self {
        Self {
            cached_panel_rect: Cell::new(RectF::new(0.0, 0.0, 0.0, 0.0)),
            hovered_index: Cell::new(None),
            cached_row_height: Cell::new(ROW_HEIGHT),
            cached_offset_x: Cell::new(0.0),
            cached_offset_y: Cell::new(0.0),
        }
    }

    /// Paint the overlay panel.
    ///
    /// `anchor`: absolute (x, y) position where the panel should appear.
    /// `snapshot`: current column metadata for display.
    /// `tree_widget_id`: owning widget for overlay hit regions.
    /// `widget_rect`: owning widget's layout rect (for event coordinate offset).
    pub fn paint(
        &self,
        cx: &mut PaintContext,
        snapshot: &[ColumnMetaSnapshot],
        anchor: (f32, f32),
        tree_widget_id: TreeWidgetId,
        widget_rect: RectF,
    ) {
        if snapshot.is_empty() {
            return;
        }

        let font_size = cx.theme.typography.font_size;
        let list_colors = &cx.theme.colors.list;

        // Compute panel dimensions.
        let mut max_title_width: f32 = 0.0;
        for snap in snapshot {
            let (w, _) = cx.text_engine.measure(&snap.title, font_size);
            max_title_width = max_title_width.max(w);
        }

        let panel_width =
            PANEL_PADDING * 2.0 + CHECKBOX_WIDTH + max_title_width + 8.0 + PIN_WIDTH + ARROW_WIDTH;
        let panel_width = panel_width.max(180.0);
        let panel_height =
            PANEL_PADDING * 2.0 + (snapshot.len() as f32 * ROW_HEIGHT) + BUTTON_ROW_HEIGHT;

        // Cache geometry for handle_event.
        self.cached_row_height.set(ROW_HEIGHT);
        self.cached_offset_x.set(anchor.0 - widget_rect.x);
        self.cached_offset_y.set(anchor.1 - widget_rect.y);

        let panel_rect = RectF::new(anchor.0, anchor.1, panel_width, panel_height);
        self.cached_panel_rect.set(panel_rect);

        let owner = panel_rect;

        // Backdrop: full-window overlay for dismiss on outside click.
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(0.0, 0.0, 100_000.0, 100_000.0),
            owner_abs_rect: owner,
            widget_id: tree_widget_id,
        });

        // Panel region: clicks inside route here.
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: panel_rect,
            owner_abs_rect: owner,
            widget_id: tree_widget_id,
        });

        // Panel background — rendered as TextBackgroundSpan in text pass.
        // The renderer draws: underlay rects → ALL text (sorted by z) → overlay rects.
        // push_rect at LAYER_OVERLAY goes in the overlay pass which renders AFTER text,
        // covering overlay text. TextBackgroundSpan renders in the text pass, before glyphs.
        let bg_run = cx.text_engine.shape(" ", font_size);
        cx.scene.push_text_with_backgrounds(
            &bg_run,
            panel_rect.x,
            panel_rect.y,
            cx.theme.colors.surfaces.level2,
            vec![TextBackgroundSpan {
                x_offset: 0.0,
                y_offset: 0.0,
                width: panel_rect.width,
                height: panel_rect.height,
                color: cx.theme.colors.surfaces.level2,
            }],
            OVERLAY_TEXT_LAYER - 1,
            None,
        );
        // Panel border — thin stroke in overlay pass (doesn't occlude text).
        cx.scene.push_border_rect(
            panel_rect,
            cx.theme.colors.borders.default,
            1.0,
            LAYER_OVERLAY,
            None,
        );

        let hovered = self.hovered_index.get();

        // Paint column rows.
        for (i, snap) in snapshot.iter().enumerate() {
            let row_y = panel_rect.y + PANEL_PADDING + (i as f32 * ROW_HEIGHT);
            let row_rect = RectF::new(
                panel_rect.x + PANEL_PADDING,
                row_y,
                panel_width - PANEL_PADDING * 2.0,
                ROW_HEIGHT,
            );

            // Checkbox — themed colors matching Checkbox widget (overlay rect pipeline).
            let cb_geom = &cx.theme.geometry.checkbox;
            let box_size = cb_geom.size;
            let check_x = row_rect.x + (CHECKBOX_WIDTH - box_size) / 2.0;
            let check_y = row_y + (ROW_HEIGHT - box_size) / 2.0;
            let check_rect = RectF::new(check_x, check_y, box_size, box_size);
            if snap.visible {
                // Checked: accent fill + white centered indicator.
                cx.scene.push_rect(
                    check_rect,
                    cx.theme.colors.accents.primary,
                    OVERLAY_TEXT_LAYER,
                    None,
                );
                let dot_size = box_size * 0.4;
                let dot_x = check_x + (box_size - dot_size) / 2.0;
                let dot_y = check_y + (box_size - dot_size) / 2.0;
                let dot_rect = RectF::new(dot_x, dot_y, dot_size, dot_size);
                cx.scene.push_rect(
                    dot_rect,
                    crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, 1.0),
                    OVERLAY_TEXT_LAYER,
                    None,
                );
            } else {
                // Unchecked: content bg + default border.
                cx.scene.push_rect(
                    check_rect,
                    cx.theme.colors.surfaces.content,
                    OVERLAY_TEXT_LAYER,
                    None,
                );
                cx.scene.push_border_rect(
                    check_rect,
                    cx.theme.colors.borders.default,
                    cb_geom.border_thickness,
                    OVERLAY_TEXT_LAYER,
                    None,
                );
            }

            // Column title with row hover as TextBackgroundSpan (text pass).
            let title_x = row_rect.x + CHECKBOX_WIDTH;
            let text_color = if snap.visible {
                cx.theme.colors.typography.primary
            } else {
                cx.theme.colors.typography.muted
            };
            let run = cx.text_engine.shape(&snap.title, font_size);
            let text_y = row_y + (ROW_HEIGHT - run.height) / 2.0;
            let mut title_backgrounds = Vec::new();
            if hovered == Some(i) {
                title_backgrounds.push(TextBackgroundSpan {
                    x_offset: row_rect.x - title_x,
                    y_offset: row_y - text_y,
                    width: row_rect.width,
                    height: ROW_HEIGHT,
                    color: list_colors.row_hover,
                });
            }
            cx.scene.push_text_with_backgrounds(
                &run,
                title_x,
                text_y,
                text_color,
                title_backgrounds,
                OVERLAY_TEXT_LAYER,
                None,
            );

            // Pin indicator.
            let pin_x = row_rect.x + row_rect.width - PIN_WIDTH - ARROW_WIDTH;
            let pin_label = match snap.domain {
                ColumnScrollDomain::PinnedLeft => "L",
                ColumnScrollDomain::PinnedRight => "R",
                ColumnScrollDomain::Scrollable => "",
            };
            if !pin_label.is_empty() {
                let pin_run = cx.text_engine.shape(pin_label, font_size);
                let pin_y = row_y + (ROW_HEIGHT - pin_run.height) / 2.0;
                cx.scene.push_text_with_backgrounds(
                    &pin_run,
                    pin_x + 4.0,
                    pin_y,
                    cx.theme.colors.typography.secondary,
                    Vec::new(),
                    OVERLAY_TEXT_LAYER,
                    None,
                );
            }

            // Arrow buttons (up/down indicators).
            let arrow_x = row_rect.x + row_rect.width - ARROW_WIDTH;
            if i > 0 {
                let up_run = cx.text_engine.shape("\u{25B2}", font_size * 0.7);
                let up_y = row_y + 2.0;
                cx.scene.push_text_with_backgrounds(
                    &up_run,
                    arrow_x + 4.0,
                    up_y,
                    cx.theme.colors.typography.secondary,
                    Vec::new(),
                    OVERLAY_TEXT_LAYER,
                    None,
                );
            }
            if i < snapshot.len() - 1 {
                let down_run = cx.text_engine.shape("\u{25BC}", font_size * 0.7);
                let down_y = row_y + ROW_HEIGHT - 12.0;
                cx.scene.push_text_with_backgrounds(
                    &down_run,
                    arrow_x + 4.0,
                    down_y,
                    cx.theme.colors.typography.secondary,
                    Vec::new(),
                    OVERLAY_TEXT_LAYER,
                    None,
                );
            }
        }

        // Reset button row.
        let button_y = panel_rect.y + PANEL_PADDING + (snapshot.len() as f32 * ROW_HEIGHT);
        let reset_run = cx.text_engine.shape("Reset", font_size);
        let reset_y = button_y + (BUTTON_ROW_HEIGHT - reset_run.height) / 2.0;
        cx.scene.push_text_with_backgrounds(
            &reset_run,
            panel_rect.x + PANEL_PADDING + 4.0,
            reset_y,
            cx.theme.colors.typography.secondary,
            Vec::new(),
            OVERLAY_TEXT_LAYER,
            None,
        );
    }

    /// Handle an event and return an action if one was triggered.
    ///
    /// `snapshot`: current column metadata (same as passed to paint).
    pub fn handle_event(
        &self,
        event: &Event,
        snapshot: &[ColumnMetaSnapshot],
    ) -> Option<ColumnConfigAction> {
        match event {
            Event::Key(ke) => {
                if ke.key_code == KeyCode::Escape {
                    return Some(ColumnConfigAction::Close);
                }
                None
            }
            Event::Pointer(pe) => {
                let panel = self.cached_panel_rect.get();
                if panel.width == 0.0 || panel.height == 0.0 {
                    return None;
                }

                let local_x = pe.global_pos.0 - panel.x;
                let local_y = pe.global_pos.1 - panel.y;

                // Outside panel = backdrop click → close.
                if local_x < 0.0 || local_y < 0.0 || local_x > panel.width || local_y > panel.height
                {
                    if pe.kind == PointerEventKind::Down
                        && pe.button == Some(PointerButton::Primary)
                    {
                        return Some(ColumnConfigAction::Close);
                    }
                    return None;
                }

                // Inside panel content area.
                let content_y = local_y - PANEL_PADDING;
                let row_area_height = snapshot.len() as f32 * ROW_HEIGHT;

                match pe.kind {
                    PointerEventKind::Move => {
                        // Update hover.
                        if content_y >= 0.0 && content_y < row_area_height {
                            let index = (content_y / ROW_HEIGHT) as usize;
                            if index < snapshot.len() {
                                self.hovered_index.set(Some(index));
                            }
                        } else {
                            self.hovered_index.set(None);
                        }
                        None
                    }
                    PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                        // Check button row (reset).
                        if content_y >= row_area_height
                            && content_y < row_area_height + BUTTON_ROW_HEIGHT
                        {
                            return Some(ColumnConfigAction::ResetAll);
                        }

                        // Check column rows.
                        if content_y >= 0.0 && content_y < row_area_height {
                            let index = (content_y / ROW_HEIGHT) as usize;
                            if index < snapshot.len() {
                                let snap = &snapshot[index];
                                let content_x = local_x - PANEL_PADDING;
                                let row_width = panel.width - PANEL_PADDING * 2.0;

                                // Arrow up region.
                                if content_x > row_width - ARROW_WIDTH
                                    && content_y.rem_euclid(ROW_HEIGHT) < ROW_HEIGHT / 2.0
                                {
                                    if index > 0 {
                                        return Some(ColumnConfigAction::MoveUp(snap.id));
                                    }
                                    return None;
                                }

                                // Arrow down region.
                                if content_x > row_width - ARROW_WIDTH
                                    && content_y.rem_euclid(ROW_HEIGHT) >= ROW_HEIGHT / 2.0
                                {
                                    if index < snapshot.len() - 1 {
                                        return Some(ColumnConfigAction::MoveDown(snap.id));
                                    }
                                    return None;
                                }

                                // Pin region.
                                if content_x > row_width - ARROW_WIDTH - PIN_WIDTH
                                    && content_x <= row_width - ARROW_WIDTH
                                {
                                    return match snap.domain {
                                        ColumnScrollDomain::Scrollable => {
                                            Some(ColumnConfigAction::PinLeft(snap.id))
                                        }
                                        ColumnScrollDomain::PinnedLeft => {
                                            Some(ColumnConfigAction::PinRight(snap.id))
                                        }
                                        ColumnScrollDomain::PinnedRight => {
                                            Some(ColumnConfigAction::Unpin(snap.id))
                                        }
                                    };
                                }

                                // Checkbox/title region → toggle visibility.
                                return Some(ColumnConfigAction::ToggleVisibility(snap.id));
                            }
                        }
                        None
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
