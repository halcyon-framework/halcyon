// DatePickerField — Trigger surface + anchored popup for DatePickerWidget.
// Follows ComboBox/DropdownMenu overlay pattern.

use std::cell::Cell;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::icon::IconId;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, RoundedCorners};
use crate::core::popup::compute_popup_rect;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, OverlayHitRegion};
use crate::model::date_value::DateValue;
use crate::model::form_binder::ChangeWriter;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::date_picker::{
    DATEPICKER_H, DATEPICKER_W, DateMark, DatePickerState, DatePickerWidget, DatePolicy,
};

/// Controls when the DatePickerField popup closes after selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFieldCommitMode {
    /// Single click on a date commits and closes the popup.
    /// Default for single-date pickers.
    CommitOnClick,
    /// Single click highlights, popup stays open. Enter or
    /// double-click commits and closes.
    CommitOnDoubleClick,
    /// Popup stays open until explicitly closed (Escape, outside
    /// click, or Enter). Selection is updated live but popup
    /// remains for browsing.
    ExplicitCommit,
}

pub struct DatePickerField {
    id: WidgetInternalId,
    picker: DatePickerWidget,
    popup_open: bool,
    focused: bool,
    hovered: bool,
    in_capture: bool,
    placeholder: String,
    cached_trigger_rect: Cell<RectF>,
    cached_open_above: Cell<bool>,
    /// Popup origin offset relative to trigger origin, in trigger-local
    /// coordinates. (popup_rect.x - trigger.x, popup_rect.y - trigger.y).
    /// Populated by paint_overlay (via compute_popup_rect) and consumed
    /// by handle_event for hit-testing.
    cached_popup_offset: Cell<(f32, f32)>,
    commit_mode: DateFieldCommitMode,
    /// Snapshot of selected date when popup opened — for Cancel revert.
    pre_open_date: Option<DateValue>,
    /// Cached OK button rect (popup-local coordinates) for hit-testing.
    cached_ok_rect: Cell<RectF>,
    /// Cached Cancel button rect (popup-local coordinates) for hit-testing.
    cached_cancel_rect: Cell<RectF>,
    /// Trigger height computed in layout from theme; consumed by handle_event.
    cached_trigger_height: Cell<f32>,
}

impl DatePickerField {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            picker: DatePickerWidget::new(None, None, None, 0).with_embedded(true),
            popup_open: false,
            focused: false,
            hovered: false,
            in_capture: false,
            placeholder: String::new(),
            cached_trigger_rect: Cell::new(RectF::zero()),
            cached_open_above: Cell::new(false),
            cached_popup_offset: Cell::new((0.0, 0.0)),
            commit_mode: DateFieldCommitMode::CommitOnClick,
            pre_open_date: None,
            cached_ok_rect: Cell::new(RectF::zero()),
            cached_cancel_rect: Cell::new(RectF::zero()),
            cached_trigger_height: Cell::new(24.0),
        }
    }

    pub fn with_commit_mode(mut self, mode: DateFieldCommitMode) -> Self {
        self.commit_mode = mode;
        self
    }

    pub fn with_today(mut self, today: DateValue) -> Self {
        self.picker = self.picker.with_today(today);
        self
    }

    pub fn with_state(mut self, state: DatePickerState) -> Self {
        *self.picker.state_mut() = state;
        self
    }

    pub fn with_policy(mut self, policy: DatePolicy) -> Self {
        self.picker = self.picker.with_policy(policy);
        self
    }

    pub fn with_marks(mut self, marks: Vec<DateMark>) -> Self {
        self.picker = self.picker.with_marks(marks);
        self
    }

    pub fn with_binding(mut self, key: &str, writer: ChangeWriter) -> Self {
        self.picker = self.picker.with_binding(key, writer);
        self
    }

    pub fn with_placeholder(mut self, s: &str) -> Self {
        self.placeholder = s.to_string();
        self
    }

    pub fn state(&self) -> &DatePickerState {
        self.picker.state()
    }

    pub fn state_mut(&mut self) -> &mut DatePickerState {
        self.picker.state_mut()
    }

    fn shows_buttons(&self) -> bool {
        self.commit_mode != DateFieldCommitMode::CommitOnClick
    }

    /// Effective popup height (taller when buttons are shown).
    fn popup_height(&self) -> f32 {
        if self.shows_buttons() {
            DATEPICKER_H + 32.0
        } else {
            DATEPICKER_H
        }
    }

    fn cancel_and_close(&mut self, cx: &mut EventContext) {
        if self.shows_buttons() {
            self.picker.state_mut().set_selected(self.pre_open_date);
        }
        self.popup_open = false;
        cx.release_keyboard_capture();
    }

    fn commit_and_close(&mut self, cx: &mut EventContext) {
        self.popup_open = false;
        cx.release_keyboard_capture();
    }
}

impl WidgetDowncast for DatePickerField {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DatePickerField {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Run picker layout so cell_rects are populated for paint_overlay
        let picker_constraints = Constraints {
            min_width: DATEPICKER_W,
            max_width: DATEPICKER_W,
            min_height: DATEPICKER_H,
            max_height: DATEPICKER_H,
        };
        self.picker.layout(cx, picker_constraints, &[]);

        let width = DATEPICKER_W
            .min(constraints.max_width)
            .max(constraints.min_width);
        let height = cx.theme.geometry.text_field.height;
        self.cached_trigger_height.set(height);
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.cached_trigger_rect.set(rect);
        let t = cx.theme;
        let radius = t.geometry.combo_box.radius;

        // Border color: focus/capture/default via control_border
        let (border_color, border_width) =
            t.control_border(cx.has_focus_cue(), cx.has_capture(), false);

        // Background fill
        cx.scene.push_rounded_rect(
            rect,
            t.colors.surfaces.content,
            RoundedCorners::uniform(radius),
            2,
            cx.scene_key,
        );

        // Border stroke
        cx.scene.push_border_rounded_rect(
            rect,
            border_color,
            border_width,
            RoundedCorners::uniform(radius),
            3,
            None,
        );

        // Label
        let label = if self.popup_open {
            String::new()
        } else {
            self.picker
                .state()
                .selected()
                .map(|d| d.to_iso8601())
                .unwrap_or_else(|| self.placeholder.clone())
        };
        let font_size = t.typography.font_size;
        let padding_x = t.list_geom.padding_x;
        let label_run = cx.text_engine.shape(&label, font_size);
        let label_x = rect.x + padding_x;
        let label_y = rect.y + (rect.height - label_run.height) / 2.0;
        let label_color = if self.picker.state().selected().is_some() && !self.popup_open {
            t.colors.list.text
        } else {
            t.colors.typography.muted
        };
        cx.scene.push_text_with_backgrounds(
            &label_run,
            label_x,
            label_y,
            label_color,
            vec![],
            crate::core::paint::LAYER_CONTENT + 1,
            None,
        );

        // Affordance indicator — same bar pattern as ComboBox
        let aw = t.geometry.combo_box.affordance_width;
        let aff_rect = RectF::new(rect.x + rect.width - aw, rect.y, aw, rect.height);
        let indicator_color = t.colors.typography.primary;
        let bar_width = (aw * 0.45).round();
        let bar_height = 1.5;
        let bar_x = aff_rect.x + (aff_rect.width - bar_width) / 2.0;
        let bar_radius = crate::core::paint::RoundedCorners::uniform(0.5);

        if self.popup_open {
            // Open: single bar
            let bar_y = aff_rect.y + (aff_rect.height - bar_height) / 2.0;
            cx.scene.push_rounded_rect(
                RectF::new(bar_x, bar_y, bar_width, bar_height),
                indicator_color,
                bar_radius,
                crate::core::paint::LAYER_CONTENT,
                None,
            );
        } else {
            // Closed: two bars
            let gap = 3.0;
            let total_h = bar_height * 2.0 + gap;
            let top_y = aff_rect.y + (aff_rect.height - total_h) / 2.0;
            cx.scene.push_rounded_rect(
                RectF::new(bar_x, top_y, bar_width, bar_height),
                indicator_color,
                bar_radius,
                crate::core::paint::LAYER_CONTENT,
                None,
            );
            cx.scene.push_rounded_rect(
                RectF::new(bar_x, top_y + bar_height + gap, bar_width, bar_height),
                indicator_color,
                bar_radius,
                crate::core::paint::LAYER_CONTENT,
                None,
            );
        }

        // Calendar icon — left of affordance zone
        let icon_size = 14.0;
        let icon_margin = 6.0;
        let icon_x = aff_rect.x - icon_margin - icon_size;
        let icon_y = rect.y + (rect.height - icon_size) / 2.0;
        if let Some(def) = cx.icon_registry.get(IconId::CALENDAR) {
            cx.scene.push_icon(
                def,
                icon_x,
                icon_y,
                icon_size,
                t.colors.typography.secondary,
                crate::core::paint::LAYER_CONTENT + 1,
            );
        }
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        if !self.popup_open {
            return;
        }

        let popup_h = self.popup_height();
        let (viewport_w, viewport_h) = cx.viewport_size();
        let (popup_rect, open_above) =
            compute_popup_rect(rect, DATEPICKER_W, popup_h, viewport_w, viewport_h);
        self.cached_open_above.set(open_above);
        self.cached_popup_offset
            .set((popup_rect.x - rect.x, popup_rect.y - rect.y));

        // Full-window backdrop for click-away dismissal (registered first, checked last)
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(0.0, 0.0, 100_000.0, 100_000.0),
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });

        // Popup region (registered second, checked first)
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: popup_rect,
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });

        // Popup border (push_border_rect at LAYER_OVERLAY — same pattern as MenuSurface).
        {
            use crate::core::paint::LAYER_OVERLAY;
            cx.scene.push_border_rect(
                popup_rect,
                cx.theme.colors.borders.default,
                1.0,
                LAYER_OVERLAY,
                None,
            );
        }

        // Background blocker: covers z=0 text from siblings painted after paint_overlay.
        // TextBackgroundSpan at LAYER_OVERLAY + 1 renders in text pass after z=0 content
        // (occluding it) but before picker text at LAYER_OVERLAY + 2.
        {
            use crate::core::paint::{LAYER_OVERLAY, TextBackgroundSpan};
            let bg_color = cx.theme.colors.surfaces.content;
            let bg_run = cx.text_engine.shape(" ", cx.theme.typography.font_size);
            cx.scene.push_text_with_backgrounds(
                &bg_run,
                popup_rect.x,
                popup_rect.y,
                bg_color,
                vec![TextBackgroundSpan {
                    x_offset: 0.0,
                    y_offset: 0.0,
                    width: DATEPICKER_W,
                    height: popup_h,
                    color: bg_color,
                }],
                LAYER_OVERLAY + 1,
                None,
            );
        }

        // Delegate to picker's paint
        self.picker.paint(cx, popup_rect, &[]);

        if self.shows_buttons() {
            use crate::core::paint::LAYER_OVERLAY;
            let bar_y = popup_rect.y + DATEPICKER_H + 4.0;
            let btn_h = 20.0;
            let btn_w = 50.0;
            let btn_gap = 6.0;
            let pad = 8.0;

            // OK button (right-aligned)
            let ok_x = popup_rect.x + DATEPICKER_W - pad - btn_w;
            let ok_rect = RectF::new(ok_x, bar_y, btn_w, btn_h);
            cx.scene.push_rounded_rect(
                ok_rect,
                cx.theme.colors.accents.primary,
                RoundedCorners::uniform(3.0),
                LAYER_OVERLAY + 2,
                None,
            );
            let ok_run = cx.text_engine.shape("OK", cx.theme.typography.font_size);
            let (ok_tw, _) = cx.text_engine.measure("OK", cx.theme.typography.font_size);
            cx.scene.push_text_with_backgrounds(
                &ok_run,
                ok_x + (btn_w - ok_tw) / 2.0,
                bar_y + 3.0,
                Color::WHITE,
                vec![],
                LAYER_OVERLAY + 3,
                None,
            );

            // Cancel button (left of OK)
            let cancel_x = ok_x - btn_gap - btn_w;
            let cancel_rect = RectF::new(cancel_x, bar_y, btn_w, btn_h);
            cx.scene.push_rounded_rect(
                cancel_rect,
                cx.theme.colors.surfaces.level1,
                RoundedCorners::uniform(3.0),
                LAYER_OVERLAY + 2,
                None,
            );
            let cancel_run = cx
                .text_engine
                .shape("Cancel", cx.theme.typography.font_size);
            let (cancel_tw, _) = cx
                .text_engine
                .measure("Cancel", cx.theme.typography.font_size);
            cx.scene.push_text_with_backgrounds(
                &cancel_run,
                cancel_x + (btn_w - cancel_tw) / 2.0,
                bar_y + 3.0,
                cx.theme.colors.typography.primary,
                vec![],
                LAYER_OVERLAY + 3,
                None,
            );

            // Cache rects in popup-local coordinates
            self.cached_ok_rect.set(RectF::new(
                ok_x - popup_rect.x,
                bar_y - popup_rect.y,
                btn_w,
                btn_h,
            ));
            self.cached_cancel_rect.set(RectF::new(
                cancel_x - popup_rect.x,
                bar_y - popup_rect.y,
                btn_w,
                btn_h,
            ));
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        let popup_h = self.popup_height();
        let trigger_h = self.cached_trigger_height.get();
        let (off_x, off_y) = self.cached_popup_offset.get();
        match event {
            Event::Pointer(pe) => {
                let (lx, ly) = pe.local_pos;

                match pe.kind {
                    PointerEventKind::Move => {
                        if self.popup_open {
                            let popup_local_x = lx - off_x;
                            let popup_local_y = ly - off_y;
                            let in_popup = popup_local_x >= 0.0
                                && popup_local_x < DATEPICKER_W
                                && popup_local_y >= 0.0
                                && popup_local_y < popup_h;
                            if in_popup {
                                let mut translated = pe.clone();
                                translated.local_pos = (popup_local_x, popup_local_y);
                                let consumed = self.picker.handle_event(
                                    cx,
                                    &Event::Pointer(translated),
                                    EventPhase::Target,
                                );
                                return consumed;
                            }
                        }
                        self.hovered = ly >= 0.0 && ly < trigger_h;
                        true
                    }
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }

                        if self.popup_open {
                            let popup_local_x = lx - off_x;
                            let popup_local_y = ly - off_y;
                            let in_popup = popup_local_x >= 0.0
                                && popup_local_x < DATEPICKER_W
                                && popup_local_y >= 0.0
                                && popup_local_y < popup_h;
                            if in_popup {
                                if self.shows_buttons() {
                                    let ok_r = self.cached_ok_rect.get();
                                    if ok_r.width > 0.0
                                        && popup_local_x >= ok_r.x
                                        && popup_local_x <= ok_r.x + ok_r.width
                                        && popup_local_y >= ok_r.y
                                        && popup_local_y <= ok_r.y + ok_r.height
                                    {
                                        self.commit_and_close(cx);
                                        return true;
                                    }
                                    let cancel_r = self.cached_cancel_rect.get();
                                    if cancel_r.width > 0.0
                                        && popup_local_x >= cancel_r.x
                                        && popup_local_x <= cancel_r.x + cancel_r.width
                                        && popup_local_y >= cancel_r.y
                                        && popup_local_y <= cancel_r.y + cancel_r.height
                                    {
                                        self.cancel_and_close(cx);
                                        return true;
                                    }
                                }
                                let mut translated = pe.clone();
                                translated.local_pos = (popup_local_x, popup_local_y);
                                let consumed = self.picker.handle_event(
                                    cx,
                                    &Event::Pointer(translated),
                                    EventPhase::Target,
                                );
                                let changed = self.picker.state_mut().take_changed().is_some();
                                match self.commit_mode {
                                    DateFieldCommitMode::CommitOnClick => {
                                        if changed {
                                            self.popup_open = false;
                                            cx.release_keyboard_capture();
                                        }
                                    }
                                    DateFieldCommitMode::CommitOnDoubleClick => {
                                        // Single click selects but doesn't close.
                                        // DoubleClick on a date cell closes.
                                        if changed && pe.kind == PointerEventKind::DoubleClick {
                                            self.popup_open = false;
                                            cx.release_keyboard_capture();
                                        }
                                    }
                                    DateFieldCommitMode::ExplicitCommit => {
                                        // Selection updates live, popup stays open.
                                        // User closes via Escape, Enter, or outside click.
                                    }
                                }
                                return consumed;
                            } else {
                                // Outside popup — dismiss (backdrop click)
                                self.cancel_and_close(cx);
                                return true;
                            }
                        } else {
                            self.pre_open_date = self.picker.state().selected();
                            self.popup_open = true;
                            cx.request_keyboard_capture();
                            return true;
                        }
                    }
                    PointerEventKind::Up => {
                        if self.popup_open {
                            let popup_local_x = lx - off_x;
                            let popup_local_y = ly - off_y;
                            let in_popup = popup_local_x >= 0.0
                                && popup_local_x < DATEPICKER_W
                                && popup_local_y >= 0.0
                                && popup_local_y < popup_h;
                            if in_popup {
                                let mut translated = pe.clone();
                                translated.local_pos = (popup_local_x, popup_local_y);
                                return self.picker.handle_event(
                                    cx,
                                    &Event::Pointer(translated),
                                    EventPhase::Target,
                                );
                            }
                        }
                        false
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down && self.popup_open => {
                if ke.key_code == KeyCode::Escape {
                    self.cancel_and_close(cx);
                    return true;
                }
                if ke.key_code == KeyCode::Enter || matches!(ke.key_code, KeyCode::Character('i')) {
                    // SPEC-HAL-176: `i` is the Navigation-mode Enter alias.
                    // Both Enter and `i` commit the cursor's date and close
                    // the popup unconditionally — they bypass `commit_mode`
                    // (CommitOnClick / CommitOnDoubleClick / ExplicitCommit),
                    // which governs pointer interaction only. JC's spec:
                    // "no matter what interaction policy of DatePicker
                    // dictates, pressing `i` on the date will select AND
                    // commit it."
                    //
                    // If year-jump buffer is active, the picker's year-jump
                    // apply block consumes Enter or `i` to apply the partial
                    // buffer (no popup close). Otherwise forward, drain, and
                    // close synchronously — explicit popup_open=false plus
                    // release_keyboard_capture for same-frame consistency,
                    // mirroring the prior Enter path.
                    let consumed = self.picker.handle_event(cx, event, EventPhase::Target);
                    self.picker.state_mut().take_changed(); // drain
                    if !self.picker.is_year_jump_active() {
                        self.popup_open = false;
                        cx.release_keyboard_capture();
                    }
                    return consumed || true;
                }
                // All other keys: forward to picker
                self.picker.handle_event(cx, event, EventPhase::Target)
            }
            Event::Key(ke) if ke.is_down => {
                if ke.key_code == KeyCode::Enter || ke.key_code == KeyCode::Space {
                    self.popup_open = true;
                    cx.request_keyboard_capture();
                    return true;
                }
                false
            }
            Event::Text(te) if self.popup_open => {
                let consumed = self.picker.handle_event(cx, event, EventPhase::Target);
                self.picker.state_mut().take_changed(); // drain, do not close
                consumed
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.focused = true;
        self.picker.on_focus(cx);
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.focused = false;
        self.picker.on_blur(cx);
    }

    fn on_capture_enter(&mut self, cx: &mut EventContext) {
        self.in_capture = true;
        self.popup_open = true;
        self.picker.on_capture_enter(cx);
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.in_capture = false;
        self.popup_open = false;
        self.picker.on_capture_leave(cx);
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "DatePickerField"
    }
}
