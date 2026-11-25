// SPEC-HAL-223: MenuSurface Overlay Renderer
// Centralized popup rendering and event routing for all menu widgets.
// Owns all geometry computation, hit-testing, and cached layout.
// Delegates interaction (index-level) to MenuSurface model.
//
// INVARIANT: MenuSurfaceOverlay instances are ephemeral.
// Cached geometry must not survive across menu lifetimes.
// No reuse or pooling allowed — each widget embedding creates its own instance.
// Cached values are valid only between paint() and the next frame's event handling.

use std::cell::Cell;

use crate::core::event::{Event, KeyCode, PointerButton, PointerEventKind};
use crate::core::math::RectF;
use crate::core::paint::{LAYER_OVERLAY, TextBackgroundSpan};
use crate::core::text_engine::TextEngine;
use crate::core::theme::Theme;
use crate::core::widget::PaintContext;
use crate::core::widget_tree::OverlayHitRegion;
use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::model::menu_surface::MenuSurface;
use crate::utils::id::TreeWidgetId;

const POPUP_TEXT_LAYER: u32 = LAYER_OVERLAY + 2;

/// Height of a separator item in the menu surface.
const SEPARATOR_HEIGHT: f32 = 7.0;
const MENU_ROW_HEIGHT_MIN: f32 = 24.0;
const MENU_PADDING_X_MIN: f32 = 8.0;

/// Overlay renderer for MenuSurface. Handles popup painting and event-to-model routing.
/// Embedded by PopupMenu, ContextMenu, and DropdownMenu widgets.
///
/// Owns all geometry authority: popup dimensions, hit-testing, Y→index resolution.
/// MenuSurface (model) operates on indices only — this overlay bridges geometry to indices.
pub struct MenuSurfaceOverlay {
    cached_popup_width: Cell<f32>,
    cached_popup_height: Cell<f32>,
    cached_row_height: Cell<f32>,
    cached_padding_x: Cell<f32>,
    cached_popup_offset_x: Cell<f32>,
    cached_popup_offset_y: Cell<f32>,
    /// SPEC-HAL-271: Index currently animating in.
    hover_anim_index: Cell<Option<usize>>,
    active_press_index: Cell<Option<usize>>,
}

impl MenuSurfaceOverlay {
    pub fn new() -> Self {
        Self {
            cached_popup_width: Cell::new(0.0),
            cached_popup_height: Cell::new(0.0),
            cached_row_height: Cell::new(20.0),
            cached_padding_x: Cell::new(6.0),
            cached_popup_offset_x: Cell::new(0.0),
            cached_popup_offset_y: Cell::new(0.0),
            hover_anim_index: Cell::new(None),
            active_press_index: Cell::new(None),
        }
    }

    /// Compute total popup height from items.
    fn compute_popup_height(items: &[MenuItem], row_height: f32) -> f32 {
        items
            .iter()
            .map(|item| match item {
                MenuItem::ActionRow { .. } => row_height,
                MenuItem::Separator => SEPARATOR_HEIGHT,
            })
            .sum()
    }

    /// Hit-test: find item at given y position (popup-local coords).
    fn item_at_y(items: &[MenuItem], y: f32, row_height: f32) -> Option<(usize, MenuItemId)> {
        let mut cursor = 0.0;
        for (i, item) in items.iter().enumerate() {
            let h = match item {
                MenuItem::ActionRow { .. } => row_height,
                MenuItem::Separator => SEPARATOR_HEIGHT,
            };
            if y >= cursor && y < cursor + h {
                return match item {
                    MenuItem::ActionRow { id, .. } => Some((i, *id)),
                    MenuItem::Separator => None,
                };
            }
            cursor += h;
        }
        None
    }

    /// Compute the popup rect using the same geometry authority as paint/hit-test.
    pub fn compute_popup_rect(
        text_engine: &mut dyn TextEngine,
        theme: &Theme,
        surface: &MenuSurface,
        anchor: (f32, f32),
        min_width: f32,
    ) -> RectF {
        let font_size = theme.typography.font_size;
        let row_height = theme.list_geom.row_height_min.max(MENU_ROW_HEIGHT_MIN);
        let padding_x = theme.list_geom.padding_x.max(MENU_PADDING_X_MIN);

        let mut max_label_width: f32 = 0.0;
        for item in surface.items() {
            if let MenuItem::ActionRow {
                label,
                icon,
                shortcut,
                ..
            } = item
            {
                let (w, _) = text_engine.measure(label, font_size);
                let icon_w = if icon.is_some() { font_size + 6.0 } else { 0.0 };
                let shortcut_w = if let Some(s) = shortcut {
                    let (sw, _) = text_engine.measure(s, font_size);
                    sw + 16.0
                } else {
                    0.0
                };
                max_label_width = max_label_width.max(w + icon_w + shortcut_w);
            }
        }

        let popup_width = (max_label_width + padding_x * 2.0).max(min_width);
        let popup_height = Self::compute_popup_height(surface.items(), row_height);
        RectF::new(anchor.0, anchor.1, popup_width, popup_height)
    }

    /// Paint the popup overlay. Called from parent widget's paint_overlay.
    ///
    /// - `anchor`: popup origin in absolute coords
    /// - `min_width`: minimum popup width
    /// - `owner_abs_rect`: if Some, used for OverlayHitRegion owner; if None, uses computed popup_rect
    /// - `widget_rect`: widget's layout rect, for computing event coordinate offset
    pub fn paint(
        &self,
        cx: &mut PaintContext,
        surface: &MenuSurface,
        anchor: (f32, f32),
        min_width: f32,
        tree_widget_id: TreeWidgetId,
        owner_abs_rect: Option<RectF>,
        widget_rect: RectF,
    ) {
        let items = surface.items();
        if items.is_empty() {
            return;
        }
        let highlighted_index = surface.highlighted_index();

        if highlighted_index != self.hover_anim_index.get() {
            self.hover_anim_index.set(highlighted_index);
        }

        let font_size = cx.theme.typography.font_size;
        let list_colors = &cx.theme.colors.list;
        // SPEC-HAL-333 emphasis: popups paint on the elevated popover
        // surface (surfaces.level3 — "Dropdowns, popovers"), not the
        // in-content list row surface. The light-theme level3 value is a
        // SPEC-HAL-332 §9 recalibration item and is not changed here.
        let panel_bg = cx.theme.colors.surfaces.level3;
        let row_height = cx.theme.list_geom.row_height_min.max(MENU_ROW_HEIGHT_MIN);
        let padding_x = cx.theme.list_geom.padding_x.max(MENU_PADDING_X_MIN);

        // Cache geometry for handle_event (no theme access there)
        self.cached_row_height.set(row_height);
        self.cached_padding_x.set(padding_x);
        self.cached_popup_offset_x.set(anchor.0 - widget_rect.x);
        self.cached_popup_offset_y.set(anchor.1 - widget_rect.y);

        let popup_rect =
            Self::compute_popup_rect(cx.text_engine, cx.theme, surface, anchor, min_width);
        let popup_width = popup_rect.width;
        let popup_height = popup_rect.height;

        self.cached_popup_width.set(popup_width);
        self.cached_popup_height.set(popup_height);

        let owner = owner_abs_rect.unwrap_or(popup_rect);

        // Backdrop: full-window overlay so any click outside popup routes here for dismiss.
        // Registered BEFORE popup region — overlay regions checked in reverse order.
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(0.0, 0.0, 100_000.0, 100_000.0),
            owner_abs_rect: owner,
            widget_id: tree_widget_id,
        });

        // Popup region: clicks inside popup route here with correct local coords.
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: popup_rect,
            owner_abs_rect: owner,
            widget_id: tree_widget_id,
        });

        // SPEC-R-013-v2: Drop shadow for popup elevation
        // Use LAYER_OVERLAY so shadow renders in overlay pass, behind popup background
        let s = &cx.theme.shadows;
        cx.scene.push_shadow(
            popup_rect,
            0.0, // sharp corners (menu has no radius)
            crate::core::paint::ShadowMode::Drop,
            (0.0, s.popup_offset_y),
            0.0,
            s.popup_blur,
            crate::core::paint::Color::from_srgb(0.0, 0.0, 0.0, s.popup_alpha),
            LAYER_OVERLAY,
            None,
        );

        // Popup border (push_border_rect, not push_rect — overlay rects at z >= LAYER_OVERLAY
        // are rendered AFTER text in the overlay pass and would occlude popup text).
        cx.scene.push_border_rect(
            popup_rect,
            cx.theme.colors.borders.strong,
            1.0,
            LAYER_OVERLAY,
            None,
        );

        // Full popup background emitted with the text run.
        let bg_run = cx.text_engine.shape(" ", font_size);
        cx.scene.push_text_with_backgrounds(
            &bg_run,
            popup_rect.x,
            popup_rect.y,
            panel_bg,
            vec![TextBackgroundSpan {
                x_offset: 0.0,
                y_offset: 0.0,
                width: popup_rect.width,
                height: popup_height,
                color: panel_bg,
            }],
            POPUP_TEXT_LAYER - 1,
            None,
        );

        // Render items
        let mut cursor_y = popup_rect.y;
        for (i, item) in items.iter().enumerate() {
            match item {
                MenuItem::ActionRow {
                    id: _,
                    label,
                    icon,
                    shortcut,
                    disabled,
                    submenu: _,
                } => {
                    // Don't highlight disabled items
                    let highlighted = highlighted_index == Some(i) && !*disabled;
                    let pressed = self.active_press_index.get() == Some(i) && !*disabled;
                    // SPEC-HAL-333 emphasis: the highlighted menu row is
                    // transient selection, not idle hover — it takes the
                    // sanctioned selection treatment (row_selected +
                    // text_selected + indicator stripe). Resting rows sit
                    // on the panel surface.
                    let text_color = if *disabled {
                        cx.theme.colors.typography.muted
                    } else if highlighted || pressed {
                        list_colors.text_selected
                    } else {
                        list_colors.text
                    };
                    let bg_color = if highlighted || pressed {
                        list_colors.row_selected
                    } else {
                        panel_bg
                    };

                    let mut text_x = popup_rect.x + padding_x;

                    // Leading icon
                    if let Some(icon_id) = icon {
                        let icon_size = font_size;
                        let icon_y = cursor_y + (row_height - icon_size) / 2.0;
                        let icon_color = if *disabled {
                            cx.theme.colors.icons.muted
                        } else {
                            cx.theme.colors.icons.primary
                        };
                        if let Some(def) = cx.icon_registry.get(*icon_id) {
                            cx.scene.push_icon(
                                def,
                                text_x,
                                icon_y,
                                icon_size,
                                icon_color,
                                POPUP_TEXT_LAYER,
                            );
                        }
                        text_x += icon_size + 6.0; // icon + gap
                    }

                    let run = cx.text_engine.shape(label, font_size);
                    let text_y = cursor_y + (row_height - run.height) / 2.0;
                    let mut backgrounds = vec![TextBackgroundSpan {
                        x_offset: -(text_x - popup_rect.x),
                        y_offset: -(text_y - cursor_y),
                        width: popup_rect.width,
                        height: row_height,
                        color: bg_color,
                    }];
                    if highlighted || pressed {
                        backgrounds.push(TextBackgroundSpan {
                            x_offset: -(text_x - popup_rect.x),
                            y_offset: -(text_y - cursor_y),
                            width: 2.0,
                            height: row_height,
                            color: list_colors.selection_indicator,
                        });
                    }

                    cx.scene.push_text_with_backgrounds(
                        &run,
                        text_x,
                        text_y,
                        text_color,
                        backgrounds,
                        POPUP_TEXT_LAYER,
                        None,
                    );

                    // Trailing shortcut hint
                    if let Some(hint) = shortcut {
                        let hint_color = if *disabled {
                            cx.theme.colors.typography.muted
                        } else {
                            cx.theme.colors.typography.primary
                        };
                        let (hw, _) = cx.text_engine.measure(hint, font_size);
                        let hint_x = popup_rect.x + popup_width - padding_x - hw;
                        let hint_run = cx.text_engine.shape(hint, font_size);
                        cx.scene.push_text_with_backgrounds(
                            &hint_run,
                            hint_x,
                            text_y,
                            hint_color,
                            vec![],
                            POPUP_TEXT_LAYER,
                            None,
                        );
                    }

                    cursor_y += row_height;
                }
                MenuItem::Separator => {
                    let line_y = cursor_y + (SEPARATOR_HEIGHT / 2.0).round();
                    cx.scene.push_rect(
                        RectF::new(popup_rect.x + 4.0, line_y, popup_rect.width - 8.0, 1.0),
                        cx.theme.colors.borders.default,
                        LAYER_OVERLAY + 1,
                        None,
                    );
                    cursor_y += SEPARATOR_HEIGHT;
                }
            }
        }
    }

    /// Handle pointer down. Converts widget-local coords to popup-local,
    /// performs hit-testing, and delegates index-level operations to MenuSurface.
    pub fn handle_pointer_down(
        &self,
        surface: &mut MenuSurface,
        local_pos: (f32, f32),
        is_primary: bool,
    ) -> bool {
        let popup_x = local_pos.0 - self.cached_popup_offset_x.get();
        let popup_y = local_pos.1 - self.cached_popup_offset_y.get();
        let popup_width = self.cached_popup_width.get();
        let popup_height = self.cached_popup_height.get();
        let row_height = self.cached_row_height.get();

        let in_popup =
            popup_x >= 0.0 && popup_x < popup_width && popup_y >= 0.0 && popup_y < popup_height;

        if !in_popup {
            self.active_press_index.set(None);
            surface.dismiss();
            return true;
        }

        if !is_primary {
            self.active_press_index.set(None);
            return true;
        }

        if let Some((idx, _id)) = Self::item_at_y(surface.items(), popup_y, row_height) {
            if let Some(MenuItem::ActionRow { disabled: true, .. }) = surface.items().get(idx) {
                self.active_press_index.set(None);
                return true; // disabled — ignore click
            }
            self.active_press_index.set(Some(idx));
            surface.set_highlighted(Some(idx));
        }
        true
    }

    pub fn handle_pointer_up(
        &self,
        surface: &mut MenuSurface,
        local_pos: (f32, f32),
        is_primary: bool,
    ) -> bool {
        let popup_x = local_pos.0 - self.cached_popup_offset_x.get();
        let popup_y = local_pos.1 - self.cached_popup_offset_y.get();
        let popup_width = self.cached_popup_width.get();
        let popup_height = self.cached_popup_height.get();
        let row_height = self.cached_row_height.get();
        let pressed = self.active_press_index.get();

        self.active_press_index.set(None);

        if !is_primary {
            return true;
        }

        let in_popup =
            popup_x >= 0.0 && popup_x < popup_width && popup_y >= 0.0 && popup_y < popup_height;
        if !in_popup {
            return true;
        }

        if let (Some(pressed_idx), Some((release_idx, _))) = (
            pressed,
            Self::item_at_y(surface.items(), popup_y, row_height),
        ) {
            if pressed_idx == release_idx {
                surface.activate_index(release_idx);
            }
        }
        true
    }

    /// Handle pointer move. Converts widget-local coords to popup-local,
    /// performs hit-testing, and sets highlight index on MenuSurface.
    pub fn handle_pointer_move(&self, surface: &mut MenuSurface, local_pos: (f32, f32)) -> bool {
        let popup_x = local_pos.0 - self.cached_popup_offset_x.get();
        let popup_y = local_pos.1 - self.cached_popup_offset_y.get();
        let popup_width = self.cached_popup_width.get();
        let popup_height = self.cached_popup_height.get();
        let row_height = self.cached_row_height.get();

        let in_popup =
            popup_x >= 0.0 && popup_x < popup_width && popup_y >= 0.0 && popup_y < popup_height;

        if in_popup {
            let hit_index =
                Self::item_at_y(surface.items(), popup_y, row_height).map(|(idx, _)| idx);
            surface.set_highlighted(hit_index);
            if self.active_press_index.get().is_some() && self.active_press_index.get() != hit_index
            {
                self.active_press_index.set(None);
            }
        } else {
            surface.set_highlighted(None);
            self.active_press_index.set(None);
        }
        true
    }

    /// Handle keyboard event for popup navigation (Up/Down/Enter/Space/Escape).
    /// Returns Some(true) if consumed, Some(false) if not consumed, None if not a relevant key.
    pub fn handle_key_down(
        &self,
        surface: &mut MenuSurface,
        key_code: KeyCode,
        enable_navigation: bool,
    ) -> Option<bool> {
        match key_code {
            KeyCode::Escape => {
                surface.dismiss();
                Some(true)
            }
            KeyCode::Down if enable_navigation => {
                surface.navigate_highlight(true);
                Some(true)
            }
            KeyCode::Up if enable_navigation => {
                surface.navigate_highlight(false);
                Some(true)
            }
            KeyCode::Enter | KeyCode::Space if enable_navigation => {
                surface.select_highlighted();
                Some(true)
            }
            _ => None,
        }
    }

    /// Full event handler for popup events. Routes pointer and keyboard events to MenuSurface.
    /// `enable_navigation`: if true, handles Up/Down/Enter/Space; if false, only Escape.
    pub fn handle_event(
        &self,
        surface: &mut MenuSurface,
        event: &Event,
        enable_navigation: bool,
    ) -> Option<bool> {
        match event {
            Event::Pointer(pe) if pe.kind == PointerEventKind::Down => {
                let is_primary = pe.button == Some(PointerButton::Primary);
                Some(self.handle_pointer_down(surface, pe.local_pos, is_primary))
            }
            Event::Pointer(pe) if pe.kind == PointerEventKind::Up => {
                let is_primary = pe.button == Some(PointerButton::Primary);
                Some(self.handle_pointer_up(surface, pe.local_pos, is_primary))
            }
            Event::Pointer(pe) if pe.kind == PointerEventKind::Move => {
                Some(self.handle_pointer_move(surface, pe.local_pos))
            }
            Event::Key(ke) if ke.is_down => {
                self.handle_key_down(surface, ke.key_code, enable_navigation)
            }
            _ => None,
        }
    }

    pub fn cached_popup_width(&self) -> f32 {
        self.cached_popup_width.get()
    }

    pub fn cached_popup_height(&self) -> f32 {
        self.cached_popup_height.get()
    }
}
