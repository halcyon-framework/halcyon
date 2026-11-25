// SPEC-HAL-272: ColorPickerField — Trigger swatch + anchored popup for ColorPicker.
// Follows DatePickerField overlay pattern.

use std::cell::Cell;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{Color, RoundedCorners, TextBackgroundSpan};
use crate::core::popup::compute_popup_rect;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, OverlayHitRegion};
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::color_picker::ColorPicker;

pub struct ColorPickerField {
    id: WidgetInternalId,
    picker: ColorPicker,
    popup_open: bool,
    focused: bool,
    hovered: bool,
    in_capture: bool,
    cached_trigger_rect: Cell<RectF>,
    cached_open_above: Cell<bool>,
    /// Popup origin offset relative to trigger origin, in trigger-local
    /// coordinates. (popup_rect.x - trigger.x, popup_rect.y - trigger.y).
    /// Populated by paint_overlay (via compute_popup_rect) and consumed
    /// by handle_event for hit-testing.
    cached_popup_offset: Cell<(f32, f32)>,
    // Cached geometry from layout (EventContext has no theme access)
    cached_popup_w: f32,
    cached_popup_h: f32,
    cached_popup_pad: f32,
    cached_trigger_h: f32,
    cell_swatch_size: Size,
    /// Color snapshot when popup opened — used for Cancel/revert.
    pre_open_color: Color,
    /// OK button rect in popup-local coordinates (popup origin = 0,0).
    cached_ok_rect: Cell<RectF>,
    /// Cancel button rect in popup-local coordinates.
    cached_cancel_rect: Cell<RectF>,
    /// Fired when the user commits via the OK button. Argument: the selected color.
    on_committed: Option<Box<dyn Fn(Color)>>,
    /// Fired when the user cancels via Cancel button, outside-popup click, or Escape.
    /// The widget reverts the color to its pre-open value before this fires.
    on_cancelled: Option<Box<dyn Fn()>>,
}

impl ColorPickerField {
    pub(crate) fn cell_editor_visual_facts(&self) -> crate::core::appearance::CellEditorVisualFacts {
        crate::core::appearance::CellEditorVisualFacts { disabled: false, hovered: self.hovered }
    }

    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            picker: ColorPicker::new().with_embedded(true),
            popup_open: false,
            focused: false,
            hovered: false,
            in_capture: false,
            cached_trigger_rect: Cell::new(RectF::zero()),
            cached_open_above: Cell::new(false),
            cached_popup_offset: Cell::new((0.0, 0.0)),
            cached_popup_w: 0.0,
            cached_popup_h: 0.0,
            cached_popup_pad: 0.0,
            cached_trigger_h: 0.0,
            cell_swatch_size: Size::zero(),
            pre_open_color: Color::WHITE,
            cached_ok_rect: Cell::new(RectF::zero()),
            cached_cancel_rect: Cell::new(RectF::zero()),
            on_committed: None,
            on_cancelled: None,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.picker.set_color(color);
        self
    }

    pub fn color(&self) -> Color {
        self.picker.color()
    }

    pub fn hex_string(&self) -> String {
        self.picker.hex_string()
    }

    pub fn picker(&self) -> &ColorPicker {
        &self.picker
    }

    pub fn picker_mut(&mut self) -> &mut ColorPicker {
        &mut self.picker
    }

    /// Set the callback fired when the user commits via the OK button.
    /// The argument is the color selected at commit time.
    pub fn set_on_committed<F: Fn(Color) + 'static>(&mut self, f: F) {
        self.on_committed = Some(Box::new(f));
    }

    /// Set the callback fired when the user cancels via the Cancel button,
    /// a click outside the popup, or Escape. The color is reverted to its
    /// pre-open value by the widget before this fires.
    pub fn set_on_cancelled<F: Fn() + 'static>(&mut self, f: F) {
        self.on_cancelled = Some(Box::new(f));
    }
}

impl Default for ColorPickerField {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for ColorPickerField {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ColorPickerField {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.color_picker;
        let sv = geom.sv_size;
        let pad = geom.popup_padding;
        let picker_inner_h = sv + geom.strip_gap + geom.hue_strip_height;
        let button_bar_h = 28.0;
        let picker_constraints = Constraints {
            min_width: sv,
            max_width: sv,
            min_height: picker_inner_h,
            max_height: picker_inner_h,
        };
        self.picker.layout(cx, picker_constraints, &[]);

        // Cache geometry for handle_event (EventContext has no theme)
        self.cached_popup_w = sv + 2.0 * pad;
        self.cached_popup_h = picker_inner_h + button_bar_h + 2.0 * pad;
        self.cached_popup_pad = pad;
        self.cached_trigger_h = geom.swatch_height;
        self.cell_swatch_size = Size::new(
            geom.swatch_width.min(constraints.max_width).max(0.0),
            geom.swatch_height.min(constraints.max_height).max(0.0),
        );

        let width = geom
            .swatch_width
            .max(constraints.min_width)
            .min(constraints.max_width);
        LayoutResult::fit(Size::new(width, geom.swatch_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.cached_trigger_rect.set(rect);
        let t = cx.theme;
        let geom = &t.geometry.color_picker;

        if cx.cell_editor_presentation() == crate::core::appearance::CellEditorPresentation::CellIntegrated {
            cx.scene.push_rect(rect, t.colors.surfaces.content, 2, cx.scene_key);
            let mark = RectF::new(rect.x + (rect.width - self.cell_swatch_size.width) * 0.5,
                rect.y + (rect.height - self.cell_swatch_size.height) * 0.5,
                self.cell_swatch_size.width, self.cell_swatch_size.height);
            cx.scene.push_rounded_rect(mark, self.picker.color(),
                RoundedCorners::uniform(geom.swatch_radius), 2, cx.scene_key);
            cx.scene.push_border_rounded_rect(mark, t.colors.borders.default, cx.one_px(),
                RoundedCorners::uniform(geom.swatch_radius), 3, None);
            return;
        }

        // Current color as fill
        let fill_color = self.picker.color();

        // Border
        let (border_color, border_width) =
            t.control_border(cx.has_focus_cue(), cx.has_capture(), false);

        // Swatch fill
        cx.scene.push_rounded_rect(
            rect,
            fill_color,
            RoundedCorners::uniform(geom.swatch_radius),
            2,
            cx.scene_key,
        );

        // Border stroke
        cx.scene.push_border_rounded_rect(
            rect,
            border_color,
            border_width,
            RoundedCorners::uniform(geom.swatch_radius),
            3,
            None,
        );
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        if !self.popup_open {
            return;
        }

        let geom = &cx.theme.geometry.color_picker;
        let sv = geom.sv_size;
        let pad = geom.popup_padding;
        let popup_w = sv + 2.0 * pad;
        let picker_inner_h = sv + geom.strip_gap + geom.hue_strip_height;
        let button_bar_h = 28.0;
        let popup_inner_h = picker_inner_h + button_bar_h;
        let popup_h = popup_inner_h + 2.0 * pad;
        let (viewport_w, viewport_h) = cx.viewport_size();
        let (popup_rect, open_above) =
            compute_popup_rect(rect, popup_w, popup_h, viewport_w, viewport_h);
        self.cached_open_above.set(open_above);
        self.cached_popup_offset
            .set((popup_rect.x - rect.x, popup_rect.y - rect.y));

        // Full-window backdrop for click-away dismissal
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(0.0, 0.0, 100_000.0, 100_000.0),
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });

        // Popup region
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: popup_rect,
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });

        // Popup border
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

        // Popup background — MUST be text pipeline (overlay pass has no text rendering)
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
                    width: popup_w,
                    height: popup_h,
                    color: bg_color,
                }],
                LAYER_OVERLAY + 1,
                None,
            );
        }

        // Picker content rect (inset by popup_padding)
        let picker_rect = RectF::new(popup_rect.x + pad, popup_rect.y + pad, sv, picker_inner_h);

        // Delegate to picker's paint
        self.picker.paint(cx, picker_rect, &[]);

        // Button bar: OK (right) and Cancel (left of OK)
        use crate::core::paint::LAYER_OVERLAY;
        let bar_y = popup_rect.y + pad + sv + geom.strip_gap + geom.hue_strip_height + 4.0;
        let btn_h = 20.0;
        let btn_w = 50.0;
        let btn_gap = 6.0;

        // OK button — unified: background span + text in one call
        let ok_x = popup_rect.x + popup_w - pad - btn_w;
        let ok_run = cx.text_engine.shape("OK", cx.theme.typography.font_size);
        let (ok_tw, _) = cx.text_engine.measure("OK", cx.theme.typography.font_size);
        let ok_text_x = ok_x + (btn_w - ok_tw) / 2.0;
        let ok_text_y = bar_y + (btn_h - ok_run.height) / 2.0;
        cx.scene.push_text_with_backgrounds(
            &ok_run,
            ok_text_x,
            ok_text_y,
            Color::WHITE,
            vec![TextBackgroundSpan {
                x_offset: ok_x - ok_text_x,
                y_offset: -(btn_h - ok_run.height) / 2.0,
                width: btn_w,
                height: btn_h,
                color: cx.theme.colors.accents.primary,
            }],
            LAYER_OVERLAY + 2,
            None,
        );

        // Cancel button — unified: background span + text in one call
        let cancel_x = ok_x - btn_gap - btn_w;
        let cancel_run = cx
            .text_engine
            .shape("Cancel", cx.theme.typography.font_size);
        let (cancel_tw, _) = cx
            .text_engine
            .measure("Cancel", cx.theme.typography.font_size);
        let cancel_text_x = cancel_x + (btn_w - cancel_tw) / 2.0;
        let cancel_text_y = bar_y + (btn_h - cancel_run.height) / 2.0;
        cx.scene.push_text_with_backgrounds(
            &cancel_run,
            cancel_text_x,
            cancel_text_y,
            cx.theme.colors.typography.primary,
            vec![TextBackgroundSpan {
                x_offset: cancel_x - cancel_text_x,
                y_offset: -(btn_h - cancel_run.height) / 2.0,
                width: btn_w,
                height: btn_h,
                color: cx.theme.colors.surfaces.level1,
            }],
            LAYER_OVERLAY + 2,
            None,
        );

        // Cache button rects in popup-local coordinates for hit-testing.
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

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        let popup_w = self.cached_popup_w;
        let popup_h = self.cached_popup_h;
        let pad = self.cached_popup_pad;
        let trigger_h = self.cached_trigger_h;
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
                                && popup_local_x < popup_w
                                && popup_local_y >= 0.0
                                && popup_local_y < popup_h;
                            if in_popup {
                                let mut translated = pe.clone();
                                translated.local_pos = (popup_local_x - pad, popup_local_y - pad);
                                return self.picker.handle_event(
                                    cx,
                                    &Event::Pointer(translated),
                                    EventPhase::Target,
                                );
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
                                && popup_local_x < popup_w
                                && popup_local_y >= 0.0
                                && popup_local_y < popup_h;
                            if in_popup {
                                let click = Point::new(popup_local_x, popup_local_y);

                                let ok_r = self.cached_ok_rect.get();
                                if ok_r.width > 0.0 && ok_r.contains(click) {
                                    // OK = commit (keep current color)
                                    self.popup_open = false;
                                    cx.release_keyboard_capture();
                                    let committed = self.picker.color();
                                    if let Some(cb) = self.on_committed.as_ref() {
                                        cb(committed);
                                    }
                                    return true;
                                }
                                let cancel_r = self.cached_cancel_rect.get();
                                if cancel_r.width > 0.0 && cancel_r.contains(click) {
                                    // Cancel = revert
                                    self.picker.set_color(self.pre_open_color);
                                    self.popup_open = false;
                                    cx.release_keyboard_capture();
                                    if let Some(cb) = self.on_cancelled.as_ref() {
                                        cb();
                                    }
                                    return true;
                                }

                                let mut translated = pe.clone();
                                translated.local_pos = (popup_local_x - pad, popup_local_y - pad);
                                return self.picker.handle_event(
                                    cx,
                                    &Event::Pointer(translated),
                                    EventPhase::Target,
                                );
                            } else {
                                // Outside popup — Cancel: revert to pre-open color
                                self.picker.set_color(self.pre_open_color);
                                self.popup_open = false;
                                if let Some(cb) = self.on_cancelled.as_ref() {
                                    cb();
                                }
                                return true;
                            }
                        } else {
                            self.pre_open_color = self.picker.color();
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
                                && popup_local_x < popup_w
                                && popup_local_y >= 0.0
                                && popup_local_y < popup_h;
                            if in_popup {
                                let mut translated = pe.clone();
                                translated.local_pos = (popup_local_x - pad, popup_local_y - pad);
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
                    // Cancel: revert to pre-open color
                    self.picker.set_color(self.pre_open_color);
                    self.popup_open = false;
                    cx.release_keyboard_capture();
                    if let Some(cb) = self.on_cancelled.as_ref() {
                        cb();
                    }
                    return true;
                }
                false
            }
            Event::Key(ke) if ke.is_down => {
                if ke.key_code == KeyCode::Enter || ke.key_code == KeyCode::Space {
                    self.pre_open_color = self.picker.color();
                    self.popup_open = true;
                    cx.request_keyboard_capture();
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

    fn focusable(&self) -> bool {
        true
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        self.in_capture = true;
        self.popup_open = true;
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        self.in_capture = false;
        self.popup_open = false;
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "ColorPickerField"
    }
}
