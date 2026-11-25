// SPEC-HAL-334: Time Model — Timestamp, TimeRange, TimeFrame, civil projection.
// One representation of time across the framework: i64 microseconds, UNIX epoch, UTC.
// Civil projection is fixed-offset only; DST and tzdb are out of scope (INV-334-5).

use crate::model::date_value::DateValue;
use crate::model::time_value::TimeValue;

pub const MICROS_PER_SECOND: i64 = 1_000_000;
pub const MICROS_PER_MINUTE: i64 = 60 * MICROS_PER_SECOND;
pub const MICROS_PER_HOUR: i64 = 60 * MICROS_PER_MINUTE;
pub const MICROS_PER_DAY: i64 = 24 * MICROS_PER_HOUR;

/// Absolute instant: microseconds since UNIX epoch, UTC. INV-334-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp(pub i64);

impl Timestamp {
    pub fn from_secs(secs: i64) -> Self {
        Self(secs * MICROS_PER_SECOND)
    }

    pub fn from_millis(ms: i64) -> Self {
        Self(ms * 1_000)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / MICROS_PER_SECOND as f64
    }
}

/// Half-open absolute range [start, end). INV-334-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeRange {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl TimeRange {
    pub fn new(start: Timestamp, end: Timestamp) -> Self {
        debug_assert!(start <= end, "TimeRange requires start <= end");
        Self {
            start,
            end: if end < start { start } else { end },
        }
    }

    pub fn duration_us(self) -> i64 {
        self.end.0 - self.start.0
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// start <= ts < end.
    pub fn contains(self, ts: Timestamp) -> bool {
        self.start <= ts && ts < self.end
    }

    /// Half-open intersection test: edge-touching ranges do not intersect.
    pub fn intersects(self, other: TimeRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    pub fn intersect(self, other: TimeRange) -> Option<TimeRange> {
        let s = self.start.max(other.start);
        let e = self.end.min(other.end);
        if s < e {
            Some(TimeRange { start: s, end: e })
        } else {
            None
        }
    }
}

/// Civil projection frame: fixed offset from UTC, in minutes. SPEC-HAL-334 §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeFrame {
    pub utc_offset_minutes: i32,
}

/// Days since 1970-01-01 for a proleptic-Gregorian civil date.
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (month as i64 + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + day as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Civil date from days since 1970-01-01. Inverse of days_from_civil.
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
    let year = (y + if month <= 2 { 1 } else { 0 }) as i32;
    (year, month, day)
}

/// Days since 1970-01-01 for a civil date. SPEC-HAL-334 §7 on-demand surface.
pub fn epoch_days(date: DateValue) -> i64 {
    days_from_civil(date.year, date.month, date.day)
}

/// Day of week, 0 = Monday … 6 = Sunday.
pub fn day_of_week(date: DateValue) -> u8 {
    let days = days_from_civil(date.year, date.month, date.day);
    ((days + 3).rem_euclid(7)) as u8
}

/// Project an absolute instant into civil date + time under a fixed-offset frame.
/// Total and monotonic per INV-334-4. Sub-second microseconds truncate.
pub fn to_civil(ts: Timestamp, frame: TimeFrame) -> (DateValue, TimeValue) {
    let shifted = ts.0 + frame.utc_offset_minutes as i64 * MICROS_PER_MINUTE;
    let days = shifted.div_euclid(MICROS_PER_DAY);
    let us_of_day = shifted.rem_euclid(MICROS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    // civil_from_days output is valid by construction; literal bypasses the Option ctor.
    let date = DateValue { year, month, day };
    let secs = us_of_day / MICROS_PER_SECOND;
    let time = TimeValue {
        hour: (secs / 3600) as u8,
        minute: ((secs % 3600) / 60) as u8,
        second: Some((secs % 60) as u8),
    };
    (date, time)
}

/// Inverse of to_civil under the same frame. `second: None` reads as 0.
pub fn from_civil(date: DateValue, time: TimeValue, frame: TimeFrame) -> Timestamp {
    let days = days_from_civil(date.year, date.month, date.day);
    let secs = time.hour as i64 * 3600 + time.minute as i64 * 60 + time.second.unwrap_or(0) as i64;
    Timestamp(
        days * MICROS_PER_DAY + secs * MICROS_PER_SECOND
            - frame.utc_offset_minutes as i64 * MICROS_PER_MINUTE,
    )
}
