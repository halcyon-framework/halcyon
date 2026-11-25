// Dialog modal screen.
//
// Self-rendering overlay widget: all draw calls emitted at LAYER_OVERLAY+,
// bypassing standard child widget paint. Identical model to menu_surface.rs.
// Opened via `cx.nav_open_modal()`, closed via button press or DismissPolicy.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::app::{Screen, ScreenContext, ScreenFrameContext};
use crate::core::event::{
    Event, EventPhase, FusedInputEvent, KeyCode, PointerButton, PointerEventKind,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::overlay_root::DismissPolicy;
use crate::core::paint::{Color, LAYER_OVERLAY, RoundedCorners, ShadowMode, TextBackgroundSpan};
use crate::core::theme::{ControlChromeCategory, ControlChromeState, ControlValidationState};
use crate::core::widget::{
    EventContext, FocusIndicatorGeometry, LayoutContext, PaintContext, UpdateContext, Widget,
    WidgetDowncast,
};
use crate::core::widget_tree::{NodeId, OverlayHitRegion};
use crate::model::dialog::DialogHandle;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::label::Label;

// ============================================================================
// Config types
// ============================================================================

pub struct DialogConfig {
    pub title: String,
    pub body: String,
    pub buttons: Vec<DialogButton>,
    pub dismiss_on_escape: bool,
    pub dismiss_on_outside_click: bool,
}

pub struct DialogButton {
    pub label: String,
    pub key: String,
    pub is_default: bool,
    pub is_cancel: bool,
}

struct DialogBody {
    text: String,
    update: Option<Box<dyn FnMut(f32) -> Option<String>>>,
}

impl DialogBody {
    fn new(text: String) -> Self {
        Self { text, update: None }
    }
}

// ============================================================================
// Layer + geometry constants
// ============================================================================

const DIALOG_BG_LAYER: u32 = LAYER_OVERLAY + 1;
const DIALOG_TEXT_LAYER: u32 = LAYER_OVERLAY + 2;

const PANEL_WIDTH: f32 = 360.0;
const PANEL_PADDING: f32 = 16.0;
const PANEL_RADIUS: f32 = 2.0;
const TITLE_BAND_HEIGHT: f32 = 34.0;
const BUTTON_SPACING: f32 = 16.0;
const BUTTON_GAP: f32 = 8.0;
const BUTTON_HEIGHT: f32 = 28.0;
const BUTTON_PADDING_X: f32 = 12.0;

// ============================================================================
// DialogWidget: self-rendering overlay widget
// ============================================================================

struct DialogWidget {
    id: WidgetInternalId,
    config: Rc<DialogConfig>,
    body: Rc<RefCell<DialogBody>>,
    pressed_index: Rc<Cell<Option<usize>>>,
    panel_rect_shared: Rc<Cell<RectF>>,
    hover_index: Cell<Option<usize>>,
    active_press_index: Cell<Option<usize>>,
    focused_button_index: Cell<Option<usize>>,
    // Geometry cached in layout, consumed in paint and handle_event
    panel_rect: Cell<RectF>,
    button_rects: RefCell<Vec<RectF>>,
    body_lines: RefCell<Vec<String>>,
}

impl DialogWidget {
    fn new(
        config: Rc<DialogConfig>,
        body: Rc<RefCell<DialogBody>>,
        pressed_index: Rc<Cell<Option<usize>>>,
        panel_rect_shared: Rc<Cell<RectF>>,
    ) -> Self {
        Self {
            id: WidgetInternalId::new(),
            config,
            body,
            pressed_index,
            panel_rect_shared,
            hover_index: Cell::new(None),
            active_press_index: Cell::new(None),
            focused_button_index: Cell::new(None),
            panel_rect: Cell::new(RectF::zero()),
            button_rects: RefCell::new(Vec::new()),
            body_lines: RefCell::new(Vec::new()),
        }
    }

    fn button_at(&self, local_pos: (f32, f32)) -> Option<usize> {
        let (px, py) = local_pos;
        self.button_rects.borrow().iter().position(|rect| {
            px >= rect.x && px < rect.x + rect.width && py >= rect.y && py < rect.y + rect.height
        })
    }

    fn default_focus_index(&self) -> Option<usize> {
        self.config
            .buttons
            .iter()
            .position(|button| button.is_default)
            .or_else(|| {
                self.config
                    .buttons
                    .iter()
                    .position(|button| button.is_cancel)
            })
            .or_else(|| (!self.config.buttons.is_empty()).then_some(0))
    }

    fn move_focus(&self, reverse: bool) {
        let count = self.config.buttons.len();
        if count == 0 {
            self.focused_button_index.set(None);
            return;
        }

        let current = self
            .focused_button_index
            .get()
            .unwrap_or_else(|| self.default_focus_index().unwrap_or(0));
        let next = if reverse {
            if current == 0 { count - 1 } else { current - 1 }
        } else {
            (current + 1) % count
        };
        self.focused_button_index.set(Some(next));
    }

    fn activate_focused_button(&self) -> bool {
        if self.focused_button_index.get().is_none() {
            self.focused_button_index.set(self.default_focus_index());
        }
        if let Some(index) = self.focused_button_index.get() {
            self.pressed_index.set(Some(index));
            return true;
        }
        false
    }
}

impl WidgetDowncast for DialogWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DialogWidget {
    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        let mut body = self.body.borrow_mut();
        let Some(update) = body.update.as_mut() else {
            return;
        };
        let Some(next) = update(dt) else { return };
        if next != body.text {
            body.text = next;
            cx.invalidate_layout();
        }
    }

    fn needs_update(&self) -> bool {
        self.body.borrow().update.is_some()
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let screen_size = Size::new(constraints.max_width, constraints.max_height);
        let font_size = cx.theme.typography.font_size;
        let content_width = PANEL_WIDTH - PANEL_PADDING * 2.0;

        // Word-wrap body text
        let mut body_lines = Vec::new();
        for paragraph in self.body.borrow().text.split('\n') {
            if paragraph.trim().is_empty() {
                body_lines.push(String::new());
                continue;
            }
            let words: Vec<&str> = paragraph.split_whitespace().collect();
            if words.is_empty() {
                body_lines.push(String::new());
                continue;
            }
            let mut current_line = String::new();
            for word in &words {
                if current_line.is_empty() {
                    current_line = word.to_string();
                    continue;
                }
                let test = format!("{} {}", current_line, word);
                let (w, _) = cx.text_engine.measure(&test, font_size);
                if w > content_width {
                    body_lines.push(std::mem::take(&mut current_line));
                    current_line = word.to_string();
                } else {
                    current_line = test;
                }
            }
            if !current_line.is_empty() {
                body_lines.push(current_line);
            }
        }
        if body_lines.is_empty() {
            body_lines.push(String::new());
        }

        // Body height
        let (_, line_height) = cx.text_engine.measure("Ag", font_size);
        let body_height = body_lines.len() as f32 * line_height;

        // Button widths
        let mut button_widths = Vec::new();
        for btn in &self.config.buttons {
            let (w, _) = cx.text_engine.measure(&btn.label, font_size);
            button_widths.push(w + BUTTON_PADDING_X * 2.0);
        }

        // Panel height
        let panel_height = PANEL_PADDING
            + TITLE_BAND_HEIGHT
            + body_height
            + BUTTON_SPACING
            + BUTTON_HEIGHT
            + PANEL_PADDING;

        // Center panel
        let panel_x = (screen_size.width - PANEL_WIDTH) / 2.0;
        let panel_y = (screen_size.height - panel_height) / 2.0;
        let panel_rect = RectF::new(panel_x, panel_y, PANEL_WIDTH, panel_height);
        self.panel_rect.set(panel_rect);
        self.panel_rect_shared.set(panel_rect);

        // Button rects: right-aligned within panel content area
        let button_row_y = panel_y + panel_height - PANEL_PADDING - BUTTON_HEIGHT;
        let mut btn_x = panel_x + PANEL_WIDTH - PANEL_PADDING;
        let mut rects = Vec::new();
        for i in (0..button_widths.len()).rev() {
            btn_x -= button_widths[i];
            rects.push(RectF::new(
                btn_x,
                button_row_y,
                button_widths[i],
                BUTTON_HEIGHT,
            ));
            btn_x -= BUTTON_GAP;
        }
        rects.reverse();
        *self.button_rects.borrow_mut() = rects;
        *self.body_lines.borrow_mut() = body_lines;

        LayoutResult::fit(screen_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let panel_rect = self.panel_rect.get();
        if panel_rect.width <= 0.0 {
            return;
        }
        let panel_abs_rect = panel_rect.translate(rect.x, rect.y);
        self.panel_rect_shared.set(panel_abs_rect);

        let font_size = cx.theme.typography.font_size;
        let title_font_size = font_size + 2.0;

        // 1. Scrim — full screen via TextBackgroundSpan (text pipeline, correct z-ordering).
        // push_rect at LAYER_OVERLAY renders in the overlay pass AFTER all text, occluding
        // dialog text. TextBackgroundSpan renders within the text pipeline where z-ordering
        // correctly layers scrim between base content (z<1000) and dialog content (z>1000).
        let scrim_run = cx.text_engine.shape(" ", font_size);
        cx.scene.push_text_with_backgrounds(
            &scrim_run,
            rect.x,
            rect.y,
            Color::from_srgb(0.0, 0.0, 0.0, 0.0),
            vec![TextBackgroundSpan {
                x_offset: 0.0,
                y_offset: 0.0,
                width: rect.width,
                height: rect.height,
                color: Color::from_srgb(0.0, 0.0, 0.0, 0.65),
            }],
            LAYER_OVERLAY,
            None,
        );

        // 2. Elevation shadow at LAYER_OVERLAY (shadow pipeline renders in overlay pass)
        let elev = cx.theme.shadows.elevation[1];
        cx.scene.push_shadow(
            panel_abs_rect,
            PANEL_RADIUS,
            ShadowMode::Drop,
            (0.0, elev.near_offset_y),
            0.0,
            elev.near_blur,
            Color::from_srgb(0.0, 0.0, 0.0, elev.near_alpha),
            LAYER_OVERLAY,
            None,
        );
        cx.scene.push_shadow(
            panel_abs_rect,
            PANEL_RADIUS,
            ShadowMode::Drop,
            (0.0, elev.far_offset_y),
            0.0,
            elev.far_blur,
            Color::from_srgb(0.0, 0.0, 0.0, elev.far_alpha),
            LAYER_OVERLAY,
            None,
        );

        // 3. Panel background — TextBackgroundSpan (text pipeline, correct z-ordering).
        // push_rounded_rect routes through the path pipeline which renders before text
        // in the main pass, causing base screen text to paint over the panel background.
        // TextBackgroundSpan renders within the text pipeline where z-ordering correctly
        // occludes base content. Sharp corners match menu_surface/tooltip_surface pattern.
        let panel_bg = cx.theme.colors.surfaces.content;
        let bg_run = cx.text_engine.shape(" ", font_size);
        cx.scene.push_text_with_backgrounds(
            &bg_run,
            panel_abs_rect.x,
            panel_abs_rect.y,
            panel_bg,
            vec![TextBackgroundSpan {
                x_offset: 0.0,
                y_offset: 0.0,
                width: panel_abs_rect.width,
                height: panel_abs_rect.height,
                color: panel_bg,
            }],
            DIALOG_BG_LAYER,
            None,
        );

        let title_bg = cx.theme.colors.surfaces.chrome;
        let title_bg_run = cx.text_engine.shape(" ", font_size);
        cx.scene.push_text_with_backgrounds(
            &title_bg_run,
            panel_abs_rect.x,
            panel_abs_rect.y,
            title_bg,
            vec![TextBackgroundSpan {
                x_offset: 0.0,
                y_offset: 0.0,
                width: panel_abs_rect.width,
                height: TITLE_BAND_HEIGHT,
                color: title_bg,
            }],
            DIALOG_BG_LAYER,
            None,
        );

        // Panel border (push_border_rect is safe at overlay z — outline only, doesn't fill)
        cx.scene.push_border_rect(
            panel_abs_rect,
            cx.theme.colors.borders.strong,
            1.0,
            LAYER_OVERLAY,
            None,
        );
        cx.scene.push_rect(
            RectF::new(
                panel_abs_rect.x,
                panel_abs_rect.y + TITLE_BAND_HEIGHT,
                panel_abs_rect.width,
                1.0,
            ),
            cx.theme.colors.borders.strong,
            LAYER_OVERLAY,
            None,
        );

        // 4. Title text
        let title_run = cx.text_engine.shape(&self.config.title, title_font_size);
        let title_y = panel_abs_rect.y + (TITLE_BAND_HEIGHT - title_run.height) * 0.5;
        cx.scene.push_text_with_backgrounds(
            &title_run,
            panel_abs_rect.x + PANEL_PADDING,
            title_y,
            cx.theme.colors.typography.primary,
            vec![],
            DIALOG_TEXT_LAYER,
            None,
        );

        // 5. Body text (word-wrapped lines)
        let body_lines = self.body_lines.borrow();
        let (_, line_height) = cx.text_engine.measure("Ag", font_size);
        let body_y_start = panel_abs_rect.y + TITLE_BAND_HEIGHT + PANEL_PADDING;
        for (i, line) in body_lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let run = cx.text_engine.shape(line, font_size);
            cx.scene.push_text_with_backgrounds(
                &run,
                panel_abs_rect.x + PANEL_PADDING,
                body_y_start + i as f32 * line_height,
                cx.theme.colors.typography.primary,
                vec![],
                DIALOG_TEXT_LAYER,
                None,
            );
        }

        // 6. Button backgrounds and labels
        // TextBackgroundSpan for button fill, emitted before glyphs at same z.
        let button_rects = self.button_rects.borrow();
        for (i, btn) in self.config.buttons.iter().enumerate() {
            let btn_rect = button_rects[i].translate(rect.x, rect.y);

            let is_hovered = self.hover_index.get() == Some(i);
            let is_focused = self.focused_button_index.get() == Some(i);
            let is_pressed = self.active_press_index.get() == Some(i) && is_hovered;
            let chrome_state = if is_pressed {
                ControlChromeState::Pressed
            } else if is_hovered || is_focused {
                ControlChromeState::Hovered
            } else {
                ControlChromeState::Idle
            };
            let chrome = cx.theme.control_chrome(
                ControlChromeCategory::RaisedClickable,
                chrome_state,
                ControlValidationState::Neutral,
            );
            let radius = cx.theme.chrome_radius(
                ControlChromeCategory::RaisedClickable,
                cx.theme.geometry.button.radius,
                false,
            );

            let (bg_color, border_color, text_color) = if btn.is_default {
                let fill = if is_pressed {
                    cx.theme.colors.accents.hover
                } else if is_hovered {
                    cx.theme
                        .colors
                        .accents
                        .primary
                        .lerp(cx.theme.colors.accents.hover, 0.5)
                } else {
                    cx.theme.colors.accents.primary
                };
                (
                    fill,
                    chrome.border_color,
                    cx.theme.colors.typography.inverse,
                )
            } else {
                (
                    chrome.fill.unwrap_or(cx.theme.colors.controls.background),
                    chrome.border_color,
                    cx.theme.colors.controls.text,
                )
            };

            let run = cx.text_engine.shape(&btn.label, font_size);
            let tx = btn_rect.x + (btn_rect.width - run.width) / 2.0;
            let ty = btn_rect.y + (btn_rect.height - run.height) / 2.0;
            cx.scene.push_text_with_backgrounds(
                &run,
                tx,
                ty,
                text_color,
                vec![TextBackgroundSpan {
                    x_offset: -(tx - btn_rect.x),
                    y_offset: -(ty - btn_rect.y),
                    width: btn_rect.width,
                    height: btn_rect.height,
                    color: bg_color,
                }],
                DIALOG_TEXT_LAYER,
                None,
            );

            cx.scene.push_border_rounded_rect(
                btn_rect,
                border_color,
                chrome.border_width,
                RoundedCorners::uniform(radius),
                DIALOG_TEXT_LAYER + 1,
                None,
            );
        }
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        let panel_rect = self.panel_rect.get();
        let panel_abs_rect = panel_rect.translate(rect.x, rect.y);
        self.panel_rect_shared.set(panel_abs_rect);

        // Full-screen overlay hit region: captures all pointer events for the dialog
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(0.0, 0.0, 100_000.0, 100_000.0),
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, _phase: EventPhase) -> bool {
        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Move => {
                    let next_hover = self.button_at(pe.local_pos);
                    if self.hover_index.get() != next_hover {
                        self.hover_index.set(next_hover);
                        cx.invalidate_layout();
                    }
                    true
                }
                PointerEventKind::Down
                | PointerEventKind::DoubleClick
                | PointerEventKind::TripleClick => {
                    if pe.button == Some(PointerButton::Primary) {
                        let next_press = self.button_at(pe.local_pos);
                        self.hover_index.set(next_press);
                        self.active_press_index.set(next_press);
                        if next_press.is_some() {
                            self.focused_button_index.set(next_press);
                        }
                        if next_press.is_some() {
                            cx.capture_pointer(PointerButton::Primary);
                        }
                        cx.invalidate_layout();
                    }
                    true
                }
                PointerEventKind::Up => {
                    if pe.button == Some(PointerButton::Primary) {
                        let release_index = self.button_at(pe.local_pos);
                        let armed_index = self.active_press_index.take();
                        self.hover_index.set(release_index);
                        if let (Some(armed), Some(released)) = (armed_index, release_index) {
                            if armed == released {
                                self.pressed_index.set(Some(armed));
                            }
                        }
                        cx.invalidate_layout();
                    }
                    true
                }
                _ => true,
            },
            Event::Key(key) if key.is_down => match key.key_code {
                KeyCode::Tab => {
                    self.move_focus(key.modifiers.shift);
                    cx.invalidate_layout();
                    true
                }
                KeyCode::Left | KeyCode::Up => {
                    self.move_focus(true);
                    cx.invalidate_layout();
                    true
                }
                KeyCode::Right | KeyCode::Down => {
                    self.move_focus(false);
                    cx.invalidate_layout();
                    true
                }
                KeyCode::Enter | KeyCode::Space => {
                    let handled = self.activate_focused_button();
                    if handled {
                        cx.invalidate_layout();
                    }
                    handled
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        !self.config.buttons.is_empty()
    }

    fn is_focus_scope(&self) -> bool {
        true
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        if self.focused_button_index.get().is_none() {
            self.focused_button_index.set(self.default_focus_index());
        }
        cx.invalidate_layout();
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.focused_button_index.set(None);
        cx.invalidate_layout();
    }

    fn accepts_keyboard_activation(&self) -> bool {
        true
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        if self.activate_focused_button() {
            cx.invalidate_layout();
        }
    }

    fn focus_indicator_geometry(&self, own_rect: RectF) -> FocusIndicatorGeometry {
        if let Some(index) = self.focused_button_index.get() {
            if let Some(button_rect) = self.button_rects.borrow().get(index).copied() {
                return FocusIndicatorGeometry::for_rect(RectF::new(
                    own_rect.x + button_rect.x,
                    own_rect.y + button_rect.y,
                    button_rect.width,
                    button_rect.height,
                ));
            }
        }
        FocusIndicatorGeometry::for_rect(own_rect)
    }
}

// ============================================================================
// Dialog Screen
// ============================================================================

pub struct Dialog {
    config: Rc<DialogConfig>,
    body: Rc<RefCell<DialogBody>>,
    handle: DialogHandle,
    needs_rebuild: bool,
    placeholder: Label,
    pressed_index: Rc<Cell<Option<usize>>>,
    panel_rect: Rc<Cell<RectF>>,
}

impl Dialog {
    pub fn new(config: DialogConfig, handle: DialogHandle) -> Self {
        let body = Rc::new(RefCell::new(DialogBody::new(config.body.clone())));
        Self {
            config: Rc::new(config),
            body,
            handle,
            needs_rebuild: true,
            placeholder: Label::new(""),
            pressed_index: Rc::new(Cell::new(None)),
            panel_rect: Rc::new(Cell::new(RectF::zero())),
        }
    }

    /// Attach a live body producer for this dialog's mounted lifetime.
    ///
    /// Runs only in the widget update phase with the engine's frame delta in
    /// seconds. `None` retains the current body; identical text does no work.
    /// Changed text invalidates wrapping and panel/button geometry through the
    /// normal layout phase without rebuilding the modal or changing its buttons,
    /// dismissal policy, focus, Capture or pending button activation.
    ///
    /// Attaching a producer opts into engine-paced update ticks while mounted;
    /// it does not request animation or repaint on unchanged ticks. Without a
    /// producer, the body remains the static `DialogConfig::body`.
    pub fn with_body_update(self, update: impl FnMut(f32) -> Option<String> + 'static) -> Self {
        self.body.borrow_mut().update = Some(Box::new(update));
        self
    }
}

impl Drop for Dialog {
    fn drop(&mut self) {
        self.handle.set_closed();
    }
}

impl Screen for Dialog {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let widget = DialogWidget::new(
            self.config.clone(),
            self.body.clone(),
            self.pressed_index.clone(),
            self.panel_rect.clone(),
        );
        let _root = cx.mount_child(Box::new(widget));
        &mut self.placeholder
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        self.needs_rebuild = true;
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::KeyDown { key_code, .. } => match key_code {
                KeyCode::Escape => {
                    if !self.config.dismiss_on_escape {
                        if let Some(idx) = self.config.buttons.iter().position(|b| b.is_cancel) {
                            self.pressed_index.set(Some(idx));
                        }
                        return true;
                    }
                    // dismiss_on_escape=true: let engine handle via DismissPolicy
                    false
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn dismiss_policy(&self) -> DismissPolicy {
        DismissPolicy {
            on_escape: self.config.dismiss_on_escape,
            on_outside_click: self.config.dismiss_on_outside_click,
        }
    }

    fn covers_point(&self, logical_pos: Point) -> bool {
        if !self.config.dismiss_on_outside_click {
            // Full-screen capture: no clicks are "outside"
            return true;
        }
        // Only panel interior is "inside" — clicks outside the panel trigger dismissal
        self.panel_rect.get().contains(logical_pos)
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if let Some(idx) = self.pressed_index.take() {
            if let Some(btn) = self.config.buttons.get(idx) {
                self.handle.set_pressed(btn.key.clone());
                cx.nav_close_modal();
            }
        }
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{KeyEvent, KeyboardAuthorityMode, Modifiers};
    use crate::core::rect_authority::{enter_paint_authority, exit_paint_authority};
    use crate::core::scene::SceneBuilder;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::{OverlayHitRegion, WidgetTree};
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use std::collections::HashMap;

    fn test_context<'a>(tree: &'a mut WidgetTree, widget_id: TreeWidgetId) -> EventContext<'a> {
        EventContext::new(
            widget_id,
            tree as *mut WidgetTree,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            KeyboardAuthorityMode::Navigation,
        )
    }

    fn key_event(key_code: KeyCode, shift: bool) -> Event {
        Event::Key(KeyEvent {
            key_code,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            is_down: true,
            is_repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            text: None,
        })
    }

    #[test]
    fn dialog_keyboard_focus_cycles_buttons_and_activates_selected() {
        let pressed = Rc::new(Cell::new(None));
        let panel_rect = Rc::new(Cell::new(RectF::zero()));
        let config = Rc::new(DialogConfig {
            title: "Confirm".to_string(),
            body: "Proceed?".to_string(),
            buttons: vec![
                DialogButton {
                    label: "Cancel".to_string(),
                    key: "cancel".to_string(),
                    is_default: false,
                    is_cancel: true,
                },
                DialogButton {
                    label: "OK".to_string(),
                    key: "ok".to_string(),
                    is_default: true,
                    is_cancel: false,
                },
            ],
            dismiss_on_escape: true,
            dismiss_on_outside_click: true,
        });
        let body = Rc::new(RefCell::new(DialogBody::new(config.body.clone())));
        let mut widget = DialogWidget::new(config, body, Rc::clone(&pressed), panel_rect);
        assert!(!widget.needs_update());
        let mut tree = WidgetTree::new();
        let widget_id = TreeWidgetId(1);

        {
            let mut cx = test_context(&mut tree, widget_id);
            widget.on_focus(&mut cx);
        }
        assert_eq!(widget.focused_button_index.get(), Some(1));

        {
            let mut cx = test_context(&mut tree, widget_id);
            assert!(widget.handle_event(
                &mut cx,
                &key_event(KeyCode::Tab, false),
                EventPhase::Target
            ));
        }
        assert_eq!(widget.focused_button_index.get(), Some(0));

        {
            let mut cx = test_context(&mut tree, widget_id);
            assert!(widget.handle_event(
                &mut cx,
                &key_event(KeyCode::Right, false),
                EventPhase::Target
            ));
        }
        assert_eq!(widget.focused_button_index.get(), Some(1));

        {
            let mut cx = test_context(&mut tree, widget_id);
            assert!(widget.handle_event(
                &mut cx,
                &key_event(KeyCode::Left, false),
                EventPhase::Target
            ));
        }
        assert_eq!(widget.focused_button_index.get(), Some(0));

        {
            let mut cx = test_context(&mut tree, widget_id);
            assert!(widget.handle_event(
                &mut cx,
                &key_event(KeyCode::Up, false),
                EventPhase::Target
            ));
        }
        assert_eq!(widget.focused_button_index.get(), Some(1));

        {
            let mut cx = test_context(&mut tree, widget_id);
            assert!(widget.handle_event(
                &mut cx,
                &key_event(KeyCode::Down, false),
                EventPhase::Target
            ));
        }
        assert_eq!(widget.focused_button_index.get(), Some(0));

        {
            let mut cx = test_context(&mut tree, widget_id);
            assert!(widget.handle_event(
                &mut cx,
                &key_event(KeyCode::Enter, false),
                EventPhase::Target
            ));
        }
        assert_eq!(pressed.get(), Some(0));
    }

    #[test]
    fn dialog_paint_exports_absolute_panel_rect_for_offset_modal_root() {
        let pressed = Rc::new(Cell::new(None));
        let panel_rect_shared = Rc::new(Cell::new(RectF::zero()));
        let config = Rc::new(DialogConfig {
            title: "Confirm".to_string(),
            body: "Proceed?".to_string(),
            buttons: vec![DialogButton {
                label: "OK".to_string(),
                key: "ok".to_string(),
                is_default: true,
                is_cancel: false,
            }],
            dismiss_on_escape: true,
            dismiss_on_outside_click: true,
        });
        let body = Rc::new(RefCell::new(DialogBody::new(config.body.clone())));
        let widget = DialogWidget::new(config, body, pressed, Rc::clone(&panel_rect_shared));
        widget
            .panel_rect
            .set(RectF::new(220.0, 180.0, 360.0, 120.0));
        *widget.button_rects.borrow_mut() = vec![RectF::new(492.0, 256.0, 72.0, 28.0)];
        *widget.body_lines.borrow_mut() = vec!["Proceed?".to_string()];

        let tree = WidgetTree::new();
        let theme = Theme::dark();
        let mut text_engine = FontManagerTextEngine::new();
        let mut scene = SceneBuilder::new();
        let timeline = crate::core::animation::Timeline::new();
        let interpolated_rects = HashMap::new();
        let mut overlay_hit_regions: Vec<OverlayHitRegion> = Vec::new();
        let mut tooltip_triggers = Vec::new();
        let icon_registry = crate::core::icons::builtin_registry();
        let owner_rect = RectF::new(0.0, 28.0, 800.0, 572.0);

        {
            let mut cx = PaintContext::new(
                &tree,
                &mut scene,
                &theme,
                &mut text_engine,
                &icon_registry,
                0,
                &timeline,
                &interpolated_rects,
                &mut overlay_hit_regions,
                &mut tooltip_triggers,
                1.0,
                None,
                KeyboardAuthorityMode::Navigation,
            );
            enter_paint_authority();
            widget.paint(&mut cx, owner_rect, &[]);
            widget.paint_overlay(&mut cx, owner_rect, TreeWidgetId(1));
            exit_paint_authority();
        }

        assert_eq!(
            panel_rect_shared.get(),
            RectF::new(220.0, 208.0, 360.0, 120.0)
        );
        assert_eq!(overlay_hit_regions.len(), 1);
        assert_eq!(overlay_hit_regions[0].owner_abs_rect, owner_rect);
    }

    #[test]
    fn dialog_dynamic_body_updates_retained_modal_without_rebuilding_or_retargeting() {
        use crate::core::app::{AppContext, AppRoot, RetainedSceneTestHarness};
        use crate::core::event::{InteractionDomains, PointerDevice, PointerId, WindowId};
        use crate::core::event_router::EventRouter;
        use crate::core::scene::SceneNode;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::screen_host::ScreenHost;
        use std::sync::Arc;
        use std::time::Instant;

        fn mounted(tree: &WidgetTree, node: NodeId) -> &DialogWidget {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<DialogWidget>()
                .unwrap()
        }

        fn button_center(tree: &WidgetTree, node: NodeId) -> Point {
            let rect = mounted(tree, node).button_rects.borrow()[1];
            let owner = tree.get_absolute_paint_rect(node).unwrap();
            Point::new(
                owner.x + rect.x + rect.width * 0.5,
                owner.y + rect.y + rect.height * 0.5,
            )
        }

        let pending = Rc::new(RefCell::new(None::<String>));
        let ticks = Rc::new(Cell::new(0));
        let handle = DialogHandle::new();
        let dialog = Dialog::new(
            DialogConfig {
                title: "Confirm".into(),
                body: "Remaining: 10".into(),
                buttons: vec![
                    DialogButton {
                        label: "Cancel".into(),
                        key: "cancel".into(),
                        is_default: false,
                        is_cancel: true,
                    },
                    DialogButton {
                        label: "Keep".into(),
                        key: "keep".into(),
                        is_default: true,
                        is_cancel: false,
                    },
                ],
                dismiss_on_escape: true,
                dismiss_on_outside_click: false,
            },
            handle.clone(),
        )
        .with_body_update({
            let pending = pending.clone();
            let ticks = ticks.clone();
            move |dt| {
                assert!(crate::core::rect_authority::is_update_authority_active());
                assert_eq!(dt, 0.0); // The retained harness supplies a deterministic zero delta.
                ticks.set(ticks.get() + 1);
                pending.borrow_mut().take()
            }
        });
        let theme = Theme::dark();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app = AppRoot::new(AppContext::new(theme.clone()), root);
        {
            let mut cx = ScreenContext::new(
                &mut app.context,
                &mut app.screens,
                &mut app.modals,
                Some(&mut tree),
            );
            cx.nav_open_modal(Box::new(dialog));
        }
        app.process_nav_queue();
        app.rebuild_active_view(&mut tree);
        let modal = app.active_modal_input_root_node().unwrap();
        let owner = tree.widget_id_for_node(modal).unwrap();
        let internal_id = mounted(&tree, modal).id();
        let overlay = app.overlay_root.top_id();
        let structure = tree.structure_generation();
        let mut harness = RetainedSceneTestHarness::new();
        let constraints = Constraints::tight(Size::new(800.0, 600.0));
        let mut text = FontManagerTextEngine::new();
        let icons = crate::core::icon::IconRegistry::new(&[]);
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();

        assert!(tree.any_widget_needs_update());
        assert!(!tree.any_widget_needs_animation());
        assert!(
            harness
                .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
                .scene_built
        );
        assert_eq!(ticks.get(), 1);
        let initial_panel = mounted(&tree, modal).panel_rect.get();
        {
            let state = domains.resolve_window_mut(WindowId::PRIMARY).unwrap();
            state.focus_manager.rebuild_focus_chain(&tree, modal);
            router.request_focus_with_text_engine(&mut tree, owner, state, Some(&mut text));
        }
        let press = button_center(&tree, modal);
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: press,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            Some(modal),
            &mut text,
            None,
            domains.resolve_window_mut(WindowId::PRIMARY).unwrap(),
            &mut bindings,
        );
        harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert_eq!(mounted(&tree, modal).active_press_index.get(), Some(1));
        let old_fragment = harness.local_fragment(owner).unwrap();

        let next = "Remaining: 9\nAdditional details".to_string();
        *pending.borrow_mut() = Some(next.clone());
        let changed = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(changed.scene_built && changed.visual_changed);
        assert!(mounted(&tree, modal).panel_rect.get().height > initial_panel.height);
        assert_eq!(mounted(&tree, modal).config.body, "Remaining: 10");
        assert_eq!(
            *mounted(&tree, modal).body_lines.borrow(),
            ["Remaining: 9", "Additional details"]
        );
        let new_fragment = harness.local_fragment(owner).unwrap();
        assert!(!Arc::ptr_eq(&old_fragment, &new_fragment));
        for expected in ["Remaining: 9", "Additional details"] {
            assert!(
                new_fragment
                    .iter()
                    .any(|node| { matches!(node, SceneNode::Text(text) if text.text == expected) })
            );
        }
        app.rebuild_active_view(&mut tree);
        assert_eq!(app.active_modal_input_root_node(), Some(modal));
        assert_eq!(app.overlay_root.top_id(), overlay);
        assert_eq!(tree.structure_generation(), structure);
        assert_eq!(mounted(&tree, modal).id(), internal_id);
        assert_eq!(mounted(&tree, modal).active_press_index.get(), Some(1));
        assert_eq!(mounted(&tree, modal).focused_button_index.get(), Some(1));
        assert_eq!(mounted(&tree, modal).pressed_index.get(), None);
        let state = domains.resolve_window_mut(WindowId::PRIMARY).unwrap();
        assert_eq!(state.focus_manager.focused(), Some(owner));
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(state.pointer_capture.captured_widget, Some(owner));
        assert_eq!(
            state
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        assert!(handle.is_open());

        for unchanged in [Some(next), None] {
            *pending.borrow_mut() = unchanged;
            let before_ticks = ticks.get();
            let clean = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
            assert_eq!(ticks.get(), before_ticks + 1);
            assert!(!clean.scene_built && !clean.visual_changed);
            assert!(!clean.needs_next_frame);
            assert!(tree.any_widget_needs_update());
            assert!(Arc::ptr_eq(
                &new_fragment,
                &harness.local_fragment(owner).unwrap()
            ));
        }

        let release = button_center(&tree, modal);
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: release,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
            Some(modal),
            &mut text,
            None,
            domains.resolve_window_mut(WindowId::PRIMARY).unwrap(),
            &mut bindings,
        );
        assert_eq!(mounted(&tree, modal).pressed_index.get(), Some(1));
        app.flush_pending_actions_all(&mut tree);
        app.process_nav_queue();
        app.rebuild_active_view(&mut tree);
        assert_eq!(handle.pressed_key().as_deref(), Some("keep"));
        assert!(!handle.is_open());
        assert!(tree.node(modal).is_none());
        assert!(app.overlay_root.top_id().is_none());
        assert!(!tree.any_widget_needs_update());
    }
}
