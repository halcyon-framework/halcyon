// SPEC-HAL-273: TimePicker — Column-based time selection widget.
// Monolithic widget (no child composition). Renders hour:minute[:second] columns directly.

use std::cell::Cell;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{
    Color, LAYER_CONTENT, LAYER_FOCUS, LAYER_OVERLAY, ShadowMode, TextBackgroundSpan,
};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::theme::ControlChromeState;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
pub use crate::model::time_value::{TimeDisplayMode, TimeValue};
use crate::utils::id::WidgetInternalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeColumn {
    Hour,
    Minute,
    Second,
    AmPm,
}

/// Editing policy for TimePicker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeEditPolicy {
    #[default]
    Full,
    AdjustOnly,
    ReadOnly,
}

/// Time domain: bounds and step configuration.
#[derive(Debug, Clone, Copy)]
pub struct TimeDomain {
    pub min: TimeValue,
    pub max: TimeValue,
    pub step_minutes: u8,
    pub step_seconds: u8,
    pub wrap: bool,
}

impl TimeDomain {
    pub fn full() -> Self {
        Self {
            min: TimeValue::new(0, 0, None),
            max: TimeValue::new(23, 59, Some(59)),
            step_minutes: 1,
            step_seconds: 1,
            wrap: true,
        }
    }

    pub fn working_hours() -> Self {
        Self {
            min: TimeValue::new(9, 0, None),
            max: TimeValue::new(17, 0, None),
            step_minutes: 15,
            step_seconds: 1,
            wrap: false,
        }
    }

    pub fn custom(min: TimeValue, max: TimeValue) -> Self {
        Self {
            min,
            max,
            step_minutes: 1,
            step_seconds: 1,
            wrap: true,
        }
    }
}

impl Default for TimeDomain {
    fn default() -> Self {
        Self::full()
    }
}

pub struct TimePicker {
    id: WidgetInternalId,
    /// Committed value.
    value: TimeValue,
    /// Working value (uncommitted, modified by user interaction).
    working: TimeValue,
    /// Display mode.
    display_mode: TimeDisplayMode,
    /// Whether to show the seconds column.
    show_seconds: bool,
    /// Which column has focus.
    focused_column: TimeColumn,
    /// Whether the widget has keyboard focus.
    focused: bool,
    /// Whether in capture (editing) mode.
    in_capture: bool,
    /// Whether in embedded mode (inside a popup).
    embedded: bool,
    /// Domain: bounds and step configuration.
    domain: TimeDomain,
    /// Editing policy.
    edit_policy: TimeEditPolicy,
    /// Callback fired on commit.
    on_commit: Option<Box<dyn FnMut(TimeValue)>>,
    /// Digit entry buffer for direct typing.
    digit_buffer: Option<u8>,
    /// Cached layout width for event handling.
    cached_width: Cell<f32>,
    /// Cached column geometry for column_at_x (EventContext has no theme).
    cached_column_width: Cell<f32>,
    cached_separator_width: Cell<f32>,
    cached_ampm_width: Cell<f32>,
}

impl TimePicker {
    pub fn new(value: TimeValue) -> Self {
        Self {
            id: WidgetInternalId::new(),
            value,
            working: value,
            display_mode: TimeDisplayMode::H24,
            show_seconds: false,
            focused_column: TimeColumn::Hour,
            focused: false,
            in_capture: false,
            embedded: false,
            domain: TimeDomain::default(),
            edit_policy: TimeEditPolicy::default(),
            on_commit: None,
            digit_buffer: None,
            cached_width: Cell::new(0.0),
            cached_column_width: Cell::new(0.0),
            cached_separator_width: Cell::new(0.0),
            cached_ampm_width: Cell::new(0.0),
        }
    }

    pub fn with_display_mode(mut self, mode: TimeDisplayMode) -> Self {
        self.display_mode = mode;
        self
    }

    pub fn with_seconds(mut self, show: bool) -> Self {
        self.show_seconds = show;
        self
    }

    pub fn with_embedded(mut self, embedded: bool) -> Self {
        self.embedded = embedded;
        self
    }

    pub fn with_domain(mut self, domain: TimeDomain) -> Self {
        self.domain = domain;
        self
    }

    pub fn with_step_minutes(mut self, step: u8) -> Self {
        self.domain.step_minutes = step.max(1);
        self
    }

    pub fn with_step_seconds(mut self, step: u8) -> Self {
        self.domain.step_seconds = step.max(1);
        self
    }

    pub fn with_edit_policy(mut self, policy: TimeEditPolicy) -> Self {
        self.edit_policy = policy;
        self
    }

    pub fn on_commit(mut self, f: impl FnMut(TimeValue) + 'static) -> Self {
        self.on_commit = Some(Box::new(f));
        self
    }

    pub fn value(&self) -> TimeValue {
        self.value
    }

    pub fn working(&self) -> TimeValue {
        self.working
    }

    pub fn set_value(&mut self, v: TimeValue) {
        self.value = v;
        self.working = v;
    }

    /// Base z-layer offset. When embedded in a popup overlay, uses LAYER_OVERLAY + 2
    /// to render above the backdrop (LAYER_OVERLAY) and popup background (LAYER_OVERLAY + 1).
    fn base_layer(&self) -> u32 {
        if self.embedded {
            LAYER_OVERLAY + 2
        } else {
            LAYER_CONTENT
        }
    }

    /// Returns the ordered list of active columns based on configuration.
    fn active_columns(&self) -> Vec<TimeColumn> {
        let mut cols = vec![TimeColumn::Hour, TimeColumn::Minute];
        if self.show_seconds {
            cols.push(TimeColumn::Second);
        }
        if self.display_mode == TimeDisplayMode::H12 {
            cols.push(TimeColumn::AmPm);
        }
        cols
    }

    fn paint_standalone(&self, cx: &mut PaintContext, rect: RectF) {
        let rect = RectF::new(rect.x, rect.y, self.cached_width.get(), rect.height);
        let t = cx.theme;
        let geom = &t.geometry.time_picker;
        let captured = cx.has_capture();
        let state = if captured {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = t.time_picker_chrome(state);
        let chrome = style.chrome;
        let radius = geom
            .radius
            .min(rect.width * 0.5)
            .min(rect.height * 0.5)
            .max(0.0);
        let edge = chrome.border_width;
        let inner = RectF::new(
            rect.x + edge,
            rect.y + edge,
            (rect.width - edge * 2.0).max(0.0),
            (rect.height - edge * 2.0).max(0.0),
        );
        let inset = edge + t.borders.b1;
        let face = RectF::new(
            rect.x + inset,
            rect.y + inset,
            (rect.width - inset * 2.0).max(0.0),
            (rect.height - inset * 2.0).max(0.0),
        );
        cx.scene
            .push_rect(rect, chrome.border_color, LAYER_CONTENT, None);
        cx.scene
            .push_rect(inner, style.inner_edge, LAYER_CONTENT + 1, None);
        cx.scene.push_rect(
            face,
            chrome.fill.unwrap_or(t.colors.surfaces.content),
            LAYER_CONTENT + 2,
            None,
        );
        if captured {
            if let Some(highlight) = self.highlight_rect(rect, geom).intersection(&face) {
                cx.scene
                    .push_rect(highlight, style.affordance, LAYER_CONTENT + 3, None);
            }
        }
        self.paint_text_content(cx, rect, LAYER_CONTENT + 4);
        if captured {
            cx.scene.push_shadow(
                rect,
                radius,
                ShadowMode::Ring,
                (0.0, 0.0),
                t.focus_ring.spread,
                0.4,
                chrome.focus_color.with_alpha(t.focus_ring.alpha),
                LAYER_FOCUS,
                None,
            );
        }
    }

    fn paint_embedded(&self, cx: &mut PaintContext, rect: RectF) {
        let t = cx.theme;
        let geom = &t.geometry.time_picker;
        let base = self.base_layer();
        let font_size = t.typography.font_size;

        if cx.has_capture() {
            let hr = self.highlight_rect(rect, geom);
            let dummy_run = cx.text_engine.shape(" ", font_size);
            let style = t.time_picker_chrome(ControlChromeState::Captured);
            cx.scene.push_text_with_backgrounds(
                &dummy_run,
                hr.x,
                hr.y,
                Color::TRANSPARENT,
                vec![TextBackgroundSpan {
                    x_offset: 0.0,
                    y_offset: 0.0,
                    width: hr.width,
                    height: hr.height,
                    color: style.affordance,
                }],
                base + 1,
                None,
            );
        }

        self.paint_text_content(cx, rect, base + 2);
    }

    fn highlight_rect(&self, rect: RectF, geom: &crate::core::theme::TimePickerGeometry) -> RectF {
        match self.focused_column {
            TimeColumn::Hour => RectF::new(rect.x, rect.y, geom.column_width, rect.height),
            TimeColumn::Minute => RectF::new(
                rect.x + geom.column_width + geom.separator_width,
                rect.y,
                geom.column_width,
                rect.height,
            ),
            TimeColumn::Second => RectF::new(
                rect.x + geom.column_width * 2.0 + geom.separator_width * 2.0,
                rect.y,
                geom.column_width,
                rect.height,
            ),
            TimeColumn::AmPm => {
                let base_x = if self.show_seconds {
                    rect.x + geom.column_width * 3.0 + geom.separator_width * 2.0
                } else {
                    rect.x + geom.column_width * 2.0 + geom.separator_width
                };
                RectF::new(base_x, rect.y, geom.ampm_width, rect.height)
            }
        }
    }

    fn paint_text_content(&self, cx: &mut PaintContext, rect: RectF, text_z: u32) {
        let t = cx.theme;
        let geom = &t.geometry.time_picker;
        let font_size = t.typography.font_size;
        let metrics = cx.text_engine.font_metrics(font_size);
        let baseline_y = rect.y
            + compute_control_text_baseline_y(
                rect.height,
                metrics.ascent,
                metrics.descent,
                t.typography.baseline_offset,
            );
        let label_y = baseline_y - metrics.ascent;

        // Hour column text
        let hour_text = self.display_hour();
        let hour_run = cx.text_engine.shape(&hour_text, font_size);
        let hour_x = rect.x + (geom.column_width - hour_run.width) * 0.5;
        cx.scene.push_text_with_backgrounds(
            &hour_run,
            hour_x,
            label_y,
            t.colors.typography.primary,
            vec![],
            text_z,
            None,
        );

        // Colon separator (hour:minute)
        let colon_run = cx.text_engine.shape(":", font_size);
        let colon_x = rect.x + geom.column_width + (geom.separator_width - colon_run.width) * 0.5;
        cx.scene.push_text_with_backgrounds(
            &colon_run,
            colon_x,
            label_y,
            t.colors.typography.secondary,
            vec![],
            text_z,
            None,
        );

        // Minute column text
        let minute_text = self.display_minute();
        let minute_run = cx.text_engine.shape(&minute_text, font_size);
        let minute_x = rect.x
            + geom.column_width
            + geom.separator_width
            + (geom.column_width - minute_run.width) * 0.5;
        cx.scene.push_text_with_backgrounds(
            &minute_run,
            minute_x,
            label_y,
            t.colors.typography.primary,
            vec![],
            text_z,
            None,
        );

        // Track x offset after minute column
        let mut after_minute_x = rect.x + geom.column_width * 2.0 + geom.separator_width;

        // Second column (if enabled)
        if self.show_seconds {
            // Second colon separator
            let colon2_x = after_minute_x + (geom.separator_width - colon_run.width) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &colon_run,
                colon2_x,
                label_y,
                t.colors.typography.secondary,
                vec![],
                text_z,
                None,
            );

            // Second column text
            let second_text = self.display_second();
            let second_run = cx.text_engine.shape(&second_text, font_size);
            let second_x = after_minute_x
                + geom.separator_width
                + (geom.column_width - second_run.width) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &second_run,
                second_x,
                label_y,
                t.colors.typography.primary,
                vec![],
                text_z,
                None,
            );

            after_minute_x += geom.separator_width + geom.column_width;
        }

        // AM/PM text (if H12)
        if self.display_mode == TimeDisplayMode::H12 {
            let ampm_text = self.display_ampm();
            let ampm_run = cx.text_engine.shape(ampm_text, font_size);
            let ampm_x = after_minute_x + (geom.ampm_width - ampm_run.width) * 0.5;
            let ampm_color = if (cx.has_focus_cue() || cx.has_capture())
                && self.focused_column == TimeColumn::AmPm
            {
                t.colors.typography.primary
            } else {
                t.colors.typography.secondary
            };
            cx.scene.push_text_with_backgrounds(
                &ampm_run,
                ampm_x,
                label_y,
                ampm_color,
                vec![],
                text_z,
                None,
            );
        }
    }

    fn commit(&mut self) {
        self.value = self.working;
        if let Some(ref mut cb) = self.on_commit {
            cb(self.value);
        }
    }

    /// Advance to next column. Returns false if already at last column.
    fn advance_column(&mut self) -> bool {
        let cols = self.active_columns();
        if let Some(idx) = cols.iter().position(|c| *c == self.focused_column) {
            if idx + 1 < cols.len() {
                self.focused_column = cols[idx + 1];
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Retreat to previous column. Returns false if already at first column.
    fn retreat_column(&mut self) -> bool {
        let cols = self.active_columns();
        if let Some(idx) = cols.iter().position(|c| *c == self.focused_column) {
            if idx > 0 {
                self.focused_column = cols[idx - 1];
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn increment_column(&mut self) {
        match self.focused_column {
            TimeColumn::Hour => {
                if self.domain.wrap {
                    self.working.hour = if self.working.hour >= 23 {
                        0
                    } else {
                        self.working.hour + 1
                    };
                } else {
                    self.working.hour = (self.working.hour + 1).min(self.domain.max.hour);
                }
            }
            TimeColumn::Minute => {
                let step = self.domain.step_minutes.max(1);
                let next = self.working.minute + step;
                if self.domain.wrap {
                    self.working.minute = if next >= 60 { 0 } else { next };
                } else {
                    self.working.minute = next.min(59);
                }
            }
            TimeColumn::Second => {
                let step = self.domain.step_seconds.max(1);
                let s = self.working.second.unwrap_or(0);
                let next = s + step;
                if self.domain.wrap {
                    self.working.second = Some(if next >= 60 { 0 } else { next });
                } else {
                    self.working.second = Some(next.min(59));
                }
            }
            TimeColumn::AmPm => {
                self.working.hour = (self.working.hour + 12) % 24;
            }
        }
    }

    fn decrement_column(&mut self) {
        match self.focused_column {
            TimeColumn::Hour => {
                if self.domain.wrap {
                    self.working.hour = if self.working.hour == 0 {
                        23
                    } else {
                        self.working.hour - 1
                    };
                } else {
                    self.working.hour = if self.working.hour > self.domain.min.hour {
                        self.working.hour - 1
                    } else {
                        self.working.hour
                    };
                }
            }
            TimeColumn::Minute => {
                let step = self.domain.step_minutes.max(1);
                if self.domain.wrap {
                    self.working.minute = if self.working.minute < step {
                        60 - step
                    } else {
                        self.working.minute - step
                    };
                } else {
                    self.working.minute = self.working.minute.saturating_sub(step);
                }
            }
            TimeColumn::Second => {
                let step = self.domain.step_seconds.max(1);
                let s = self.working.second.unwrap_or(0);
                if self.domain.wrap {
                    self.working.second = Some(if s < step { 60 - step } else { s - step });
                } else {
                    self.working.second = Some(s.saturating_sub(step));
                }
            }
            TimeColumn::AmPm => {
                self.working.hour = (self.working.hour + 12) % 24;
            }
        }
    }

    fn apply_digit(&mut self, digit: u8) {
        if let Some(first) = self.digit_buffer {
            let value = first * 10 + digit;
            let valid = match self.focused_column {
                TimeColumn::Hour => {
                    if self.display_mode == TimeDisplayMode::H12 {
                        value >= 1 && value <= 12
                    } else {
                        value < 24
                    }
                }
                TimeColumn::Minute | TimeColumn::Second => value < 60,
                TimeColumn::AmPm => false,
            };
            if valid {
                match self.focused_column {
                    TimeColumn::Hour => {
                        if self.display_mode == TimeDisplayMode::H12 {
                            let is_pm = self.working.hour >= 12;
                            let mut h24 = value;
                            if value == 12 {
                                h24 = 0;
                            }
                            if is_pm {
                                h24 += 12;
                            }
                            self.working.hour = h24.min(23);
                        } else {
                            self.working.hour = value;
                        }
                    }
                    TimeColumn::Minute => self.working.minute = value,
                    TimeColumn::Second => {
                        self.working.second = Some(value);
                    }
                    TimeColumn::AmPm => {}
                }
                self.digit_buffer = None;
                // Auto-advance to next column (INV-273-D3)
                self.advance_column();
            } else {
                // Second digit invalid for this column.
                // Flush first digit as single value, advance, then apply second digit to new column.
                let first = self.digit_buffer.take().unwrap();

                // Apply first digit to current column
                match self.focused_column {
                    TimeColumn::Hour => {
                        if self.display_mode == TimeDisplayMode::H12 {
                            let is_pm = self.working.hour >= 12;
                            let mut h24 = first;
                            if is_pm {
                                h24 += 12;
                            }
                            self.working.hour = h24.min(23);
                        } else {
                            self.working.hour = first;
                        }
                    }
                    TimeColumn::Minute => self.working.minute = first,
                    TimeColumn::Second => self.working.second = Some(first),
                    TimeColumn::AmPm => {}
                }

                // Advance to next column
                self.advance_column();

                // Apply second digit to the new column (recursive single-digit apply)
                self.apply_digit(digit);
            }
        } else {
            // First digit — check if it can start a valid two-digit value
            if self.focused_column == TimeColumn::AmPm {
                return; // AmPm doesn't accept digits
            }

            let can_start_two_digit = match self.focused_column {
                TimeColumn::Hour => {
                    if self.display_mode == TimeDisplayMode::H12 {
                        digit <= 1 // only 1x can be two-digit (10, 11, 12)
                    } else {
                        digit <= 2 // 0x, 1x, 2x can be two-digit (00-23)
                    }
                }
                TimeColumn::Minute | TimeColumn::Second => {
                    digit <= 5 // 0x-5x can be two-digit (00-59)
                }
                TimeColumn::AmPm => false,
            };

            if can_start_two_digit {
                // Wait for second digit
                self.digit_buffer = Some(digit);
            } else {
                // Can't start a valid pair — apply immediately as the value and auto-advance
                match self.focused_column {
                    TimeColumn::Hour => {
                        if self.display_mode == TimeDisplayMode::H12 {
                            let is_pm = self.working.hour >= 12;
                            let mut h24 = digit;
                            if is_pm {
                                h24 += 12;
                            }
                            self.working.hour = h24.min(23);
                        } else {
                            self.working.hour = digit;
                        }
                    }
                    TimeColumn::Minute => self.working.minute = digit,
                    TimeColumn::Second => {
                        self.working.second = Some(digit);
                    }
                    TimeColumn::AmPm => {}
                }
                self.advance_column();
            }
        }
    }

    fn flush_digit_buffer(&mut self) {
        if let Some(digit) = self.digit_buffer.take() {
            match self.focused_column {
                TimeColumn::Hour => {
                    if self.display_mode == TimeDisplayMode::H12 {
                        let is_pm = self.working.hour >= 12;
                        let mut h24 = digit;
                        if is_pm {
                            h24 += 12;
                        }
                        self.working.hour = h24.min(23);
                    } else {
                        self.working.hour = digit;
                    }
                }
                TimeColumn::Minute => self.working.minute = digit,
                TimeColumn::Second => {
                    self.working.second = Some(digit);
                }
                TimeColumn::AmPm => {}
            }
            // NO auto-advance on single digit (INV-273-D5)
        }
    }

    fn display_hour(&self) -> String {
        match self.display_mode {
            TimeDisplayMode::H24 => format!("{:02}", self.working.hour),
            TimeDisplayMode::H12 => {
                let h = if self.working.hour == 0 {
                    12
                } else if self.working.hour > 12 {
                    self.working.hour - 12
                } else {
                    self.working.hour
                };
                format!("{:2}", h)
            }
        }
    }

    fn display_minute(&self) -> String {
        format!("{:02}", self.working.minute)
    }

    fn display_second(&self) -> String {
        format!("{:02}", self.working.second.unwrap_or(0))
    }

    fn display_ampm(&self) -> &'static str {
        if self.working.hour < 12 { "AM" } else { "PM" }
    }

    /// Determine which column a local x coordinate falls in, using cached geometry.
    fn column_at_x(&self, local_x: f32) -> Option<TimeColumn> {
        let cw = self.cached_column_width.get();
        let sw = self.cached_separator_width.get();
        let aw = self.cached_ampm_width.get();

        let mut x = 0.0;
        // Hour
        if local_x < x + cw {
            return Some(TimeColumn::Hour);
        }
        x += cw + sw;
        // Minute
        if local_x < x + cw {
            return Some(TimeColumn::Minute);
        }
        x += cw;
        // Second (if visible)
        if self.show_seconds {
            x += sw;
            if local_x < x + cw {
                return Some(TimeColumn::Second);
            }
            x += cw;
        }
        // AmPm (if H12)
        if self.display_mode == TimeDisplayMode::H12 {
            if local_x < x + aw {
                return Some(TimeColumn::AmPm);
            }
        }
        None
    }
}

impl WidgetDowncast for TimePicker {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TimePicker {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.time_picker;
        let mut w = geom.column_width * 2.0 + geom.separator_width;
        if self.show_seconds {
            w += geom.separator_width + geom.column_width;
        }
        if self.display_mode == TimeDisplayMode::H12 {
            w += geom.ampm_width;
        }
        let h = geom.column_height;
        self.cached_width.set(w);
        self.cached_column_width.set(geom.column_width);
        self.cached_separator_width.set(geom.separator_width);
        self.cached_ampm_width.set(geom.ampm_width);
        let width = w.min(constraints.max_width);
        let height = h.max(constraints.min_height).min(constraints.max_height);
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if self.embedded {
            self.paint_embedded(cx, rect);
        } else {
            self.paint_standalone(cx, rect);
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                let (lx, _ly) = pe.local_pos;

                match pe.kind {
                    PointerEventKind::Down => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if self.edit_policy == TimeEditPolicy::ReadOnly {
                            return false;
                        }
                        // Click enters capture AND sets focused column
                        cx.request_keyboard_capture();
                        if let Some(col) = self.column_at_x(lx) {
                            self.flush_digit_buffer();
                            self.focused_column = col;
                        }
                        true
                    }
                    PointerEventKind::Scroll { delta_y, .. } => {
                        // Scroll works in BOTH modes (pointer action, not keyboard)
                        if self.edit_policy == TimeEditPolicy::ReadOnly {
                            return false;
                        }
                        if let Some(col) = self.column_at_x(lx) {
                            self.flush_digit_buffer();
                            let prev_col = self.focused_column;
                            self.focused_column = col;
                            if delta_y < 0.0 {
                                self.increment_column();
                            } else if delta_y > 0.0 {
                                self.decrement_column();
                            }
                            self.focused_column = prev_col;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down => {
                // Navigation mode: only Enter enters capture
                if !self.in_capture {
                    match ke.key_code {
                        KeyCode::Enter => {
                            if self.edit_policy != TimeEditPolicy::ReadOnly {
                                cx.request_keyboard_capture();
                                true
                            } else {
                                false
                            }
                        }
                        _ => false, // all other keys: do nothing in navigation mode
                    }
                } else {
                    // Capture mode: full editing
                    match ke.key_code {
                        KeyCode::Left => {
                            self.flush_digit_buffer();
                            if !self.retreat_column() {
                                let cols = self.active_columns();
                                self.focused_column = *cols.last().unwrap();
                            }
                            true
                        }
                        KeyCode::Right => {
                            self.flush_digit_buffer();
                            if !self.advance_column() {
                                let cols = self.active_columns();
                                self.focused_column = cols[0];
                            }
                            true
                        }
                        KeyCode::Tab => {
                            self.flush_digit_buffer();
                            if ke.modifiers.shift {
                                if !self.retreat_column() {
                                    cx.release_keyboard_capture();
                                    return false; // propagate Tab
                                }
                            } else {
                                if !self.advance_column() {
                                    cx.release_keyboard_capture();
                                    return false; // propagate Tab
                                }
                            }
                            true
                        }
                        KeyCode::Up => {
                            self.flush_digit_buffer();
                            self.increment_column();
                            true
                        }
                        KeyCode::Down => {
                            self.flush_digit_buffer();
                            self.decrement_column();
                            true
                        }
                        KeyCode::Enter | KeyCode::Space => {
                            // Commit + exit capture
                            self.flush_digit_buffer();
                            self.commit();
                            cx.release_keyboard_capture();
                            true
                        }
                        KeyCode::Escape => {
                            // Revert + exit capture
                            self.working = self.value;
                            self.digit_buffer = None;
                            cx.release_keyboard_capture();
                            true
                        }
                        _ => false,
                    }
                }
            }
            Event::Text(te) => {
                // Text input ONLY in capture mode
                if !self.in_capture {
                    return false;
                }
                if self.edit_policy == TimeEditPolicy::AdjustOnly
                    || self.edit_policy == TimeEditPolicy::ReadOnly
                {
                    return false;
                }
                if te.modifiers.ctrl || te.modifiers.alt || te.modifiers.meta {
                    return false;
                }
                if let Some(ch) = te.text.chars().next() {
                    match ch {
                        // hjkl vim keys (capture mode)
                        'h' => {
                            self.flush_digit_buffer();
                            if !self.retreat_column() {
                                let cols = self.active_columns();
                                self.focused_column = *cols.last().unwrap();
                            }
                            return true;
                        }
                        'l' => {
                            self.flush_digit_buffer();
                            if !self.advance_column() {
                                let cols = self.active_columns();
                                self.focused_column = cols[0];
                            }
                            return true;
                        }
                        'k' => {
                            self.flush_digit_buffer();
                            self.increment_column();
                            return true;
                        }
                        'j' => {
                            self.flush_digit_buffer();
                            self.decrement_column();
                            return true;
                        }
                        // AM/PM toggle
                        'a' | 'A' => {
                            if self.display_mode == TimeDisplayMode::H12 {
                                self.flush_digit_buffer();
                                if self.working.hour >= 12 {
                                    self.working.hour -= 12;
                                }
                                return true;
                            }
                        }
                        'p' | 'P' => {
                            if self.display_mode == TimeDisplayMode::H12 {
                                self.flush_digit_buffer();
                                if self.working.hour < 12 {
                                    self.working.hour += 12;
                                }
                                return true;
                            }
                        }
                        _ => {}
                    }
                    // Digit entry
                    if let Some(digit) = ch.to_digit(10) {
                        self.apply_digit(digit as u8);
                        return true;
                    } else {
                        self.flush_digit_buffer();
                    }
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
        self.edit_policy != TimeEditPolicy::ReadOnly
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        self.in_capture = true;
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        self.in_capture = false;
        self.digit_buffer = None;
        // Revert working to committed on capture leave (INV-273-CAP3)
        self.working = self.value;
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
        self.focused_column = TimeColumn::Hour;
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        self.in_capture = false;
        self.digit_buffer = None;
        self.working = self.value;
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "TimePicker"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{KeyEvent, KeyboardAuthorityMode, Modifiers, TextInputEvent};
    use crate::utils::id::TreeWidgetId;

    fn event_context() -> EventContext<'static> {
        EventContext::new(
            TreeWidgetId(1),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            KeyboardAuthorityMode::Capture,
        )
    }

    fn key_down(key_code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            key_code,

            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            is_down: true,
            is_repeat: false,
            modifiers: Modifiers::default(),
            text: None,
        })
    }

    fn text_input(text: &str) -> Event {
        Event::Text(TextInputEvent {
            text: text.to_string(),
            modifiers: Modifiers::default(),
        })
    }

    #[test]
    fn digit_keydown_between_text_events_does_not_flush_buffer() {
        let mut picker = TimePicker::new(TimeValue::new(0, 0, None));
        picker.in_capture = true;

        let mut cx = event_context();
        assert!(picker.handle_event(&mut cx, &text_input("1"), EventPhase::Target));

        let mut cx = event_context();
        assert!(!picker.handle_event(
            &mut cx,
            &key_down(KeyCode::Character('2')),
            EventPhase::Target
        ));

        let mut cx = event_context();
        assert!(picker.handle_event(&mut cx, &text_input("2"), EventPhase::Target));

        assert_eq!(picker.working.hour, 12);
        assert_eq!(picker.focused_column, TimeColumn::Minute);
        assert_eq!(picker.digit_buffer, None);
    }

    #[test]
    fn time_picker_theme_overrides_remain_resolved_inputs() {
        use crate::core::theme::{DensityMode, Theme};

        for dark in [false, true] {
            for density in [
                DensityMode::Compact,
                DensityMode::Regular,
                DensityMode::Intermediate30,
                DensityMode::Comfortable,
            ] {
                let mut theme = if dark {
                    Theme::dark_with_density(density)
                } else {
                    Theme::light_with_density(density)
                };
                assert_eq!(
                    theme
                        .time_picker_chrome(ControlChromeState::Captured)
                        .chrome
                        .focus_color,
                    theme
                        .input_chrome(ControlChromeState::Captured)
                        .chrome
                        .focus_color,
                );
                let other_default = if dark {
                    Theme::light_with_density(density)
                } else {
                    Theme::dark_with_density(density)
                };
                theme.focus_ring.capture_color = other_default.focus_ring.capture_color;
                assert_eq!(
                    theme
                        .time_picker_chrome(ControlChromeState::Captured)
                        .chrome
                        .focus_color,
                    theme.focus_ring.capture_color,
                );
                let geometry = theme.geometry.time_picker;
                theme.colors.surfaces.content = theme.colors.semantic.info;
                theme.colors.surfaces.level2 = theme.colors.semantic.warning;
                theme.colors.borders.default = theme.colors.semantic.error;
                theme.colors.borders.focus = theme.colors.semantic.success;
                theme.focus_ring.capture_color = theme.colors.icons.folder;

                for state in [
                    ControlChromeState::Idle,
                    ControlChromeState::Focused,
                    ControlChromeState::Captured,
                ] {
                    let style = theme.time_picker_chrome(state);
                    assert_eq!(style.chrome.fill, Some(theme.colors.surfaces.content));
                    assert_eq!(style.affordance, theme.colors.surfaces.level2);
                    let expected_border = if state == ControlChromeState::Idle {
                        theme.colors.borders.default
                    } else {
                        theme.colors.borders.focus
                    };
                    assert_eq!(style.chrome.border_color, expected_border);
                    if state == ControlChromeState::Captured {
                        assert_eq!(style.chrome.focus_color, theme.focus_ring.capture_color);
                    }
                }
                assert_eq!(
                    theme.geometry.time_picker.column_width,
                    geometry.column_width
                );
                assert_eq!(
                    theme.geometry.time_picker.column_height,
                    geometry.column_height
                );
                assert_eq!(
                    theme.geometry.time_picker.separator_width,
                    geometry.separator_width
                );
                assert_eq!(theme.geometry.time_picker.radius, geometry.radius);
            }
        }
    }
}
