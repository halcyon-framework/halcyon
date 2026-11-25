// =============================================================================
// SPEC-HAL-335 §4: TimeGeometryEngine — Temporal Horizontal Geometry Authority
// =============================================================================
//
// The time-dimension peer of ColumnGeometryEngine: sole owner of the mapping
// from absolute time to horizontal pixels.
//
// INV-335-G1: Single horizontal authority. No scroll_x, no mirrored offset,
//             no second origin exists anywhere. Horizontal navigation mutates
//             this engine and nothing else.
// INV-335-G2: x_of / instant_at are pure, deterministic, mutually inverse
//             within the viewport, and viewport-local (x = 0 at the left
//             edge). The mapping is LINEAR in absolute time. TimeScale
//             parameterizes tier composition, gridlines, and the zoom unit —
//             it never warps the mapping.
// INV-335-G3: zoom_at(x, factor) preserves the instant at x.
// INV-335-G5: resolve() is pure; once-per-frame consumption is enforced at
//             the consumer seam, mirroring ColumnGeometryEngine evaluation
//             discipline.
// INV-335-G6: Tier composition per scale.
// INV-335-G7: The engine holds the DEFAULT working calendar only. Per-row
//             calendars are data (RowBandDescriptor), never engine state.
//
// The engine never reads the system clock: `now` is application-fed.
// =============================================================================

use crate::model::date_value::DateValue;
use crate::model::time::{
    MICROS_PER_DAY, MICROS_PER_HOUR, MICROS_PER_MINUTE, MICROS_PER_SECOND, TimeFrame, TimeRange,
    Timestamp, day_of_week, epoch_days, from_civil, to_civil,
};
use crate::model::time_value::TimeValue;

const DEFAULT_MIN_PX_PER_UNIT: f32 = 16.0;
const DEFAULT_MAX_PX_PER_UNIT: f32 = 512.0;
const MAX_TIER_CELLS: usize = 1024;
const MAX_NON_WORKING_DAYS: usize = 4096;

const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn midnight() -> TimeValue {
    TimeValue::new(0, 0, None)
}

// =============================================================================
// TimeScale
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeScale {
    Seconds,
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
}

impl TimeScale {
    /// Zoom unit in microseconds. Months uses a 30-day convenience unit for
    /// zoom arithmetic ONLY; tier and gridline boundaries are true calendar
    /// boundaries (variable month widths under the linear mapping).
    pub fn unit_us(self) -> i64 {
        match self {
            TimeScale::Seconds => MICROS_PER_SECOND,
            TimeScale::Minutes => MICROS_PER_MINUTE,
            TimeScale::Hours => MICROS_PER_HOUR,
            TimeScale::Days => MICROS_PER_DAY,
            TimeScale::Weeks => 7 * MICROS_PER_DAY,
            TimeScale::Months => 30 * MICROS_PER_DAY,
        }
    }
}

// =============================================================================
// WorkingCalendar — INV-335-G7: default calendar only
// =============================================================================

#[derive(Debug, Clone)]
pub struct WorkingCalendar {
    /// Index = day of week, 0 = Monday … 6 = Sunday.
    pub working_days: [bool; 7],
    pub day_start: TimeValue,
    pub day_end: TimeValue,
    pub holidays: Vec<DateValue>,
}

impl Default for WorkingCalendar {
    fn default() -> Self {
        Self {
            working_days: [true, true, true, true, true, false, false],
            day_start: TimeValue::new(9, 0, None),
            day_end: TimeValue::new(17, 0, None),
            holidays: Vec::new(),
        }
    }
}

impl WorkingCalendar {
    pub fn is_working_day(&self, date: DateValue) -> bool {
        self.working_days[day_of_week(date) as usize] && !self.holidays.contains(&date)
    }
}

// =============================================================================
// ISO-8601 week numbering
// =============================================================================

/// ISO-8601 weeks in the given year: 53 iff Jan 1 or Dec 31 is a Thursday.
pub fn iso_weeks_in_year(year: i32) -> u32 {
    let thu = |d: DateValue| day_of_week(d) == 3;
    let jan1 = DateValue {
        year,
        month: 1,
        day: 1,
    };
    let dec31 = DateValue {
        year,
        month: 12,
        day: 31,
    };
    if thu(jan1) || thu(dec31) { 53 } else { 52 }
}

/// ISO-8601 week number of the given date.
pub fn iso_week(date: DateValue) -> u32 {
    let wd = day_of_week(date) as i64 + 1; // 1 = Monday … 7 = Sunday
    let jan1 = DateValue {
        year: date.year,
        month: 1,
        day: 1,
    };
    let ordinal = epoch_days(date) - epoch_days(jan1) + 1;
    let week = (ordinal - wd + 10) / 7;
    if week < 1 {
        iso_weeks_in_year(date.year - 1)
    } else if week == 53 && iso_weeks_in_year(date.year) == 52 {
        1
    } else {
        week as u32
    }
}

// =============================================================================
// Header types — the contract TimelineHeader paints (SPEC-HAL-336)
// =============================================================================

/// One labeled cell of a header tier. x/width are viewport-local and may
/// extend beyond the viewport; the painter clips.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderCell {
    pub x: f32,
    pub width: f32,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeaderTier {
    pub cells: Vec<HeaderCell>,
}

/// Per-frame resolution consumed by header, canvas, and hit-testing.
/// INV-335-G5: computed once per frame; all consumers share one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeGeometryResolution {
    pub visible_range: TimeRange,
    /// tiers[0] = coarse (top), tiers[1] = fine (bottom). INV-335-G6.
    pub tiers: Vec<HeaderTier>,
    /// x of fine-unit boundaries within [0, viewport_width].
    pub gridlines: Vec<f32>,
    /// x-spans (start, end) where the default calendar is non-working,
    /// merged where contiguous and clamped to the viewport.
    pub non_working: Vec<(f32, f32)>,
    /// x of the application-fed `now`, if inside the viewport.
    pub now_x: Option<f32>,
}

// =============================================================================
// Tier units
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TierUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

/// INV-335-G6 composition: (top, bottom) per scale.
fn tier_units(scale: TimeScale) -> (TierUnit, TierUnit) {
    match scale {
        TimeScale::Months => (TierUnit::Year, TierUnit::Month),
        TimeScale::Weeks => (TierUnit::Month, TierUnit::Week),
        TimeScale::Days => (TierUnit::Month, TierUnit::Day),
        TimeScale::Hours => (TierUnit::Day, TierUnit::Hour),
        TimeScale::Minutes => (TierUnit::Hour, TierUnit::Minute),
        TimeScale::Seconds => (TierUnit::Minute, TierUnit::Second),
    }
}

// =============================================================================
// TimeGeometryEngine
// =============================================================================

#[derive(Debug, Clone)]
pub struct TimeGeometryEngine {
    /// Instant at viewport x = 0. The only horizontal position state.
    anchor: Timestamp,
    px_per_unit: f32,
    scale: TimeScale,
    viewport_width: f32,
    frame: TimeFrame,
    calendar: WorkingCalendar,
    now: Option<Timestamp>,
    min_px_per_unit: f32,
    max_px_per_unit: f32,
}

impl TimeGeometryEngine {
    pub fn new(anchor: Timestamp, scale: TimeScale, px_per_unit: f32, viewport_width: f32) -> Self {
        Self {
            anchor,
            px_per_unit: px_per_unit.clamp(DEFAULT_MIN_PX_PER_UNIT, DEFAULT_MAX_PX_PER_UNIT),
            scale,
            viewport_width,
            frame: TimeFrame::default(),
            calendar: WorkingCalendar::default(),
            now: None,
            min_px_per_unit: DEFAULT_MIN_PX_PER_UNIT,
            max_px_per_unit: DEFAULT_MAX_PX_PER_UNIT,
        }
    }

    // ------------------------------------------------------------------ state

    pub fn set_frame(&mut self, frame: TimeFrame) {
        self.frame = frame;
    }

    pub fn set_calendar(&mut self, calendar: WorkingCalendar) {
        self.calendar = calendar;
    }

    /// Application-fed `now`. The engine never reads the system clock.
    pub fn set_now(&mut self, now: Option<Timestamp>) {
        self.now = now;
    }

    pub fn set_viewport_width(&mut self, width: f32) {
        self.viewport_width = width;
    }

    pub fn set_zoom_limits(&mut self, min: f32, max: f32) {
        debug_assert!(min > 0.0 && min <= max, "invalid zoom limits");
        self.min_px_per_unit = min;
        self.max_px_per_unit = max;
        self.px_per_unit = self.px_per_unit.clamp(min, max);
    }

    pub fn anchor(&self) -> Timestamp {
        self.anchor
    }

    pub fn scale(&self) -> TimeScale {
        self.scale
    }

    pub fn px_per_unit(&self) -> f32 {
        self.px_per_unit
    }

    pub fn frame(&self) -> TimeFrame {
        self.frame
    }

    pub fn viewport_width(&self) -> f32 {
        self.viewport_width
    }

    pub fn calendar(&self) -> &WorkingCalendar {
        &self.calendar
    }

    // ---------------------------------------------------------------- mapping

    fn px_per_us(&self) -> f64 {
        self.px_per_unit as f64 / self.scale.unit_us() as f64
    }

    /// INV-335-G2: viewport-local x of an absolute instant. Linear.
    pub fn x_of(&self, ts: Timestamp) -> f32 {
        ((ts.0 - self.anchor.0) as f64 * self.px_per_us()) as f32
    }

    /// INV-335-G2: inverse of x_of within the viewport.
    pub fn instant_at(&self, x: f32) -> Timestamp {
        Timestamp(self.anchor.0 + (x as f64 / self.px_per_us()).round() as i64)
    }

    pub fn visible_range(&self) -> TimeRange {
        let end = self.instant_at(self.viewport_width).max(self.anchor);
        TimeRange::new(self.anchor, end)
    }

    // ------------------------------------------------------------- navigation

    /// INV-335-G4. Positive dx scrolls the view forward in time.
    pub fn scroll_by_px(&mut self, dx: f32) {
        self.anchor = Timestamp(self.anchor.0 + (dx as f64 / self.px_per_us()).round() as i64);
    }

    pub fn scroll_to(&mut self, ts: Timestamp) {
        self.anchor = ts;
    }

    /// INV-335-G3: the instant at x is identical before and after zoom.
    pub fn zoom_at(&mut self, x: f32, factor: f32) {
        debug_assert!(factor > 0.0, "zoom factor must be positive");
        let pinned = self.instant_at(x);
        self.px_per_unit =
            (self.px_per_unit * factor).clamp(self.min_px_per_unit, self.max_px_per_unit);
        self.anchor = Timestamp(pinned.0 - (x as f64 / self.px_per_us()).round() as i64);
    }

    /// Changes tier/gridline composition while preserving the on-screen
    /// mapping: px-per-microsecond is invariant across the scale change
    /// (then clamped to the zoom limits).
    pub fn set_scale(&mut self, scale: TimeScale) {
        let ppus = self.px_per_us();
        self.scale = scale;
        self.px_per_unit = ((ppus * scale.unit_us() as f64) as f32)
            .clamp(self.min_px_per_unit, self.max_px_per_unit);
    }

    /// Fit the viewport to `range` (SPEC-HAL-341 INV-341-V1): anchor at its
    /// start, choose the finest scale spanning at most ~48 units so the header
    /// tiers suit the span (INV-335-G6), and set px-per-unit so the range fills
    /// the viewport. Exact when the resulting px-per-unit is within the zoom
    /// band; clamped (slightly loose) otherwise.
    pub fn fit_range(&mut self, range: TimeRange) {
        let dur_us = (range.end.0 - range.start.0).max(1) as f64;
        let vw = self.viewport_width.max(1.0) as f64;
        let mut chosen = TimeScale::Months;
        for s in [
            TimeScale::Seconds,
            TimeScale::Minutes,
            TimeScale::Hours,
            TimeScale::Days,
            TimeScale::Weeks,
            TimeScale::Months,
        ] {
            chosen = s;
            if dur_us / s.unit_us() as f64 <= 48.0 {
                break;
            }
        }
        self.scale = chosen;
        let units = dur_us / chosen.unit_us() as f64;
        self.px_per_unit = ((vw / units) as f32).clamp(self.min_px_per_unit, self.max_px_per_unit);
        self.anchor = range.start;
    }

    // ------------------------------------------------------ boundary walking

    fn shifted(&self, ts: Timestamp) -> i64 {
        ts.0 + self.frame.utc_offset_minutes as i64 * MICROS_PER_MINUTE
    }

    fn unshift(&self, s: i64) -> Timestamp {
        Timestamp(s - self.frame.utc_offset_minutes as i64 * MICROS_PER_MINUTE)
    }

    fn floor_to(&self, ts: Timestamp, unit: TierUnit) -> Timestamp {
        let s = self.shifted(ts);
        match unit {
            TierUnit::Second => self.unshift(s.div_euclid(MICROS_PER_SECOND) * MICROS_PER_SECOND),
            TierUnit::Minute => self.unshift(s.div_euclid(MICROS_PER_MINUTE) * MICROS_PER_MINUTE),
            TierUnit::Hour => self.unshift(s.div_euclid(MICROS_PER_HOUR) * MICROS_PER_HOUR),
            TierUnit::Day => self.unshift(s.div_euclid(MICROS_PER_DAY) * MICROS_PER_DAY),
            TierUnit::Week => {
                let days = s.div_euclid(MICROS_PER_DAY);
                let dow = (days + 3).rem_euclid(7); // 0 = Monday
                self.unshift((days - dow) * MICROS_PER_DAY)
            }
            TierUnit::Month => {
                let (date, _) = to_civil(ts, self.frame);
                let first = DateValue {
                    year: date.year,
                    month: date.month,
                    day: 1,
                };
                from_civil(first, midnight(), self.frame)
            }
            TierUnit::Year => {
                let (date, _) = to_civil(ts, self.frame);
                let first = DateValue {
                    year: date.year,
                    month: 1,
                    day: 1,
                };
                from_civil(first, midnight(), self.frame)
            }
        }
    }

    fn next_boundary(&self, at: Timestamp, unit: TierUnit) -> Timestamp {
        match unit {
            TierUnit::Second => Timestamp(at.0 + MICROS_PER_SECOND),
            TierUnit::Minute => Timestamp(at.0 + MICROS_PER_MINUTE),
            TierUnit::Hour => Timestamp(at.0 + MICROS_PER_HOUR),
            TierUnit::Day => Timestamp(at.0 + MICROS_PER_DAY),
            TierUnit::Week => Timestamp(at.0 + 7 * MICROS_PER_DAY),
            TierUnit::Month => {
                let (date, _) = to_civil(at, self.frame);
                let (y, m) = if date.month == 12 {
                    (date.year + 1, 1)
                } else {
                    (date.year, date.month + 1)
                };
                from_civil(
                    DateValue {
                        year: y,
                        month: m,
                        day: 1,
                    },
                    midnight(),
                    self.frame,
                )
            }
            TierUnit::Year => {
                let (date, _) = to_civil(at, self.frame);
                from_civil(
                    DateValue {
                        year: date.year + 1,
                        month: 1,
                        day: 1,
                    },
                    midnight(),
                    self.frame,
                )
            }
        }
    }

    fn unit_label(&self, at: Timestamp, unit: TierUnit, top: bool) -> String {
        let (date, time) = to_civil(at, self.frame);
        match unit {
            TierUnit::Year => format!("{}", date.year),
            TierUnit::Month => {
                let m = MONTH_ABBR[(date.month - 1) as usize];
                if top {
                    format!("{} {}", m, date.year)
                } else {
                    m.to_string()
                }
            }
            TierUnit::Week => format!("W{}", iso_week(date)),
            TierUnit::Day => {
                if top {
                    format!("{} {}", MONTH_ABBR[(date.month - 1) as usize], date.day)
                } else {
                    format!("{}", date.day)
                }
            }
            TierUnit::Hour => {
                if top {
                    format!("{:02}:00", time.hour)
                } else {
                    format!("{:02}", time.hour)
                }
            }
            TierUnit::Minute => {
                if top {
                    format!("{:02}:{:02}", time.hour, time.minute)
                } else {
                    format!("{:02}", time.minute)
                }
            }
            TierUnit::Second => format!("{:02}", time.second.unwrap_or(0)),
        }
    }

    // -------------------------------------------------------------- resolve

    fn build_tier(&self, unit: TierUnit, top: bool, range: TimeRange) -> HeaderTier {
        let mut cells = Vec::new();
        let mut at = self.floor_to(range.start, unit);
        let mut guard = 0usize;
        while at < range.end {
            let next = self.next_boundary(at, unit);
            let x = self.x_of(at);
            let width = self.x_of(next) - x;
            cells.push(HeaderCell {
                x,
                width,
                label: self.unit_label(at, unit, top),
            });
            at = next;
            guard += 1;
            if guard > MAX_TIER_CELLS {
                break;
            }
        }
        HeaderTier { cells }
    }

    fn build_gridlines(&self, unit: TierUnit, range: TimeRange) -> Vec<f32> {
        let mut out = Vec::new();
        let mut at = self.floor_to(range.start, unit);
        let mut guard = 0usize;
        while at < range.end {
            let x = self.x_of(at);
            if x >= 0.0 && x <= self.viewport_width {
                out.push(x);
            }
            at = self.next_boundary(at, unit);
            guard += 1;
            if guard > MAX_TIER_CELLS {
                break;
            }
        }
        out
    }

    fn build_non_working(&self, range: TimeRange) -> Vec<(f32, f32)> {
        let ppus = self.px_per_us();
        let px_per_day = (ppus * MICROS_PER_DAY as f64) as f32;
        let px_per_hour = (ppus * MICROS_PER_HOUR as f64) as f32;
        let emit_days = px_per_day >= 2.0;
        let emit_off_hours = px_per_hour >= 4.0;
        if !emit_days && !emit_off_hours {
            return Vec::new();
        }

        // Spans in absolute time, naturally sorted by the day walk.
        let mut spans: Vec<(i64, i64)> = Vec::new();
        let mut day = self.floor_to(range.start, TierUnit::Day);
        let mut guard = 0usize;
        while day < range.end {
            let next = self.next_boundary(day, TierUnit::Day);
            let (date, _) = to_civil(day, self.frame);
            if !self.calendar.is_working_day(date) {
                if emit_days {
                    spans.push((day.0, next.0));
                }
            } else if emit_off_hours {
                let ws = from_civil(date, self.calendar.day_start, self.frame).0;
                let we = from_civil(date, self.calendar.day_end, self.frame).0;
                if ws > day.0 {
                    spans.push((day.0, ws));
                }
                if we < next.0 {
                    spans.push((we, next.0));
                }
            }
            day = next;
            guard += 1;
            if guard > MAX_NON_WORKING_DAYS {
                break;
            }
        }

        // Merge contiguous spans.
        let mut merged: Vec<(i64, i64)> = Vec::new();
        for s in spans {
            match merged.last_mut() {
                Some(last) if s.0 <= last.1 => {
                    if s.1 > last.1 {
                        last.1 = s.1;
                    }
                }
                _ => merged.push(s),
            }
        }

        // Map to viewport-clamped x spans.
        merged
            .iter()
            .filter_map(|&(a, b)| {
                let x0 = self.x_of(Timestamp(a)).max(0.0);
                let x1 = self.x_of(Timestamp(b)).min(self.viewport_width);
                if x1 > x0 { Some((x0, x1)) } else { None }
            })
            .collect()
    }

    /// Pure per-frame resolution. INV-335-G5: consumers share one instance
    /// per frame; none recomputes independently.
    pub fn resolve(&self) -> TimeGeometryResolution {
        let range = self.visible_range();
        let (top, bottom) = tier_units(self.scale);
        let tiers = vec![
            self.build_tier(top, true, range),
            self.build_tier(bottom, false, range),
        ];
        let gridlines = self.build_gridlines(bottom, range);
        let non_working = self.build_non_working(range);
        let now_x = self.now.and_then(|n| {
            let x = self.x_of(n);
            if x >= 0.0 && x <= self.viewport_width {
                Some(x)
            } else {
                None
            }
        });
        TimeGeometryResolution {
            visible_range: range,
            tiers,
            gridlines,
            non_working,
            now_x,
        }
    }

    // ------------------------------------------------------- working time

    /// Microseconds of working time in [a, b) under the default calendar.
    pub fn working_time_between(&self, a: Timestamp, b: Timestamp) -> i64 {
        if b <= a {
            return 0;
        }
        let mut total = 0i64;
        let mut day = self.floor_to(a, TierUnit::Day);
        let mut guard = 0usize;
        while day < b {
            let next = self.next_boundary(day, TierUnit::Day);
            let (date, _) = to_civil(day, self.frame);
            if self.calendar.is_working_day(date) {
                let ws = from_civil(date, self.calendar.day_start, self.frame).0;
                let we = from_civil(date, self.calendar.day_end, self.frame).0;
                let s = ws.max(a.0);
                let e = we.min(b.0);
                if e > s {
                    total += e - s;
                }
            }
            day = next;
            guard += 1;
            if guard > 100_000 {
                break;
            }
        }
        total
    }
}
