// SPEC-HAL-238: DateValue — timezone-neutral immutable date type.
// Validation at construction, chronological ordering, leap-year correctness.

/// Timezone-neutral date value. Immutable after construction.
/// All fields are pub for read access; mutation only via succ/pred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateValue {
    pub year: i32,
    pub month: u8, // 1–12
    pub day: u8,   // 1–N
}

/// Returns true if the given year is a leap year per Gregorian rules.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in the given month of the given year.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 => 31,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 0,
    }
}

impl DateValue {
    /// Construct a validated DateValue. Returns None on invalid input.
    pub fn new(year: i32, month: u8, day: u8) -> Option<DateValue> {
        if month < 1 || month > 12 {
            return None;
        }
        let max_day = days_in_month(year, month);
        if day < 1 || day > max_day {
            return None;
        }
        Some(DateValue { year, month, day })
    }

    /// Format as ISO 8601 "YYYY-MM-DD" with zero-padded month and day.
    pub fn to_iso8601(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Parse from ISO 8601 "YYYY-MM-DD". Returns None on malformed input.
    pub fn from_iso8601(s: &str) -> Option<DateValue> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let year: i32 = parts[0].parse().ok()?;
        let month: u8 = parts[1].parse().ok()?;
        let day: u8 = parts[2].parse().ok()?;
        DateValue::new(year, month, day)
    }

    /// Next calendar day. Returns None at maximum representable date.
    pub fn succ(&self) -> Option<DateValue> {
        let max_day = days_in_month(self.year, self.month);
        if self.day < max_day {
            return DateValue::new(self.year, self.month, self.day + 1);
        }
        if self.month < 12 {
            return DateValue::new(self.year, self.month + 1, 1);
        }
        // Year rollover
        DateValue::new(self.year + 1, 1, 1)
    }

    /// Previous calendar day. Returns None at minimum representable date.
    pub fn pred(&self) -> Option<DateValue> {
        if self.day > 1 {
            return DateValue::new(self.year, self.month, self.day - 1);
        }
        if self.month > 1 {
            let prev_month = self.month - 1;
            let prev_day = days_in_month(self.year, prev_month);
            return DateValue::new(self.year, prev_month, prev_day);
        }
        // Year rollback
        DateValue::new(self.year - 1, 12, 31)
    }
}

impl std::fmt::Display for DateValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_iso8601())
    }
}
