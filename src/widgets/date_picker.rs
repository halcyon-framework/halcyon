// SPEC-HAL-238: DatePicker Widget — Inline calendar picker.
// Structural contract and invariants defined in SPEC-HAL-238.
// Popup mode deferred — inline only.

use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_FOCUS, RoundedCorners};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::date_value::{self, DateValue};
use crate::model::form_binder::{BindingKey, ChangeWriter};
use crate::utils::id::WidgetInternalId;

// ============================================================================
// Geometry Constants
// ============================================================================

pub const DATEPICKER_W: f32 = 252.0;
pub const DATEPICKER_H: f32 = 236.0;
const HEADER_H: f32 = 32.0;
const CELL_W: f32 = 36.0;
const CELL_H: f32 = 30.0;
const CELL_RADIUS: f32 = 4.0;
const ARROW_W: f32 = 28.0;
const DOW_ROW_H: f32 = 24.0;

// ============================================================================
// DatePickerState
// ============================================================================

pub struct DatePickerState {
    selected: Option<DateValue>,
    view_year: i32,
    view_month: u8,
    kbd_cursor: Option<DateValue>,
    min_date: Option<DateValue>,
    max_date: Option<DateValue>,
    dirty: bool,
}

impl DatePickerState {
    pub fn new(
        initial: Option<DateValue>,
        min_date: Option<DateValue>,
        max_date: Option<DateValue>,
    ) -> Self {
        let (vy, vm) = if let Some(d) = initial {
            (d.year, d.month)
        } else {
            // Default view: 2026-03 (current date context)
            (2026, 3)
        };
        Self {
            selected: initial,
            view_year: vy,
            view_month: vm,
            kbd_cursor: initial,
            min_date,
            max_date,
            dirty: false,
        }
    }

    pub fn selected(&self) -> Option<DateValue> {
        self.selected
    }

    pub fn view_year(&self) -> i32 {
        self.view_year
    }

    pub fn view_month(&self) -> u8 {
        self.view_month
    }

    pub fn kbd_cursor(&self) -> Option<DateValue> {
        self.kbd_cursor
    }

    /// Navigate view to previous month. No effect on selected or dirty.
    pub fn prev_month(&mut self) {
        if self.view_month == 1 {
            self.view_month = 12;
            self.view_year -= 1;
        } else {
            self.view_month -= 1;
        }
        self.clamp_view_to_constraints();
    }

    /// Navigate view to next month. No effect on selected or dirty.
    pub fn next_month(&mut self) {
        if self.view_month == 12 {
            self.view_month = 1;
            self.view_year += 1;
        } else {
            self.view_month += 1;
        }
        self.clamp_view_to_constraints();
    }

    /// Navigate view to previous year.
    pub fn prev_year(&mut self) {
        if self.view_year > 1 {
            self.view_year -= 1;
        }
        self.clamp_view_to_constraints();
    }

    /// Navigate view to next year.
    pub fn next_year(&mut self) {
        if self.view_year < 9999 {
            self.view_year += 1;
        }
        self.clamp_view_to_constraints();
    }

    /// Jump view to the given date and place kbd cursor on it.
    pub fn jump_to_date(&mut self, date: DateValue) {
        self.view_year = date.year;
        self.view_month = date.month;
        self.kbd_cursor = Some(date);
    }

    /// Clamp view to min_date/max_date constraints so the user cannot navigate
    /// to a month where every day is outside the allowed range.
    pub fn clamp_view_to_constraints(&mut self) {
        if let Some(min) = self.min_date {
            if self.view_year < min.year
                || (self.view_year == min.year && self.view_month < min.month)
            {
                self.view_year = min.year;
                self.view_month = min.month;
            }
        }
        if let Some(max) = self.max_date {
            if self.view_year > max.year
                || (self.view_year == max.year && self.view_month > max.month)
            {
                self.view_year = max.year;
                self.view_month = max.month;
            }
        }
    }

    /// Returns true if the given date is within constraints.
    pub fn is_enabled(&self, date: DateValue) -> bool {
        if let Some(min) = self.min_date {
            if date < min {
                return false;
            }
        }
        if let Some(max) = self.max_date {
            if date > max {
                return false;
            }
        }
        true
    }

    /// Commit a day in the current view month.
    pub fn commit_day(&mut self, day: u8) {
        if let Some(date) = DateValue::new(self.view_year, self.view_month, day) {
            if !self.is_enabled(date) {
                return;
            }
            if self.selected == Some(date) {
                return; // Idempotent — no dirty
            }
            self.selected = Some(date);
            self.kbd_cursor = Some(date);
            self.dirty = true;
        }
    }

    /// Commit the current keyboard cursor position.
    pub fn commit_kbd_cursor(&mut self) {
        if let Some(date) = self.kbd_cursor {
            if !self.is_enabled(date) {
                return;
            }
            if self.selected == Some(date) {
                return;
            }
            self.selected = Some(date);
            self.dirty = true;
        }
    }

    /// Move keyboard cursor by delta_days. Skips disabled dates.
    pub fn move_kbd_cursor(&mut self, delta_days: i32) {
        let start = self.kbd_cursor.unwrap_or_else(|| {
            // If no cursor, start from view month day 1
            DateValue::new(self.view_year, self.view_month, 1).unwrap()
        });

        let forward = delta_days > 0;
        let steps = delta_days.unsigned_abs();
        let mut current = start;

        // Move by the requested number of steps
        for _ in 0..steps {
            if forward {
                if let Some(next) = current.succ() {
                    current = next;
                } else {
                    return; // Hit date boundary
                }
            } else if let Some(prev) = current.pred() {
                current = prev;
            } else {
                return; // Hit date boundary
            }
        }

        // INV-238-KBD-5: Skip disabled dates in movement direction
        let mut skip_count = 0;
        while !self.is_enabled(current) && skip_count < 60 {
            if forward {
                if let Some(next) = current.succ() {
                    current = next;
                } else {
                    return;
                }
            } else if let Some(prev) = current.pred() {
                current = prev;
            } else {
                return;
            }
            skip_count += 1;
        }

        if skip_count >= 60 || !self.is_enabled(current) {
            return; // No enabled date found within range
        }

        self.kbd_cursor = Some(current);
        // Update view to show the cursor
        self.view_year = current.year;
        self.view_month = current.month;
    }

    /// If dirty, clear dirty and return Some(selected). Else None.
    pub fn take_changed(&mut self) -> Option<Option<DateValue>> {
        if self.dirty {
            self.dirty = false;
            Some(self.selected)
        } else {
            None
        }
    }

    /// Set the selected date directly (used for revert on cancel).
    pub fn set_selected(&mut self, date: Option<DateValue>) {
        self.selected = date;
        if let Some(d) = date {
            self.view_year = d.year;
            self.view_month = d.month;
        }
    }
}

// ============================================================================
// Grid Helpers
// ============================================================================

/// Tomohiko Sakamoto's algorithm for day-of-week.
/// Returns 0=Sunday, 1=Monday, ..., 6=Saturday.
fn day_of_week(year: i32, month: u8, day: u8) -> u8 {
    static T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = year;
    if month < 3 {
        y -= 1;
    }
    let dow = (y + y / 4 - y / 100 + y / 400 + T[(month - 1) as usize] + day as i32) % 7;
    // Ensure non-negative
    ((dow + 7) % 7) as u8
}

/// Returns the weekday offset (0..6) of day 1 in the grid for the given first_dow.
fn first_weekday_offset(year: i32, month: u8, first_dow: u8) -> u8 {
    let dow_of_first = day_of_week(year, month, 1); // 0=Sun
    ((dow_of_first + 7 - first_dow) % 7) as u8
}

/// Month names for header display.
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Day-of-week labels starting from a given first_dow.
fn dow_labels(first_dow: u8) -> [&'static str; 7] {
    const ALL: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
    let mut out = [""; 7];
    for i in 0..7 {
        out[i] = ALL[((first_dow as usize) + i) % 7];
    }
    out
}

// ============================================================================
// Date Marks
// ============================================================================

/// Visual annotation for a specific date cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateMarkKind {
    /// Small dot below the day number (default accent color).
    Dot,
    /// Subtle background tint (uses accent at low alpha).
    Tint,
}

#[derive(Debug, Clone)]
pub struct DateMark {
    pub date: DateValue,
    pub kind: DateMarkKind,
}

// ============================================================================
// DatePolicy — predicate-based date annotation and disable policy
// ============================================================================

/// Visual highlight produced by a DatePolicy rule.
#[derive(Debug, Clone, Copy)]
pub enum PolicyHighlight {
    /// Small dot below the day number (same rendering as DateMarkKind::Dot).
    Dot,
    /// Subtle accent tint behind the cell fill (same as DateMarkKind::Tint).
    Tint,
    /// Caller-specified background tint color (replaces accent color for the tint fill).
    Custom(Color),
}

/// Predicate-based date annotation and disable policy.
///
/// Both closures are evaluated per visible cell at paint time.
/// Neither closure may mutate external state.
///
/// `marked_dates: Vec<DateMark>` remains fully functional and is evaluated
/// first; policy results are merged additively.
pub struct DatePolicy {
    /// Returns a highlight annotation for the given date, or None.
    pub highlight: Option<Box<dyn Fn(DateValue) -> Option<PolicyHighlight> + Send + Sync>>,
    /// Returns true if the given date should be treated as disabled/unselectable.
    /// OR'd with the existing min_date/max_date constraint from DatePickerState.
    pub disabled: Option<Box<dyn Fn(DateValue) -> bool + Send + Sync>>,
}

impl DatePolicy {
    pub fn new() -> Self {
        Self {
            highlight: None,
            disabled: None,
        }
    }

    pub fn with_highlight<F>(mut self, f: F) -> Self
    where
        F: Fn(DateValue) -> Option<PolicyHighlight> + Send + Sync + 'static,
    {
        self.highlight = Some(Box::new(f));
        self
    }

    pub fn with_disabled<F>(mut self, f: F) -> Self
    where
        F: Fn(DateValue) -> bool + Send + Sync + 'static,
    {
        self.disabled = Some(Box::new(f));
        self
    }
}

// ============================================================================
// DatePickerWidget
// ============================================================================

pub struct DatePickerWidget {
    id: WidgetInternalId,
    state: DatePickerState,
    first_dow: u8, // 0=Sunday, 1=Monday
    today: Option<DateValue>,
    binding: Option<(BindingKey, ChangeWriter)>,
    // Cached from layout
    cell_rects: [[RectF; 7]; 6],
    header_rect: RectF,
    prev_month_rect: RectF,
    next_month_rect: RectF,
    prev_year_rect: RectF,
    next_year_rect: RectF,
    dow_row_rect: RectF,
    hovered_cell: Option<(usize, usize)>,
    hovered_arrow: Option<u8>, // 0=prev_year, 1=prev_month, 2=next_month, 3=next_year
    layout_bounds: Size,
    focused: bool,
    in_capture: bool,
    hover_t: Cell<f32>,
    hover_anim: Cell<Option<TransitionHandle>>,
    focus_t: Cell<f32>,
    focus_anim: Cell<Option<TransitionHandle>>,
    marked_dates: Vec<DateMark>,
    digit_buffer: Option<(u8, u64)>, // (first_digit: 0–9, typed_at_ms: frame_time_ms)
    year_jump_buffer: Option<String>,
    policy: Option<DatePolicy>,
    last_cursor: Option<DateValue>,
    embedded: bool,
    paint_text_layer: u32,
}

impl DatePickerWidget {
    pub fn new(
        initial: Option<DateValue>,
        min_date: Option<DateValue>,
        max_date: Option<DateValue>,
        first_dow: u8,
    ) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: DatePickerState::new(initial, min_date, max_date),
            first_dow,
            today: None,
            binding: None,
            cell_rects: [[RectF::zero(); 7]; 6],
            header_rect: RectF::zero(),
            prev_month_rect: RectF::zero(),
            next_month_rect: RectF::zero(),
            prev_year_rect: RectF::zero(),
            next_year_rect: RectF::zero(),
            dow_row_rect: RectF::zero(),
            hovered_cell: None,
            hovered_arrow: None,
            layout_bounds: Size::zero(),
            focused: false,
            in_capture: false,
            hover_t: Cell::new(0.0),
            hover_anim: Cell::new(None),
            focus_t: Cell::new(0.0),
            focus_anim: Cell::new(None),
            marked_dates: Vec::new(),
            digit_buffer: None,
            year_jump_buffer: None,
            policy: None,
            last_cursor: None,
            embedded: false,
            paint_text_layer: 0,
        }
    }

    pub fn with_today(mut self, today: DateValue) -> Self {
        self.today = Some(today);
        self
    }

    /// SPEC-HAL-207: Configure binding for date emission.
    pub fn set_binding(&mut self, key: &str, writer: ChangeWriter) {
        let binding_key = writer.intern(key);
        self.binding = Some((binding_key, writer));
    }

    pub fn with_binding(mut self, key: &str, writer: ChangeWriter) -> Self {
        self.set_binding(key, writer);
        self
    }

    pub fn with_marks(mut self, marks: Vec<DateMark>) -> Self {
        self.marked_dates = marks;
        self
    }

    pub fn with_policy(mut self, policy: DatePolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn with_embedded(mut self, embedded: bool) -> Self {
        self.embedded = embedded;
        self.paint_text_layer = if embedded {
            crate::core::paint::LAYER_OVERLAY + 2
        } else {
            0
        };
        self
    }

    pub fn is_year_jump_active(&self) -> bool {
        self.year_jump_buffer.is_some()
    }

    fn set_cursor(&mut self, date: DateValue) {
        self.state.kbd_cursor = Some(date);
        self.last_cursor = Some(date);
    }

    fn restore_cursor_if_needed(&mut self) {
        if self.state.kbd_cursor.is_none() {
            let fallback = self.state.selected.or(self.last_cursor);
            if let Some(date) = fallback {
                self.state.kbd_cursor = Some(date);
                self.state.view_year = date.year;
                self.state.view_month = date.month;
            }
        }
    }

    fn policy_allows(&self, date: DateValue) -> bool {
        match self.policy.as_ref() {
            Some(pol) => pol.disabled.as_ref().map_or(true, |f| !f(date)),
            None => true,
        }
    }

    pub fn state(&self) -> &DatePickerState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut DatePickerState {
        &mut self.state
    }

    /// Emit current selection via binding.
    /// Commit the current kbd_cursor as the selection (toggle off if already selected).
    fn commit_cursor_selection(&mut self) {
        if let Some(cursor) = self.state.kbd_cursor {
            if !self.policy_allows(cursor) {
                return;
            }
            if self.state.is_enabled(cursor) {
                if self.state.selected == Some(cursor) {
                    self.state.selected = None;
                    self.state.dirty = true;
                } else {
                    self.state.commit_kbd_cursor();
                }
                if self.state.dirty {
                    self.emit_binding();
                }
            }
        }
    }

    /// SPEC-HAL-176: Commit the keyboard cursor's date as the selection
    /// unconditionally — no toggle, never deselects on repeat.
    ///
    /// Behaviorally identical to `commit_cursor_selection` EXCEPT for
    /// the toggle: when `state.selected == Some(cursor)`, this method
    /// is a no-op (idempotent), whereas `commit_cursor_selection`
    /// deselects.
    ///
    /// Used by the `i` Enter-alias key path (in both standalone
    /// DatePickerWidget and DatePickerField-embedded modes), and by
    /// DatePickerField's `i` handling. Respects `policy_allows` and
    /// `state.is_enabled` constraints — disabled and policy-excluded
    /// dates are not committed.
    ///
    /// Emits the binding via `emit_binding` if the commit actually
    /// changed the selection (dirty flag set).
    pub fn force_commit_cursor(&mut self) {
        if let Some(cursor) = self.state.kbd_cursor {
            if !self.policy_allows(cursor) {
                return;
            }
            if self.state.is_enabled(cursor) {
                self.state.commit_kbd_cursor();
                if self.state.dirty {
                    self.emit_binding();
                }
            }
        }
    }

    fn emit_binding(&self) {
        if let Some((key, writer)) = &self.binding {
            let text = self
                .state
                .selected
                .map(|d| d.to_iso8601())
                .unwrap_or_default();
            writer.set_text(*key, text);
        }
    }

    /// Resolve which day number a cell (row, col) corresponds to.
    /// Returns None if the cell is out-of-month.
    fn cell_day(&self, row: usize, col: usize) -> Option<u8> {
        let offset =
            first_weekday_offset(self.state.view_year, self.state.view_month, self.first_dow);
        let cell_index = row * 7 + col;
        let day_num = cell_index as i32 - offset as i32 + 1;
        let max_days = date_value::days_in_month(self.state.view_year, self.state.view_month);
        if day_num >= 1 && day_num <= max_days as i32 {
            Some(day_num as u8)
        } else {
            None
        }
    }

    /// Hit-test pointer position against arrow rects.
    fn hit_test_arrow(&self, lx: f32, ly: f32) -> Option<u8> {
        if self.prev_year_rect.contains_point(lx, ly) {
            Some(0)
        } else if self.prev_month_rect.contains_point(lx, ly) {
            Some(1)
        } else if self.next_month_rect.contains_point(lx, ly) {
            Some(2)
        } else if self.next_year_rect.contains_point(lx, ly) {
            Some(3)
        } else {
            None
        }
    }

    /// Hit-test pointer position against day cells.
    fn hit_test_cell(&self, lx: f32, ly: f32) -> Option<(usize, usize)> {
        for row in 0..6 {
            for col in 0..7 {
                let r = self.cell_rects[row][col];
                if lx >= r.x && lx < r.x + r.width && ly >= r.y && ly < r.y + r.height {
                    return Some((row, col));
                }
            }
        }
        None
    }

    fn clamp_cursor_to_view(&mut self) {
        if let Some(cursor) = self.state.kbd_cursor {
            let max_day = crate::model::date_value::days_in_month(
                self.state.view_year,
                self.state.view_month,
            );
            let clamped = cursor.day.min(max_day);
            self.state.kbd_cursor =
                DateValue::new(self.state.view_year, self.state.view_month, clamped);
        }
    }

    /// Flush any pending single digit in the buffer. Returns true if a cursor move occurred.
    fn flush_digit_buffer(&mut self) {
        if let Some((d, _)) = self.digit_buffer.take() {
            if d >= 1 {
                let max = crate::model::date_value::days_in_month(
                    self.state.view_year,
                    self.state.view_month,
                );
                if d <= max {
                    if let Some(date) =
                        DateValue::new(self.state.view_year, self.state.view_month, d)
                    {
                        self.set_cursor(date);
                        self.state.view_year = date.year;
                        self.state.view_month = date.month;
                    }
                }
            }
        }
    }
}

impl WidgetDowncast for DatePickerWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DatePickerWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let w = DATEPICKER_W;
        let h = DATEPICKER_H;
        self.layout_bounds = Size::new(w, h);

        // Header rect: full width, HEADER_H tall
        self.header_rect = RectF::new(0.0, 0.0, w, HEADER_H);

        // Arrow rects within header
        self.prev_year_rect = RectF::new(0.0, 0.0, ARROW_W, HEADER_H);
        self.prev_month_rect = RectF::new(ARROW_W, 0.0, ARROW_W, HEADER_H);
        self.next_month_rect = RectF::new(w - ARROW_W * 2.0, 0.0, ARROW_W, HEADER_H);
        self.next_year_rect = RectF::new(w - ARROW_W, 0.0, ARROW_W, HEADER_H);

        // Day-of-week header row
        let dow_y = HEADER_H;
        self.dow_row_rect = RectF::new(0.0, dow_y, w, DOW_ROW_H);

        // 6 rows x 7 cols of day cells
        let grid_y = HEADER_H + DOW_ROW_H;
        for row in 0..6 {
            for col in 0..7 {
                self.cell_rects[row][col] = RectF::new(
                    col as f32 * CELL_W,
                    grid_y + row as f32 * CELL_H,
                    CELL_W,
                    CELL_H,
                );
            }
        }

        LayoutResult::fit(self.layout_bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let key = SceneNodeKey::from(self.id.0);
        cx.with_scene_key(key, |cx| {
            // Consume animations
            {
                let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
                if let Some(handle) = self.hover_anim.get() {
                    if let Some(value) = cx.get_animation::<f32>(handle) {
                        self.hover_t.set(value);
                    } else {
                        self.hover_anim.set(None);
                    }
                }
                if let Some(handle) = self.focus_anim.get() {
                    if let Some(value) = cx.get_animation::<f32>(handle) {
                        self.focus_t.set(value);
                    } else {
                        self.focus_anim.set(None);
                    }
                }
            }

            let theme = cx.theme;
            let font_size = theme.typography.font_size;

            // Background gradient — skip when embedded (paint_overlay provides the background blocker)
            if !self.embedded {
                let grad_top = theme.colors.surfaces.level2.lighten(0.03);
                let grad_bottom = theme.colors.surfaces.level2;
                cx.paint_vertical_gradient(
                    rect,
                    CELL_RADIUS,
                    grad_top,
                    grad_bottom,
                    2,
                    cx.scene_key,
                );
            }

            if !self.embedded {
                // Outer border — color/width changes when focused
                let (border_color, border_width) = if self.focused {
                    (theme.colors.borders.focus, 2.0)
                } else {
                    (theme.colors.borders.default, 1.0)
                };
                cx.scene.push_border_rounded_rect(
                    rect,
                    border_color,
                    border_width,
                    RoundedCorners::uniform(CELL_RADIUS),
                    2,
                    None,
                );

                // Focus ring — Capture mode only
                let focus_alpha = self.focus_t.get();
                if self.in_capture && focus_alpha > 0.001 {
                    let fr = &theme.focus_ring;
                    cx.scene.push_shadow(
                        rect,
                        CELL_RADIUS,
                        crate::core::paint::ShadowMode::Ring,
                        (0.0, 0.0),
                        fr.spread + 0.5,
                        0.4,
                        fr.capture_color.with_alpha(0.85 * focus_alpha),
                        LAYER_FOCUS,
                        None,
                    );
                }
            }

            // ================================================================
            // Header: month/year text centered, with arrow buttons
            // ================================================================
            let header_text = format!(
                "{} {}",
                MONTH_NAMES[(self.state.view_month - 1) as usize],
                self.state.view_year
            );
            let header_run = cx.text_engine.shape(&header_text, font_size);
            let header_text_x = rect.x + (rect.width - header_run.width) * 0.5;
            let metrics = cx.text_engine.font_metrics(font_size);
            let header_baseline_y = rect.y
                + compute_control_text_baseline_y(
                    HEADER_H,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                );
            let header_text_y = header_baseline_y - metrics.ascent;
            cx.scene.push_text_with_backgrounds(
                &header_run,
                header_text_x,
                header_text_y,
                theme.colors.typography.primary,
                vec![],
                self.paint_text_layer,
                None,
            );

            // Prev year arrow "«"
            let arrow_color = |idx: u8| {
                if self.hovered_arrow == Some(idx) {
                    theme.colors.typography.primary
                } else {
                    theme.colors.typography.secondary
                }
            };

            let py_run = cx.text_engine.shape("\u{00AB}", font_size);
            let py_x =
                rect.x + self.prev_year_rect.x + (self.prev_year_rect.width - py_run.width) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &py_run,
                py_x,
                header_text_y,
                arrow_color(0),
                vec![],
                self.paint_text_layer,
                None,
            );

            // Prev month arrow "<"
            let pm_run = cx.text_engine.shape("\u{2039}", font_size);
            let pm_x =
                rect.x + self.prev_month_rect.x + (self.prev_month_rect.width - pm_run.width) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &pm_run,
                pm_x,
                header_text_y,
                arrow_color(1),
                vec![],
                self.paint_text_layer,
                None,
            );

            // Next month arrow ">"
            let nm_run = cx.text_engine.shape("\u{203A}", font_size);
            let nm_x =
                rect.x + self.next_month_rect.x + (self.next_month_rect.width - nm_run.width) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &nm_run,
                nm_x,
                header_text_y,
                arrow_color(2),
                vec![],
                self.paint_text_layer,
                None,
            );

            // Next year arrow "»"
            let ny_run = cx.text_engine.shape("\u{00BB}", font_size);
            let ny_x =
                rect.x + self.next_year_rect.x + (self.next_year_rect.width - ny_run.width) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &ny_run,
                ny_x,
                header_text_y,
                arrow_color(3),
                vec![],
                self.paint_text_layer,
                None,
            );

            // Digit typeahead ghost indicator
            if let Some((d, _)) = self.digit_buffer {
                let ghost = format!("{}", d);
                let run = cx.text_engine.shape(&ghost, font_size * 0.85);
                let ghost_x = rect.x + self.header_rect.x + self.header_rect.width
                    - ARROW_W * 2.0
                    - run.width
                    - 4.0;
                cx.scene.push_text_with_backgrounds(
                    &run,
                    ghost_x,
                    header_text_y,
                    theme.colors.typography.muted,
                    vec![],
                    self.paint_text_layer,
                    None,
                );
            }

            // Year-jump ghost indicator — centered tightly under header date text
            if let Some(ref buf) = self.year_jump_buffer {
                let padded: String = format!("g:{:_<6}", buf);
                let ghost_font = font_size * 0.75;
                let run = cx.text_engine.shape(&padded, ghost_font);
                let ghost_x = rect.x + (rect.width - run.width) * 0.5;
                let ghost_metrics = cx.text_engine.font_metrics(ghost_font);
                let ghost_line_h = ghost_metrics.ascent + ghost_metrics.descent;
                let ghost_y = header_text_y + metrics.ascent + metrics.descent + 1.0
                    - ghost_metrics.ascent
                    + ghost_line_h * 0.5;
                cx.scene.push_text_with_backgrounds(
                    &run,
                    ghost_x,
                    ghost_y,
                    theme.colors.typography.muted,
                    vec![],
                    self.paint_text_layer,
                    None,
                );
            }

            // ================================================================
            // Day-of-week labels
            // ================================================================
            let labels = dow_labels(self.first_dow);
            let dow_baseline_y = rect.y
                + self.dow_row_rect.y
                + compute_control_text_baseline_y(
                    DOW_ROW_H,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                );
            let dow_text_y = dow_baseline_y - metrics.ascent;
            for (col, label) in labels.iter().enumerate() {
                let run = cx.text_engine.shape(label, font_size);
                let cx_pos = rect.x + col as f32 * CELL_W + (CELL_W - run.width) * 0.5;
                cx.scene.push_text_with_backgrounds(
                    &run,
                    cx_pos,
                    dow_text_y,
                    theme.colors.typography.muted,
                    vec![],
                    self.paint_text_layer,
                    None,
                );
            }

            // ================================================================
            // Day cells: 6 rows x 7 cols
            // ================================================================
            let offset =
                first_weekday_offset(self.state.view_year, self.state.view_month, self.first_dow);
            let max_days = date_value::days_in_month(self.state.view_year, self.state.view_month);

            for row in 0..6usize {
                for col in 0..7usize {
                    let cell_index = row * 7 + col;
                    let day_num = cell_index as i32 - offset as i32 + 1;
                    let cell_rect_local = self.cell_rects[row][col];
                    let cell_rect = RectF::new(
                        rect.x + cell_rect_local.x,
                        rect.y + cell_rect_local.y,
                        cell_rect_local.width,
                        cell_rect_local.height,
                    );

                    let is_in_month = day_num >= 1 && day_num <= max_days as i32;
                    let cell_date = if is_in_month {
                        DateValue::new(self.state.view_year, self.state.view_month, day_num as u8)
                    } else {
                        None
                    };
                    // Policy evaluation — per cell, closures called only for in-month dates
                    let (policy_highlight, policy_disabled) =
                        if let (Some(d), Some(pol)) = (cell_date, self.policy.as_ref()) {
                            let hi = pol.highlight.as_ref().and_then(|f| f(d));
                            let dis = pol.disabled.as_ref().map_or(false, |f| f(d));
                            (hi, dis)
                        } else {
                            (None, false)
                        };

                    // Check for tint mark (paint behind fill, only for in-month non-selected)
                    let has_tint = cell_date.map_or(false, |d| {
                        self.marked_dates
                            .iter()
                            .any(|m| m.date == d && m.kind == DateMarkKind::Tint)
                    }) || matches!(policy_highlight, Some(PolicyHighlight::Tint));

                    if !is_in_month {
                        // Out-of-month: show adjacent month day at muted alpha
                        let adj_day = if day_num < 1 {
                            // Previous month
                            let prev_month = if self.state.view_month == 1 {
                                12
                            } else {
                                self.state.view_month - 1
                            };
                            let prev_year = if self.state.view_month == 1 {
                                self.state.view_year - 1
                            } else {
                                self.state.view_year
                            };
                            let prev_days = date_value::days_in_month(prev_year, prev_month);
                            (prev_days as i32 + day_num) as u8
                        } else {
                            // Next month
                            (day_num - max_days as i32) as u8
                        };
                        let text = format!("{}", adj_day);
                        let run = cx.text_engine.shape(&text, font_size);
                        let tx = cell_rect.x + (CELL_W - run.width) * 0.5;
                        let cell_baseline = cell_rect.y
                            + compute_control_text_baseline_y(
                                CELL_H,
                                metrics.ascent,
                                metrics.descent,
                                theme.typography.baseline_offset,
                            );
                        let ty = cell_baseline - metrics.ascent;
                        cx.scene.push_text_with_backgrounds(
                            &run,
                            tx,
                            ty,
                            theme.colors.typography.muted.with_alpha(0.4),
                            vec![],
                            self.paint_text_layer,
                            None,
                        );
                        continue;
                    }

                    let day = day_num as u8;
                    let date = DateValue::new(self.state.view_year, self.state.view_month, day);
                    let is_selected = date.is_some() && self.state.selected == date;
                    let is_cursor = date.is_some() && self.state.kbd_cursor == date;
                    let is_hovered = self.hovered_cell == Some((row, col));
                    let is_today = date.is_some() && self.today == date;
                    let is_disabled =
                        date.map_or(true, |d| !self.state.is_enabled(d)) || policy_disabled;

                    // Background fill based on priority: selected > cursor > hover
                    let inset = 2.0;
                    let fill_rect = RectF::new(
                        cell_rect.x + inset,
                        cell_rect.y + inset,
                        cell_rect.width - inset * 2.0,
                        cell_rect.height - inset * 2.0,
                    );

                    // Tint mark: behind fills, only if not selected
                    // When embedded: deferred into TextBackgroundSpan below.
                    // When standalone: render as geometry.
                    if !self.embedded && !is_selected {
                        let custom_tint_color = match policy_highlight {
                            Some(PolicyHighlight::Custom(c)) => Some(c.with_alpha(0.18)),
                            _ => None,
                        };
                        if has_tint {
                            let tint_color = custom_tint_color
                                .unwrap_or_else(|| theme.colors.accents.primary.with_alpha(0.12));
                            cx.scene.push_rounded_rect(
                                fill_rect,
                                tint_color,
                                RoundedCorners::uniform(CELL_RADIUS),
                                3,
                                None,
                            );
                        } else if let Some(c) = custom_tint_color {
                            cx.scene.push_rounded_rect(
                                fill_rect,
                                c,
                                RoundedCorners::uniform(CELL_RADIUS),
                                3,
                                None,
                            );
                        }
                    }

                    if !self.embedded {
                        if is_selected {
                            cx.scene.push_rounded_rect(
                                fill_rect,
                                theme.colors.accents.primary,
                                RoundedCorners::uniform(CELL_RADIUS),
                                3,
                                None,
                            );
                        } else if is_cursor && !is_disabled {
                            let cursor_color = theme.colors.borders.focus.with_alpha(0.4);
                            cx.scene.push_rounded_rect(
                                fill_rect,
                                cursor_color,
                                RoundedCorners::uniform(CELL_RADIUS),
                                3,
                                None,
                            );
                        } else if is_hovered && !is_disabled {
                            cx.scene.push_rounded_rect(
                                fill_rect,
                                theme.colors.list.row_hover,
                                RoundedCorners::uniform(CELL_RADIUS),
                                3,
                                None,
                            );
                        }
                    }

                    // Today ring
                    if is_today {
                        if self.embedded {
                            // Overlay geometry — renders after text pass, visible over blocker.
                            cx.scene.push_border_rounded_rect(
                                fill_rect,
                                theme.colors.borders.focus,
                                1.0,
                                RoundedCorners::uniform(CELL_RADIUS),
                                crate::core::paint::LAYER_OVERLAY + 1,
                                None,
                            );
                        } else {
                            cx.scene.push_border_rounded_rect(
                                fill_rect,
                                theme.colors.borders.focus,
                                1.0,
                                RoundedCorners::uniform(CELL_RADIUS),
                                4,
                                None,
                            );
                        }
                    }

                    // Text
                    let text_color = if is_selected {
                        theme.colors.typography.inverse
                    } else if is_disabled {
                        theme.colors.typography.muted
                    } else {
                        theme.colors.typography.primary
                    };

                    let text = format!("{}", day);
                    let run = cx.text_engine.shape(&text, font_size);
                    let tx = cell_rect.x + (CELL_W - run.width) * 0.5;
                    let cell_baseline = cell_rect.y
                        + compute_control_text_baseline_y(
                            CELL_H,
                            metrics.ascent,
                            metrics.descent,
                            theme.typography.baseline_offset,
                        );
                    let ty = cell_baseline - metrics.ascent;

                    // When embedded: attach fill as TextBackgroundSpan so it renders
                    // in the text pass (after the background blocker at LAYER_OVERLAY+1,
                    // but before glyphs at the same z level).
                    let cell_backgrounds = if self.embedded {
                        use crate::core::paint::TextBackgroundSpan;
                        let mut spans: Vec<TextBackgroundSpan> = Vec::new();

                        // Tint (lowest priority — behind fill)
                        if !is_selected {
                            let custom_tint_color = match policy_highlight {
                                Some(PolicyHighlight::Custom(c)) => Some(c.with_alpha(0.18)),
                                _ => None,
                            };
                            let tint_color = if has_tint {
                                Some(custom_tint_color.unwrap_or_else(|| {
                                    theme.colors.accents.primary.with_alpha(0.12)
                                }))
                            } else {
                                custom_tint_color
                            };
                            if let Some(c) = tint_color {
                                spans.push(TextBackgroundSpan {
                                    x_offset: fill_rect.x - tx,
                                    y_offset: fill_rect.y - ty,
                                    width: fill_rect.width,
                                    height: fill_rect.height,
                                    color: c,
                                });
                            }
                        }

                        // Cell fill (selected / cursor / hover)
                        // Hover uses focus tint — row_hover is too close to level2 blocker bg.
                        let fill_color = if is_selected {
                            Some(theme.colors.accents.primary)
                        } else if is_cursor && !is_disabled {
                            Some(theme.colors.borders.focus.with_alpha(0.4))
                        } else if is_hovered && !is_disabled {
                            Some(theme.colors.borders.focus.with_alpha(0.15))
                        } else {
                            None
                        };
                        if let Some(c) = fill_color {
                            spans.push(TextBackgroundSpan {
                                x_offset: fill_rect.x - tx,
                                y_offset: fill_rect.y - ty,
                                width: fill_rect.width,
                                height: fill_rect.height,
                                color: c,
                            });
                        }

                        // Dot mark as background span (bottom of cell)
                        if let Some(d) = cell_date {
                            let has_dot = self
                                .marked_dates
                                .iter()
                                .any(|m| m.date == d && m.kind == DateMarkKind::Dot)
                                || matches!(policy_highlight, Some(PolicyHighlight::Dot));
                            if has_dot {
                                let dot_size = 3.0;
                                let dot_x = cell_rect.x + (CELL_W - dot_size) * 0.5;
                                let dot_y = cell_rect.y + cell_rect.height - 3.0 - dot_size;
                                let dot_color = if is_selected {
                                    theme.colors.typography.inverse
                                } else {
                                    theme.colors.accents.primary
                                };
                                spans.push(TextBackgroundSpan {
                                    x_offset: dot_x - tx,
                                    y_offset: dot_y - ty,
                                    width: dot_size,
                                    height: dot_size,
                                    color: dot_color,
                                });
                            }
                        }

                        spans
                    } else {
                        vec![]
                    };

                    cx.scene.push_text_with_backgrounds(
                        &run,
                        tx,
                        ty,
                        text_color,
                        cell_backgrounds,
                        self.paint_text_layer,
                        None,
                    );

                    // Dot mark — standalone mode only (embedded handled in cell_backgrounds above)
                    if !self.embedded {
                        if let Some(d) = cell_date {
                            let has_dot = self
                                .marked_dates
                                .iter()
                                .any(|m| m.date == d && m.kind == DateMarkKind::Dot)
                                || matches!(policy_highlight, Some(PolicyHighlight::Dot));
                            if has_dot {
                                let dot_size = 3.0;
                                let dot_x = cell_rect.x + (CELL_W - dot_size) * 0.5;
                                let dot_y = cell_rect.y + cell_rect.height - 3.0 - dot_size;
                                let dot_rect = RectF::new(dot_x, dot_y, dot_size, dot_size);
                                let dot_color = if is_selected {
                                    theme.colors.typography.inverse
                                } else {
                                    theme.colors.accents.primary
                                };
                                cx.scene.push_rounded_rect(
                                    dot_rect,
                                    dot_color,
                                    RoundedCorners::uniform(1.5),
                                    5,
                                    None,
                                );
                            }
                        }
                    }
                }
            }
        });
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        let result = match event {
            Event::Pointer(pe) => {
                let (lx, ly) = pe.local_pos;
                match pe.kind {
                    PointerEventKind::Move => {
                        // Hit-test arrows
                        let old_arrow = self.hovered_arrow;
                        self.hovered_arrow = self.hit_test_arrow(lx, ly);

                        // Hit-test cells
                        let old_cell = self.hovered_cell;
                        self.hovered_cell = self.hit_test_cell(lx, ly);

                        old_arrow != self.hovered_arrow || old_cell != self.hovered_cell
                    }
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }

                        if !self.embedded {
                            cx.request_keyboard_capture();
                        }

                        // Check arrow buttons first
                        if let Some(arrow) = self.hit_test_arrow(lx, ly) {
                            match arrow {
                                0 => self.state.prev_year(),
                                1 => self.state.prev_month(),
                                2 => self.state.next_month(),
                                3 => self.state.next_year(),
                                _ => {}
                            }
                            return true;
                        }

                        // Check day cells
                        if let Some((row, col)) = self.hit_test_cell(lx, ly) {
                            if let Some(day) = self.cell_day(row, col) {
                                if let Some(date) =
                                    DateValue::new(self.state.view_year, self.state.view_month, day)
                                {
                                    if !self.policy_allows(date) {
                                        return true;
                                    }
                                }
                                self.state.commit_day(day);
                                if self.state.dirty {
                                    self.emit_binding();
                                    // Don't clear dirty here — take_changed handles that
                                }
                            }
                        }

                        true
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down => {
                if !self.focused {
                    return false;
                }
                // Year-jump mode: Enter applies partial buffer, other non-character keys cancel
                if self.year_jump_buffer.is_some() {
                    if ke.key_code == KeyCode::Enter
                        || matches!(ke.key_code, KeyCode::Character('i'))
                    {
                        let buf = self.year_jump_buffer.take().unwrap();
                        if buf.len() >= 4 {
                            if let Ok(year) = buf[..4].parse::<i32>() {
                                if year >= 1 && year <= 9999 {
                                    self.state.view_year = year;
                                    if buf.len() >= 6 {
                                        if let Ok(month) = buf[4..6].parse::<u8>() {
                                            if month >= 1 && month <= 12 {
                                                self.state.view_month = month;
                                            }
                                        }
                                    }
                                    self.state.clamp_view_to_constraints();
                                    self.clamp_cursor_to_view();
                                }
                            }
                        }
                        return true;
                    }
                    if !matches!(ke.key_code, KeyCode::Character(_)) {
                        self.year_jump_buffer = None;
                    }
                }
                match ke.key_code {
                    KeyCode::Left if ke.modifiers.ctrl => {
                        self.flush_digit_buffer();
                        self.state.prev_month();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::Right if ke.modifiers.ctrl => {
                        self.flush_digit_buffer();
                        self.state.next_month();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::Up if ke.modifiers.ctrl => {
                        self.flush_digit_buffer();
                        self.state.prev_year();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::Down if ke.modifiers.ctrl => {
                        self.flush_digit_buffer();
                        self.state.next_year();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::Left if ke.modifiers.alt || ke.modifiers.meta => {
                        self.flush_digit_buffer();
                        self.state.prev_month();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::Right if ke.modifiers.alt || ke.modifiers.meta => {
                        self.flush_digit_buffer();
                        self.state.next_month();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::Left => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(-1);
                        true
                    }
                    KeyCode::Right => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(1);
                        true
                    }
                    KeyCode::Up => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(-7);
                        true
                    }
                    KeyCode::Down => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(7);
                        true
                    }
                    KeyCode::PageUp => {
                        self.flush_digit_buffer();
                        self.state.prev_month();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::PageDown => {
                        self.flush_digit_buffer();
                        self.state.next_month();
                        self.clamp_cursor_to_view();
                        true
                    }
                    KeyCode::Enter => {
                        if !self.in_capture {
                            // Navigation mode: Enter enters Capture (same as `i`)
                            cx.request_keyboard_capture();
                            true
                        } else {
                            // Capture mode: commit cursor date, release Capture (focus stays)
                            self.flush_digit_buffer();
                            self.commit_cursor_selection();
                            cx.release_keyboard_capture();
                            true
                        }
                    }
                    KeyCode::Character('i') if self.in_capture => {
                        // SPEC-HAL-176: `i` is the Navigation-mode Enter alias.
                        // In Capture mode, `i` commits the cursor's date and
                        // releases capture. Unlike plain Enter, `i` uses
                        // `force_commit_cursor` (no toggle): pressing `i` on
                        // the already-selected date is idempotent — it does
                        // NOT deselect. JC's spec: "no matter what interaction
                        // policy of DatePicker dictates, pressing `i` on the
                        // date will select AND commit it."
                        //
                        // Navigation-mode entry (`i` → ENTER_WIDGET → Capture)
                        // is handled exclusively by the framework binding
                        // registry — dispatch_engine_action consumes the
                        // Key(Character('i')) event before route_input_event
                        // runs. handle_event only sees Character('i') here
                        // when already in Capture, where dispatch_engine_action's
                        // Capture branch returns false for plain keys and lets
                        // the event fall through.
                        //
                        // The paired TextInput("i") that arrives in the same
                        // frame is harmless: after this arm runs, release_capture
                        // flips in_capture to false, and the Event::Text branch's
                        // `if self.in_capture` guard rejects the Text event.
                        self.flush_digit_buffer();
                        self.force_commit_cursor();
                        cx.release_keyboard_capture();
                        true
                    }
                    KeyCode::Space => {
                        self.flush_digit_buffer();
                        self.commit_cursor_selection();
                        true
                    }
                    KeyCode::Tab => {
                        self.in_capture = false;
                        self.digit_buffer = None;
                        false
                    }
                    _ => false,
                }
            }
            Event::Text(te) if self.in_capture => {
                if te.text.len() != 1 {
                    return false;
                }
                let ch = te.text.chars().next().unwrap();
                // Ctrl+h/l: month navigation (before the modifier drop guard)
                if te.modifiers.ctrl && !te.modifiers.alt && !te.modifiers.meta {
                    match ch {
                        'h' => {
                            self.flush_digit_buffer();
                            self.state.prev_month();
                            self.clamp_cursor_to_view();
                            return true;
                        }
                        'l' => {
                            self.flush_digit_buffer();
                            self.state.next_month();
                            self.clamp_cursor_to_view();
                            return true;
                        }
                        _ => return false,
                    }
                }
                // Don't consume modified text input
                if te.modifiers.alt || te.modifiers.meta {
                    return false;
                }

                // Year-jump mode: digits go into year_jump_buffer
                if let Some(ref mut buf) = self.year_jump_buffer {
                    if ch.is_ascii_digit() {
                        buf.push(ch);
                        if buf.len() == 6 {
                            // Full YYYYMM — parse and apply
                            if let (Ok(year), Ok(month)) =
                                (buf[..4].parse::<i32>(), buf[4..].parse::<u8>())
                            {
                                if year >= 1 && year <= 9999 && month >= 1 && month <= 12 {
                                    self.state.view_year = year;
                                    self.state.view_month = month;
                                    self.state.clamp_view_to_constraints();
                                    self.clamp_cursor_to_view();
                                }
                            }
                            self.year_jump_buffer = None;
                        } else if buf.len() == 4 {
                            // Year complete — validate silently, keep accepting digits for month
                            if let Ok(year) = buf.parse::<i32>() {
                                if !(year >= 1 && year <= 9999) {
                                    // Invalid year typed — abort
                                    self.year_jump_buffer = None;
                                }
                            }
                        }
                        return true;
                    } else {
                        self.year_jump_buffer = None;
                        // fall through to normal handling
                    }
                }

                match ch {
                    'h' => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(-1);
                        true
                    }
                    'j' => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(7);
                        true
                    }
                    'k' => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(-7);
                        true
                    }
                    'l' => {
                        self.flush_digit_buffer();
                        self.state.move_kbd_cursor(1);
                        true
                    }
                    't' => {
                        self.flush_digit_buffer();
                        if let Some(today) = self.today {
                            self.state.jump_to_date(today);
                        }
                        true
                    }
                    'g' => {
                        self.flush_digit_buffer();
                        self.year_jump_buffer = Some(String::new());
                        true
                    }
                    c if c.is_ascii_digit() => {
                        let digit = c as u8 - b'0';
                        let now = cx.frame_time_ms();

                        if let Some((first, typed_at)) = self.digit_buffer {
                            if now.saturating_sub(typed_at) < 600 {
                                let two_digit = first * 10 + digit;
                                let max = crate::model::date_value::days_in_month(
                                    self.state.view_year,
                                    self.state.view_month,
                                );
                                if two_digit >= 1 && two_digit <= max {
                                    if let Some(date) = DateValue::new(
                                        self.state.view_year,
                                        self.state.view_month,
                                        two_digit,
                                    ) {
                                        self.set_cursor(date);
                                        self.state.view_year = date.year;
                                        self.state.view_month = date.month;
                                    }
                                    self.digit_buffer = None;
                                    return true;
                                }
                                self.flush_digit_buffer();
                            } else {
                                self.flush_digit_buffer();
                            }
                        }

                        if digit == 0 {
                            self.digit_buffer = Some((0, now));
                        } else {
                            let max = crate::model::date_value::days_in_month(
                                self.state.view_year,
                                self.state.view_month,
                            );
                            if digit <= max {
                                if let Some(date) = DateValue::new(
                                    self.state.view_year,
                                    self.state.view_month,
                                    digit,
                                ) {
                                    self.set_cursor(date);
                                    self.state.view_year = date.year;
                                    self.state.view_month = date.month;
                                }
                            }
                            self.digit_buffer = Some((digit, now));
                        }

                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        };

        // Sync last_cursor from any state-driven cursor change
        if let Some(c) = self.state.kbd_cursor {
            self.last_cursor = Some(c);
        }

        result
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
        let anim = Animation {
            from: self.focus_t.get(),
            to: 1.0,
            duration: 0.08,
            easing: Easing::EaseOut,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.focus_anim.set(Some(handle));
        } else {
            self.focus_t.set(1.0);
            self.focus_anim.set(None);
        }
        self.restore_cursor_if_needed();
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        self.in_capture = true;
        self.restore_cursor_if_needed();
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        self.in_capture = false;
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.focused = false;
        self.in_capture = false;
        self.digit_buffer = None;
        self.year_jump_buffer = None;
        let anim = Animation {
            from: self.focus_t.get(),
            to: 0.0,
            duration: 0.08,
            easing: Easing::EaseIn,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.focus_anim.set(Some(handle));
        } else {
            self.focus_t.set(0.0);
            self.focus_anim.set(None);
        }
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        let anim = Animation {
            from: self.hover_t.get(),
            to: 1.0,
            duration: 0.08,
            easing: Easing::EaseOut,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.hover_anim.set(Some(handle));
        } else {
            self.hover_t.set(1.0);
            self.hover_anim.set(None);
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.hovered_cell = None;
        self.hovered_arrow = None;
        let anim = Animation {
            from: self.hover_t.get(),
            to: 0.0,
            duration: 0.08,
            easing: Easing::EaseIn,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.hover_anim.set(Some(handle));
        } else {
            self.hover_t.set(0.0);
            self.hover_anim.set(None);
        }
    }

    fn on_mount(&mut self) {
        self.hover_t.set(0.0);
        self.hover_anim.set(None);
        self.focus_t.set(0.0);
        self.focus_anim.set(None);
    }

    fn debug_type_name(&self) -> &'static str {
        "DatePickerWidget"
    }
}
