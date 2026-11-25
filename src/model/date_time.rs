//! Civil date/time selection shared by the five picker presentations.
//!
//! Values have no locale, timezone, display step, Widget or rendering ownership.

use crate::model::date_value::{DateValue, days_in_month};
use crate::model::time_value::TimeValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeMode {
    Date,
    Time,
    DateTime,
    DateRange,
    DateTimeRange,
}

impl DateTimeMode {
    pub fn has_date(self) -> bool {
        self != Self::Time
    }

    pub fn has_time(self) -> bool {
        matches!(self, Self::Time | Self::DateTime | Self::DateTimeRange)
    }

    pub fn is_range(self) -> bool {
        matches!(self, Self::DateRange | Self::DateTimeRange)
    }
}

/// A timezone-neutral civil date and time, not an absolute instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeValue {
    pub date: DateValue,
    pub time: TimeValue,
}

impl DateTimeValue {
    pub fn new(date: DateValue, time: TimeValue) -> Result<Self, DateTimeSelectionError> {
        validate_date(date)?;
        validate_time(time)?;
        Ok(Self { date, time })
    }

    fn key(self) -> (DateValue, u8, u8, u8) {
        (
            self.date,
            self.time.hour,
            self.time.minute,
            self.time.second.unwrap_or(0),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeSelection {
    Date(DateValue),
    Time(TimeValue),
    DateTime(DateTimeValue),
    /// Both calendar dates are included.
    DateRange {
        start: DateValue,
        end: DateValue,
    },
    /// Civil interval [start, end), with a strictly later end.
    DateTimeRange {
        start: DateTimeValue,
        end: DateTimeValue,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeEndpoint {
    Start,
    End,
}

#[derive(Clone, Copy)]
pub(crate) struct CivilDateBounds {
    pub(crate) min: DateValue,
    pub(crate) max: DateValue,
}

impl CivilDateBounds {
    pub(crate) fn new(min: DateValue, max: DateValue) -> Result<Self, DateTimeSelectionError> {
        validate_date(min)?;
        validate_date(max)?;
        if min > max {
            return Err(DateTimeSelectionError::InvalidDateBounds);
        }
        Ok(Self { min, max })
    }

    pub(crate) fn contains(self, date: DateValue) -> bool {
        self.min <= date && date <= self.max
    }

    pub(crate) fn month_allowed(self, year: i32, month: u8) -> bool {
        let Some(first) = DateValue::new(year, month, 1) else {
            return false;
        };
        let last = DateValue::new(year, month, days_in_month(year, month)).unwrap();
        first <= self.max && last >= self.min
    }

    pub(crate) fn view_date(self, date: DateValue) -> DateValue {
        date.clamp(self.min, self.max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeSelectionError {
    InvalidDate,
    InvalidTime,
    ModeMismatch,
    InvalidEndpoint,
    InvalidRange,
    Unavailable,
    MissingDateContext,
    MissingRangePolicy,
    InvalidStep,
    InvalidDateBounds,
    OutsideDateBounds,
}

impl std::fmt::Display for DateTimeSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidDate => "Invalid calendar date",
            Self::InvalidTime => "Invalid time of day",
            Self::ModeMismatch => "Selection does not match picker mode",
            Self::InvalidEndpoint => "This selection has no end boundary",
            Self::InvalidRange => "End must follow start",
            Self::Unavailable => "Selection is unavailable",
            Self::MissingDateContext => "Time availability requires an explicit date",
            Self::MissingRangePolicy => "Range availability requires a complete interval policy",
            Self::InvalidStep => "Selection increments must fit the hour or minute column",
            Self::InvalidDateBounds => "Minimum date must not follow maximum date",
            Self::OutsideDateBounds => "Selection is outside date bounds",
        })
    }
}

impl std::error::Error for DateTimeSelectionError {}

fn validate_date(date: DateValue) -> Result<(), DateTimeSelectionError> {
    if DateValue::new(date.year, date.month, date.day) == Some(date) {
        Ok(())
    } else {
        Err(DateTimeSelectionError::InvalidDate)
    }
}

fn validate_time(time: TimeValue) -> Result<(), DateTimeSelectionError> {
    if time.hour < 24 && time.minute < 60 && time.second.is_none_or(|second| second < 60) {
        Ok(())
    } else {
        Err(DateTimeSelectionError::InvalidTime)
    }
}

impl DateTimeSelection {
    pub fn mode(self) -> DateTimeMode {
        match self {
            Self::Date(_) => DateTimeMode::Date,
            Self::Time(_) => DateTimeMode::Time,
            Self::DateTime(_) => DateTimeMode::DateTime,
            Self::DateRange { .. } => DateTimeMode::DateRange,
            Self::DateTimeRange { .. } => DateTimeMode::DateTimeRange,
        }
    }

    pub fn validate(self) -> Result<(), DateTimeSelectionError> {
        match self {
            Self::Date(date) => validate_date(date),
            Self::Time(time) => validate_time(time),
            Self::DateTime(value) => DateTimeValue::new(value.date, value.time).map(|_| ()),
            Self::DateRange { start, end } => {
                validate_date(start)?;
                validate_date(end)?;
                if start <= end {
                    Ok(())
                } else {
                    Err(DateTimeSelectionError::InvalidRange)
                }
            }
            Self::DateTimeRange { start, end } => {
                DateTimeValue::new(start.date, start.time)?;
                DateTimeValue::new(end.date, end.time)?;
                if start.key() < end.key() {
                    Ok(())
                } else {
                    Err(DateTimeSelectionError::InvalidRange)
                }
            }
        }
    }

    pub fn date(self, endpoint: DateTimeEndpoint) -> Option<DateValue> {
        match (self, endpoint) {
            (Self::Date(date), DateTimeEndpoint::Start) => Some(date),
            (Self::DateTime(value), DateTimeEndpoint::Start) => Some(value.date),
            (Self::DateRange { start, .. }, DateTimeEndpoint::Start) => Some(start),
            (Self::DateRange { end, .. }, DateTimeEndpoint::End) => Some(end),
            (Self::DateTimeRange { start, .. }, DateTimeEndpoint::Start) => Some(start.date),
            (Self::DateTimeRange { end, .. }, DateTimeEndpoint::End) => Some(end.date),
            _ => None,
        }
    }

    pub fn time(self, endpoint: DateTimeEndpoint) -> Option<TimeValue> {
        match (self, endpoint) {
            (Self::Time(time), DateTimeEndpoint::Start) => Some(time),
            (Self::DateTime(value), DateTimeEndpoint::Start) => Some(value.time),
            (Self::DateTimeRange { start, .. }, DateTimeEndpoint::Start) => Some(start.time),
            (Self::DateTimeRange { end, .. }, DateTimeEndpoint::End) => Some(end.time),
            _ => None,
        }
    }

    pub(crate) fn with_date(
        self,
        endpoint: DateTimeEndpoint,
        date: DateValue,
    ) -> Result<Self, DateTimeSelectionError> {
        validate_date(date)?;
        Ok(match (self, endpoint) {
            (Self::Date(_), DateTimeEndpoint::Start) => Self::Date(date),
            (Self::DateTime(value), DateTimeEndpoint::Start) => {
                Self::DateTime(DateTimeValue { date, ..value })
            }
            (Self::DateRange { end, .. }, DateTimeEndpoint::Start) => {
                Self::DateRange { start: date, end }
            }
            (Self::DateRange { start, .. }, DateTimeEndpoint::End) => {
                Self::DateRange { start, end: date }
            }
            (Self::DateTimeRange { start, end }, DateTimeEndpoint::Start) => Self::DateTimeRange {
                start: DateTimeValue { date, ..start },
                end,
            },
            (Self::DateTimeRange { start, end }, DateTimeEndpoint::End) => Self::DateTimeRange {
                start,
                end: DateTimeValue { date, ..end },
            },
            _ => return Err(DateTimeSelectionError::InvalidEndpoint),
        })
    }

    pub(crate) fn with_time(
        self,
        endpoint: DateTimeEndpoint,
        time: TimeValue,
    ) -> Result<Self, DateTimeSelectionError> {
        validate_time(time)?;
        Ok(match (self, endpoint) {
            (Self::Time(_), DateTimeEndpoint::Start) => Self::Time(time),
            (Self::DateTime(value), DateTimeEndpoint::Start) => {
                Self::DateTime(DateTimeValue { time, ..value })
            }
            (Self::DateTimeRange { start, end }, DateTimeEndpoint::Start) => Self::DateTimeRange {
                start: DateTimeValue { time, ..start },
                end,
            },
            (Self::DateTimeRange { start, end }, DateTimeEndpoint::End) => Self::DateTimeRange {
                start,
                end: DateTimeValue { time, ..end },
            },
            _ => return Err(DateTimeSelectionError::InvalidEndpoint),
        })
    }
}

/// One complete working selection; edits never overwrite the committed value.
pub(crate) struct DateTimeSelectionState {
    committed: DateTimeSelection,
    working: DateTimeSelection,
}

impl DateTimeSelectionState {
    pub(crate) fn new(value: DateTimeSelection) -> Result<Self, DateTimeSelectionError> {
        value.validate()?;
        Ok(Self {
            committed: value,
            working: value,
        })
    }

    pub(crate) fn committed(&self) -> DateTimeSelection {
        self.committed
    }
    pub(crate) fn working(&self) -> DateTimeSelection {
        self.working
    }

    pub(crate) fn set_working(
        &mut self,
        value: DateTimeSelection,
    ) -> Result<(), DateTimeSelectionError> {
        if self.committed.mode() != value.mode() {
            return Err(DateTimeSelectionError::ModeMismatch);
        }
        self.working = value;
        Ok(())
    }

    pub(crate) fn set_value(
        &mut self,
        value: DateTimeSelection,
    ) -> Result<(), DateTimeSelectionError> {
        value.validate()?;
        if self.committed.mode() != value.mode() {
            return Err(DateTimeSelectionError::ModeMismatch);
        }
        self.committed = value;
        self.working = value;
        Ok(())
    }

    pub(crate) fn commit(
        &mut self,
        is_available: impl FnOnce(DateTimeSelection) -> bool,
    ) -> Result<Option<DateTimeSelection>, DateTimeSelectionError> {
        self.working.validate()?;
        if !is_available(self.working) {
            return Err(DateTimeSelectionError::Unavailable);
        }
        if self.committed == self.working {
            return Ok(None);
        }
        self.committed = self.working;
        Ok(Some(self.committed))
    }

    pub(crate) fn cancel(&mut self) {
        self.working = self.committed;
    }
}

/// Frame-independent calendar navigation; no selection or commit authority.
#[derive(Clone, Copy)]
pub(crate) struct CalendarView {
    pub(crate) year: i32,
    pub(crate) month: u8,
    pub(crate) first_dow: u8,
}

impl CalendarView {
    pub(crate) fn new(date: DateValue, first_dow: u8) -> Self {
        Self {
            year: date.year,
            month: date.month,
            first_dow: first_dow % 7,
        }
    }

    pub(crate) fn cells(self) -> [Option<DateValue>; 42] {
        let first = DateValue::new(self.year, self.month, 1);
        let Some(first) = first else {
            return [None; 42];
        };
        let sunday_dow = (crate::model::time::day_of_week(first) + 1) % 7;
        let offset = (sunday_dow + 7 - self.first_dow) % 7;
        let count = days_in_month(self.year, self.month);
        std::array::from_fn(|index| {
            let day = index as i32 - offset as i32 + 1;
            if day > 0 && day <= count as i32 {
                DateValue::new(self.year, self.month, day as u8)
            } else {
                None
            }
        })
    }

    pub(crate) fn move_month(&mut self, delta: i32) {
        let total = self.year as i64 * 12 + self.month as i64 - 1 + delta as i64;
        let year = total.div_euclid(12);
        if let Ok(year) = i32::try_from(year) {
            self.year = year;
            self.month = (total.rem_euclid(12) + 1) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(day: u8) -> DateValue {
        DateValue::new(2026, 10, day).unwrap()
    }
    fn value(day: u8, hour: u8, minute: u8) -> DateTimeValue {
        DateTimeValue::new(date(day), TimeValue::new(hour, minute, Some(17))).unwrap()
    }

    #[test]
    fn date_time_picker_values_preserve_all_modes_and_exact_times() {
        let selections = [
            DateTimeSelection::Date(date(10)),
            DateTimeSelection::Time(TimeValue::new(9, 7, Some(17))),
            DateTimeSelection::DateTime(value(10, 9, 7)),
            DateTimeSelection::DateRange {
                start: date(10),
                end: date(10),
            },
            DateTimeSelection::DateTimeRange {
                start: value(10, 9, 7),
                end: value(11, 10, 13),
            },
        ];
        for selection in selections {
            let mut state = DateTimeSelectionState::new(selection).unwrap();
            assert_eq!(state.working(), selection);
            assert_eq!(state.commit(|_| true), Ok(None));
            assert_eq!(state.committed(), selection);
        }
    }

    #[test]
    fn date_time_picker_range_never_swaps_or_commits_invalid_working_endpoints() {
        let initial = DateTimeSelection::DateTimeRange {
            start: value(10, 9, 7),
            end: value(11, 10, 13),
        };
        let mut state = DateTimeSelectionState::new(initial).unwrap();
        let reversed = initial
            .with_date(DateTimeEndpoint::Start, date(12))
            .unwrap();
        state.set_working(reversed).unwrap();
        assert_eq!(state.working(), reversed);
        assert_eq!(
            state.commit(|_| true),
            Err(DateTimeSelectionError::InvalidRange)
        );
        assert_eq!(state.committed(), initial);
        let repaired = reversed.with_date(DateTimeEndpoint::End, date(13)).unwrap();
        state.set_working(repaired).unwrap();
        assert_eq!(
            state.commit(|selection| {
                assert_eq!(selection, repaired);
                false
            }),
            Err(DateTimeSelectionError::Unavailable)
        );
        assert_eq!(state.committed(), initial);
        assert_eq!(state.commit(|_| true), Ok(Some(repaired)));
    }

    #[test]
    fn date_time_picker_cancel_is_explicit_and_component_edits_preserve_seconds() {
        let initial = DateTimeSelection::DateTime(value(10, 9, 7));
        let mut state = DateTimeSelectionState::new(initial).unwrap();
        let edited = initial
            .with_time(DateTimeEndpoint::Start, TimeValue::new(10, 7, Some(17)))
            .unwrap();
        state.set_working(edited).unwrap();
        assert_eq!(state.committed(), initial);
        assert_eq!(
            state
                .working()
                .time(DateTimeEndpoint::Start)
                .unwrap()
                .second,
            Some(17)
        );
        state.cancel();
        assert_eq!(state.working(), initial);
    }

    #[test]
    fn date_time_picker_calendar_is_bounded_and_navigation_is_only_view_state() {
        let mut calendar = CalendarView::new(DateValue::new(2024, 2, 29).unwrap(), 1);
        let cells = calendar.cells();
        assert_eq!(cells.iter().flatten().count(), 29);
        assert_eq!(cells[3], DateValue::new(2024, 2, 1));
        calendar.move_month(12);
        assert_eq!((calendar.year, calendar.month), (2025, 2));
        assert_eq!(calendar.cells().iter().flatten().count(), 28);
        calendar.move_month(-14);
        assert_eq!((calendar.year, calendar.month), (2023, 12));
    }
}
